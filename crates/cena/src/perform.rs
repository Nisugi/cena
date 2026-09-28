//! What an agent may run at the `behaviors` level, and how each is started as
//! an operation (`plan/35` §3 and §6, M7 step 3).
//!
//! # Start and steer, and nothing that writes
//!
//! > **AUTHOR, 2026-09-27**, choosing among three: *start and steer only*.
//!
//! So an agent may walk (`go2 <place>`, `go2 stop`), hunt (`hunt <profile>`,
//! `quick`, `bounty`, `hunt stop`), heal (`heal`, `heal stock`, `heal fill`),
//! keep spells up (`keep`) and waggle (`waggle`). Everything else is refused,
//! each for its own reason:
//!
//! - **what writes a setting or a profile**: a hunt profile's steps are game
//!   commands, so editing one would get round the Commands level;
//! - **`multi` and `foreach`**: they send any game line;
//! - **`sc`**: it casts at anyone, players included;
//! - **a group hunt**: it starts other characters' hunts, whose levels are
//!   their own players';
//! - **`agent`**: an agent never sets its own level.
//!
//! The list grows from a real case, as the denylist does (`plan/35` §3).
//!
//! # One run's controls
//!
//! Each operation's stop is its own token, joined to the run's
//! ([`cena_behavior::operation::Steering`]) once the run has begun; a stop
//! that arrives before then keeps it from beginning.

use std::sync::Arc;

use cena_behavior::operation::{Steering, Underway};
use cena_behavior::{hunt, travel};
use cena_session::operation::{
    Allows, Control, Ended, Halt, Performer, Progress, Reporter, Start, Started, Steer, Work,
};
use cena_session::{AuthorityToken, SessionHandle, SessionObserver};
use tokio_util::sync::CancellationToken;

/// What an agent is told it may run.
pub(crate) const ALLOWED: &str = "go2 <place>, go2 stop, hunt <profile>, hunt <profile> quick, \
hunt <profile> bounty, hunt stop, heal (spellcast, ranged, blood), heal stock, heal fill, keep, \
waggle [names]";

/// The authority travel's walks claim (`travel.rs`).
pub(crate) const TRAVEL_TOKEN: AuthorityToken = AuthorityToken(2);

/// The authority the hunt desk's runs claim (`hunt.rs`).
pub(crate) const HUNT_TOKEN: AuthorityToken = AuthorityToken(3);

/// A line an agent may run, parsed.
enum Job {
    Walk(travel::Command),
    Hunt(hunt::Command),
}

/// The job `line` is, or why an agent may not run it.
fn job(line: &str) -> Result<Job, String> {
    if let Some(parsed) = travel::parse_command(line) {
        return match parsed? {
            command @ (travel::Command::Go(_) | travel::Command::Stop) => Ok(Job::Walk(command)),
            _ => Err(format!(
                "that go2 command saves, lists or routes; an agent may run {ALLOWED}"
            )),
        };
    }
    if let Some(parsed) = hunt::parse_command(line) {
        return match parsed? {
            command @ (hunt::Command::Run(_)
            | hunt::Command::Quick(_)
            | hunt::Command::Bounty(_)
            | hunt::Command::Stop
            | hunt::Command::Heal { .. }
            | hunt::Command::Stock { .. }
            | hunt::Command::Keep
            | hunt::Command::Waggle(_)) => Ok(Job::Hunt(command)),
            hunt::Command::Group { .. } => Err(
                "a group hunt starts other characters' hunts, whose levels are their own; an agent may not start one"
                    .to_owned(),
            ),
            hunt::Command::Sc(_) => {
                Err("sc casts at anyone, players included; an agent may not run it".to_owned())
            }
            _ => Err(format!(
                "that reads or changes settings and profiles, which an agent may not; it may run {ALLOWED}"
            )),
        };
    }
    Err(format!("an agent may run {ALLOWED}"))
}

