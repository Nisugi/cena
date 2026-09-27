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

use cena_behavior::operation::Underway;
use cena_behavior::{hunt, travel};
use cena_session::operation::{Allows, Control, Ended, Performer, Start, Started, Steer, Work};
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
    let (session, observer) = (handle.clone(), observer.clone());
    let start: Start = Arc::new(move |line: &str| {
        started(&session, &observer, travel.as_ref(), hunt.as_ref(), line)
    });
    let performer = Performer {
        allowed: ALLOWED.to_owned(),
        allows,
        start,
    };
    if !handle.set_performer(performer) {
        eprintln!("  !! [agent] something already runs this session's behaviors for an agent");
    }
}

/// Start `line`, one `allows` took.
fn started(
    handle: &SessionHandle,
    observer: &SessionObserver,
    travel: Option<&Arc<travel::Desk>>,
    hunt: Option<&Arc<hunt::Desk>>,
    line: &str,
) -> Started {
    let stop = CancellationToken::new();
    let steer: Steer = {
        let stop = stop.clone();
        Arc::new(move |control: Control| match control {
            Control::Stop => {
                stop.cancel();
                Ok(())
            }
        })
    };
    let job = match job(line) {
        Ok(job) => job,
        Err(why) => return at_once(steer, Ended::plainly(Work::NoOpportunity, &why)),
    };
    let (handle, observer) = (handle.clone(), observer.clone());
    match job {
        Job::Walk(travel::Command::Stop) => {
            at_once(steer, stopping(travel.is_some_and(|d| d.stop())))
        }
        Job::Hunt(hunt::Command::Stop) => at_once(steer, stopping(hunt.is_some_and(|d| d.stop()))),
        Job::Walk(command) => {
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
                match desk.underway(&handle, joined, command) {
                    Some(underway) => follow(underway, &stop, hunt::HuntEnd::ended).await,
                    None => Ended::plainly(Work::NoOpportunity, "not_started"),
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
    use super::{canonical, job};

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