/// A line as it is kept and compared: its words, one space apart.
fn canonical(line: &str) -> String {
    line.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// Let an agent run behaviors on this session: travel's walks and the hunt
/// desk's runs, where each exists.
pub(crate) fn install(
    handle: &SessionHandle,
    observer: &SessionObserver,
    travel: Option<Arc<travel::Desk>>,
    hunt: Option<Arc<hunt::Desk>>,
) {
    let allows: Allows = Arc::new(|line: &str| {
        let line = canonical(line);
        job(&line).map(|_| line)
    });
    // A takeover stops what runs first, as the player's own stops do.
    let (walking, hunting) = (travel.clone(), hunt.clone());
    let halt: Halt = Arc::new(move || {
        let _ = walking.as_ref().is_some_and(|desk| desk.stop());
        let _ = hunting.as_ref().is_some_and(|desk| desk.stop());
    });
    let (session, observer) = (handle.clone(), observer.clone());
    let start: Start = Arc::new(move |line: &str, reporter: Reporter| {
        started(
            &session,
            &observer,
            travel.as_ref(),
            hunt.as_ref(),
            line,
            reporter,
        )
    });
    let performer = Performer {
        allowed: ALLOWED.to_owned(),
        allows,
        start,
        halt,
    };
    if !handle.set_performer(performer) {
        eprintln!("  !! [agent] something already runs this session's behaviors for an agent");
    }
}

/// The controls of a hunt once it has begun: filled by its operation's task,
/// read by its operation's [`Steer`].
type Reins = Arc<std::sync::Mutex<Option<Steering>>>;

/// How an operation is steered. Stop is its own token, joined to the run's
/// once it has begun; hold, resume and retreat are a hunt's alone, and reach
/// its run once it has begun (`cena_behavior::hunt`, steer's docs).
fn steer_for(stop: &CancellationToken, reins: Option<Reins>) -> Steer {
    let stop = stop.clone();
    Arc::new(move |control: Control| {
        if control == Control::Stop {
            stop.cancel();
            return Ok(());
        }
        let Some(reins) = &reins else {
            return Err(format!(
                "only a hunt can be told to {}; this can only be stopped",
                control.word()
            ));
        };
        let held = reins
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let Some(steering) = held.as_ref() else {
            return Err("the hunt has not begun yet; ask again in a moment".to_owned());
        };
        match control {
            Control::Hold => steering.hold(),
            Control::Resume => steering.resume(),
            Control::Retreat => steering.retreat(),
            Control::Stop => steering.stop(),
        }
        Ok(())
    })
}

/// Start `line`, one `allows` took.
fn started(
    handle: &SessionHandle,
    observer: &SessionObserver,
    travel: Option<&Arc<travel::Desk>>,
    hunt: Option<&Arc<hunt::Desk>>,
    line: &str,
    reporter: Reporter,
) -> Started {
    let stop = CancellationToken::new();
    let job = match job(line) {
        Ok(job) => job,
        Err(why) => {
            return at_once(
                steer_for(&stop, None),
                Ended::plainly(Work::NoOpportunity, &why),
            );
        }
    };
    let (handle, observer) = (handle.clone(), observer.clone());
    match job {
        Job::Walk(travel::Command::Stop) => at_once(
            steer_for(&stop, None),
            stopping(travel.is_some_and(|d| d.stop())),
        ),
        Job::Hunt(hunt::Command::Stop) => at_once(
            steer_for(&stop, None),
            stopping(hunt.is_some_and(|d| d.stop())),
        ),
        Job::Walk(command) => {
            let steer = steer_for(&stop, None);
            let Some(desk) = travel.cloned() else {
                return at_once(steer, Ended::plainly(Work::NoOpportunity, "no_map"));
            };
            let ended = async move {
                let Ok(joined) = observer.subscribe().await else {
                    return Ended::plainly(Work::NoOpportunity, "session_unreadable");
                };
                if stop.is_cancelled() {
                    return Ended::plainly(Work::Interrupted, "stopped");
                }
                match desk.underway(&handle, joined, command) {
                    Some(underway) => follow(underway, &stop, travel::Travelled::ended).await,
                    None => Ended::plainly(Work::NoOpportunity, "not_started"),
                }
            };
            Started {
                ended: Box::pin(ended),
                steer,
                token: Some(TRAVEL_TOKEN),
            }
        }
        Job::Hunt(command) => {
            // A hunt proper is held and retreated; a heal, a keep or a
            // waggle, run by the same desk, is only stopped.
            let hunting = matches!(
                command,
                hunt::Command::Run(_) | hunt::Command::Quick(_) | hunt::Command::Bounty(_)
            );
            let reins: Option<Reins> = hunting.then(Reins::default);
            let steer = steer_for(&stop, reins.clone());
            let Some(desk) = hunt.cloned() else {
                return at_once(steer, Ended::plainly(Work::NoOpportunity, "no_map"));
            };
            let ended = async move {
                let Ok(joined) = observer.subscribe().await else {
                    return Ended::plainly(Work::NoOpportunity, "session_unreadable");
                };
                if stop.is_cancelled() {
                    return Ended::plainly(Work::Interrupted, "stopped");
                }
                let Some(underway) = desk.underway(&handle, joined, command) else {
                    return Ended::plainly(Work::NoOpportunity, "not_started");
                };
                if let Some(reins) = &reins {
                    *reins
                        .lock()
                        .unwrap_or_else(std::sync::PoisonError::into_inner) =
                        Some(underway.steering.clone());
                }
                let feed = underway.steering.progress();
                tokio::select! {
                    ended = follow(underway, &stop, hunt::HuntEnd::ended) => ended,
                    never = forward(feed, reporter) => match never {},
                }
            };
            Started {
                ended: Box::pin(ended),
                steer,
                token: Some(HUNT_TOKEN),
            }
        }
    }
}

/// Pass a run's progress on to its operation for as long as the run goes
/// on; the caller stops waiting on this when the run ends.
async fn forward(
    mut feed: tokio::sync::watch::Receiver<Option<Progress>>,
    reporter: Reporter,
) -> std::convert::Infallible {
    loop {
        let progress = feed.borrow_and_update().clone();
        if let Some(progress) = progress {
            reporter.progress(progress);
        }
        if feed.changed().await.is_err() {
            // Every sender gone: nothing more will come.
            return std::future::pending().await;
        }
    }
}

/// `go2 stop` or `hunt stop`, done as it was started: whether there was
/// anything to stop.
fn stopping(stopped: bool) -> Ended {
    if stopped {
        Ended::plainly(Work::Completed, "stopped")
    } else {
        Ended::plainly(Work::NoOpportunity, "nothing_running")
    }
}

/// An operation over before it began.
fn at_once(steer: Steer, ended: Ended) -> Started {
    Started {
        ended: Box::pin(std::future::ready(ended)),
        steer,
        token: None,
    }
}

/// Wait out a run, stopping it if `stop` is cancelled first, and read its
/// ending.
async fn follow<T>(
    underway: Underway<T>,
    stop: &CancellationToken,
    read: fn(&T) -> Ended,
) -> Ended {
    let Underway { mut task, steering } = underway;
    let end = tokio::select! {
        end = &mut task => end,
        () = stop.cancelled() => {
            steering.stop();
            task.await
        }
    };
    end.map_or_else(
        |_| Ended::plainly(Work::Unknown, "task_failed"),
        |end| read(&end),
    )
}

#[cfg(test)]
mod tests {
    use super::{Reins, canonical, job, steer_for};
    use cena_behavior::operation::Steering;
    use cena_session::operation::Control;
    use tokio_util::sync::CancellationToken;

    /// Only a hunt is held, resumed or retreated, and only once its run has
    /// begun; anything can be stopped, at any time.
    #[test]
    fn only_a_hunt_is_held_and_only_once_it_has_begun() {
        let stop = CancellationToken::new();
        let walk = steer_for(&stop, None);
        assert!(walk(Control::Hold).is_err_and(|why| why.contains("only a hunt")));
        let reins = Reins::default();
        let hunt = steer_for(&stop, Some(reins.clone()));
        assert!(hunt(Control::Retreat).is_err_and(|why| why.contains("not begun")));
        *reins.lock().unwrap() = Some(Steering::new(CancellationToken::new()));
        assert_eq!(hunt(Control::Hold), Ok(()));
        assert_eq!(hunt(Control::Stop), Ok(()));
        assert!(stop.is_cancelled());
    }

    /// The author's list: start and steer, and nothing that writes.
    #[test]
    fn an_agent_may_start_and_steer_and_nothing_else() {
        for line in [
            "go2 bank",
            "go2 stop",
            "hunt ojandhaart",
            "hunt ojandhaart quick",
            "hunt ojandhaart bounty",
            "hunt stop",
            "heal",
            "heal stock",
            "heal fill",
            "keep",
            "waggle Nerten",
        ] {
            assert!(job(line).is_ok(), "{line}: {:?}", job(line).err());
        }
        for (line, why) in [
            ("hunt set ojandhaart rest.stop_after 3", "settings"),
            ("hunt import bigshot.yaml", "settings"),
            ("hunt ojandhaart with Nerten", "group"),
            ("sc 118 Nerten", "players"),
            ("heal set container pouch", "settings"),
            ("go2 save bank = current", "saves"),
            ("multi 2,drop sword", "may run"),
            ("foreach gem in sack;drop gem", "may run"),
            ("agent level behaviors", "may run"),
            ("drop sword", "may run"),
        ] {
            let said = job(line).err().unwrap_or_default();
            assert!(said.contains(why), "{line}: {said:?}");
        }
        assert_eq!(
            canonical("  hunt   ojandhaart  quick "),
            "hunt ojandhaart quick"
        );
    }
}
