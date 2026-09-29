//! Hydra's own command line: every line that starts with the command symbol.
//!
//! **Installed when the session is built, before it connects** (author,
//! 2026-09-23: the symbol is "going to be for more than just travelling, so it
//! shouldn't be tied to a map, it should be something loaded on startup by
//! default"). It used to be opened by travel, and only when travel had a map
//! -- so with no map nothing claimed `;`, and `;go2 bank` was said in the room.
//!
//! The symbol decides and the word routes: a line marked as Hydra's goes to
//! the first handler that knows its word, and a word nobody knows is answered
//! "I do not know" by the session. It never reaches the game either way.
//!
//! Handlers join once the session knows enough to run them. Travel needs the
//! login's own state, so it registers after the login burst; a travel command
//! typed before then is told to wait rather than dropped silently.
//!
//! # Knowing when a command is over
//!
//! `;multi 2,get gem,;sc 401,put gem in sack` waits for `;sc 401` to finish
//! before the `put` (`crate::batch`). So a family whose commands start
//! something that goes on -- a walk, a hunt-desk run, a batch -- is a
//! [`Starter`], and hands back the task it started ([`Took::Started`]);
//! [`Commands::route`] gives that to whoever asked. Typed at the prompt, the
//! task is let go and runs on its own, as it always did.

use cena_session::command::claimant::{Claimed, Desk, Runner};
use cena_session::{Notice, NoticeKind, SessionHandle};
use std::sync::{Arc, OnceLock};

/// A family of commands: `Some` when the line was its own, `None` when the
/// word is not one it knows.
pub(crate) type Handler = Arc<dyn Fn(&str) -> Option<Claimed> + Send + Sync>;

/// A family whose commands may start something that goes on: `Some` when
/// the line was its own, with the task when it started one.
pub(crate) type Starter = Arc<dyn Fn(&str) -> Option<Took> + Send + Sync>;

/// How a family stops what it started, for `;stop`: `true` when something
/// was running.
pub(crate) type Stopper = Arc<dyn Fn() -> bool + Send + Sync>;

/// What a family did with a line of its own.
pub(crate) enum Took {
    /// Done, or answered, by the time it returned.
    Done,
    /// Started something that is over when this task is.
    Started(tokio::task::JoinHandle<()>),
}

/// What `;help` says: every family, and the word that says more of it
/// (`plan/44` Q04). Answered before any family is ready, since it needs
/// nothing but this list.
pub(crate) const HELP: &[&str] = &[
    "Hydra's commands start with your command symbol: . unless you changed it. None of them reaches the game.",
    "hunt help        hunt on a profile, see and change its settings, lead a group, stop",
    "heal help        heal with herbs: name the herb container, heal, stock it",
    "waggle help      cast a list of spells on people",
    "keep list        spells kept up: keep add <spell>, then keep; hunt stop ends it",
    "sc help          cast one spell as set up: sc 401; beside a hunt, never in its place",
    "trigger help     triggers: what they watch and what they do; the Triggers window edits them",
    "go2 help         walk to a place or a room, save places, stop",
    "loot, combat     reports on what was recorded: loot summary, combat hunts",
    "history help     read back what you saw: history tail, history last 15m, history search <text>",
    "sorter           show a container's contents one line per category: sorter on, off or status",
    "multi help, foreach help   run commands several times, or once for each item",
    "agent help       what an agent (a program such as Claude Code) may do with this character",
    "lich help        run your own Lich for this character, and keep it on",
    "stop             stop everything Hydra is doing on this character: a hunt, a walk, a batch",
    "keys import <file>   in a play window: a Wrayth key set into this character's keys (keys import global <file>: every character's)",
    "to <name> <command>, all <command>   send a command on another character, or on every one",
    "<script> [args]  run one of your Lich scripts; k, l, p, u as in Lich. scripts: where they are; scripts import, scripts check",
];

/// Whether a line, without its symbol, asks for [`HELP`].
fn asks_for_help(line: &str) -> bool {
    let line = line.trim();
    line.eq_ignore_ascii_case("help") || line == "?"
}

/// Whether a line, without its symbol, is `;stop`: everything Hydra is doing
/// on this character. The play window's Stop sends it (`plan/47` step 4).
fn asks_to_stop(line: &str) -> bool {
    line.trim().eq_ignore_ascii_case("stop")
}

/// The handlers the command line routes to, filled as each becomes ready.
#[derive(Clone, Default)]
pub(crate) struct Commands {
    travel: Arc<OnceLock<Starter>>,
    hunt: Arc<OnceLock<Starter>>,
    loot: Arc<OnceLock<Handler>>,
    combat: Arc<OnceLock<Handler>>,
    history: Arc<OnceLock<Handler>>,
    sorter: Arc<OnceLock<Handler>>,
    trigger: Arc<OnceLock<Handler>>,
    batch: Arc<OnceLock<Starter>>,
    agent: Arc<OnceLock<Handler>>,
    /// The player's own Lich (`crate::lich`).
    lich: Arc<OnceLock<Handler>>,
    relay: Arc<OnceLock<Starter>>,
    /// The player's own scripts (`crate::scripts`): heard last of all.
    scripts: Arc<OnceLock<Handler>>,
    /// What `;stop` stops: each family that starts something that goes on,
    /// by the word the player knows it by.
    stoppers: Arc<std::sync::Mutex<Vec<(&'static str, Stopper)>>>,
}

impl Commands {
    /// Put the command line on `handle`, with the default symbol until the
    /// character's own is known (`SessionHandle::set_command_symbol`).
    pub(crate) fn install(handle: &SessionHandle) -> Self {
        let commands = Self::default();
        let routes = commands.clone();
        let told = handle.clone();
        let runner: Runner = Arc::new(move |line: &str| {
            if asks_for_help(line) {
                let lines = HELP.iter().map(|&line| line.to_owned()).collect();
                told.say(Notice::table(NoticeKind::Info, lines));
                return Claimed::Done;
            }
            if asks_to_stop(line) {
                let stopped = routes.stop();
                told.say(Notice::line(
                    NoticeKind::Info,
                    if stopped.is_empty() {
                        "Nothing was running.".to_owned()
                    } else {
                        format!("Stopped: {}.", stopped.join(", "))
                    },
                ));
                return Claimed::Done;
            }
            // A task it started runs on by itself: nobody typing waits.
            if routes.route(line).is_some() {
                return Claimed::Done;
            }
            let starting = if cena_behavior::travel::parse_command(line).is_some() {
                Some("Travel")
            } else if cena_behavior::hunt::parse_command(line).is_some() {
                Some("Hunt")
            } else if line.split_whitespace().next() == Some("agent") {
                Some("Agent")
            } else if line.split_whitespace().next() == Some("lich") {
                Some("Lich")
            } else {
                None
            };
            if let Some(family) = starting {
                told.say(Notice::line(
                    NoticeKind::Warn,
                    format!(
                        "{family} is still starting; nothing was sent. Try again once logged in."
                    ),
                ));
                return Claimed::Done;
            }
            // The player's own scripts come last: a word of Hydra's, or of a
            // family still starting, is never a script's (`plan/46` §10,
            // question 5).
            if let Some(scripts) = routes.scripts.get()
                && scripts(line).is_some()
            {
                return Claimed::Done;
            }
            Claimed::Unknown
        });
        if !handle.set_desk(Desk::new(None, runner)) {
            eprintln!("  !! [commands] something already runs this session's commands");
        }
        commands
    }

    /// The family that knows `line` (without its symbol) runs it, and says
    /// what it did; `None` when no family knows the word. Each family
    /// answers `Some` for its own words and `None` for the rest, so the
    /// first to answer has the line.
    pub(crate) fn route(&self, line: &str) -> Option<Took> {
        for family in [&self.travel, &self.hunt, &self.batch, &self.relay] {
            if let Some(starter) = family.get()
                && let Some(took) = starter(line)
            {
                return Some(took);
            }
        }
        for family in [
            &self.loot,
            &self.combat,
            &self.history,
            &self.sorter,
            &self.trigger,
            &self.agent,
            &self.lich,
        ] {
            if let Some(handler) = family.get()
                && handler(line).is_some()
            {
                return Some(Took::Done);
            }
        }
        None
    }

    /// `;stop` stops `name`'s work with `stopper` from now on.
    pub(crate) fn stops(&self, name: &'static str, stopper: Stopper) {
        self.stoppers
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .push((name, stopper));
    }

    /// Stop every family's work; the names of those that had some.
    fn stop(&self) -> Vec<&'static str> {
        self.stoppers
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .iter()
            .filter(|(_, stopper)| stopper())
            .map(|(name, _)| *name)
            .collect()
    }

    /// Route travel's words to `handler` from now on. Once: a second call is
    /// ignored, as the session's own `set_desk` is.
    pub(crate) fn travel(&self, handler: Starter) {
        if self.travel.set(handler).is_err() {
            eprintln!("  !! [commands] travel was registered twice; keeping the first");
        }
    }

    /// Route hunt's words to `handler` from now on. Once, as for travel.
    pub(crate) fn hunt(&self, handler: Starter) {
        if self.hunt.set(handler).is_err() {
            eprintln!("  !! [commands] hunt was registered twice; keeping the first");
        }
    }

    /// Route `;loot` to `handler` from now on. Once, as for travel.
    pub(crate) fn loot(&self, handler: Handler) {
        if self.loot.set(handler).is_err() {
            eprintln!("  !! [commands] loot was registered twice; keeping the first");
        }
    }

    /// Route `;combat` to `handler` from now on. Once, as for travel.
    pub(crate) fn combat(&self, handler: Handler) {
        if self.combat.set(handler).is_err() {
            eprintln!("  !! [commands] combat was registered twice; keeping the first");
        }
    }

    /// Route `;history` to `handler` from now on. Once, as for travel.
    pub(crate) fn history(&self, handler: Handler) {
        if self.history.set(handler).is_err() {
            eprintln!("  !! [commands] history was registered twice; keeping the first");
        }
    }

    /// Route `;sorter` to `handler` from now on. Once, as for travel.
    pub(crate) fn sorter(&self, handler: Handler) {
        if self.sorter.set(handler).is_err() {
            eprintln!("  !! [commands] sorter was registered twice; keeping the first");
        }
    }

    /// Route `;agent` to `handler` from now on. Once, as for travel.
    pub(crate) fn agent(&self, handler: Handler) {
        if self.agent.set(handler).is_err() {
            eprintln!("  !! [commands] agent was registered twice; keeping the first");
        }
    }

    /// Route `;lich` to `handler` from now on. Once, as for travel.
    pub(crate) fn lich(&self, handler: Handler) {
        if self.lich.set(handler).is_err() {
            eprintln!("  !! [commands] lich was registered twice; keeping the first");
        }
    }

    /// Route `;trigger` to `handler` from now on. Once, as for travel.
    pub(crate) fn trigger(&self, handler: Handler) {
        if self.trigger.set(handler).is_err() {
            eprintln!("  !! [commands] trigger was registered twice; keeping the first");
        }
    }

    /// Route `;to` and `;all` to `handler` from now on. Once, as for travel.
    pub(crate) fn relay(&self, handler: Starter) {
        if self.relay.set(handler).is_err() {
            eprintln!("  !! [commands] relay was registered twice; keeping the first");
        }
    }

    /// Route the player's scripts to `handler` from now on, after every
    /// other word. Once, as for travel.
    pub(crate) fn scripts(&self, handler: Handler) {
        if self.scripts.set(handler).is_err() {
            eprintln!("  !! [commands] scripts were registered twice; keeping the first");
        }
    }

    /// Route `;multi` and `;foreach` to `handler` from now on. Once, as for
    /// travel.
    pub(crate) fn batch(&self, handler: Starter) {
        if self.batch.set(handler).is_err() {
            eprintln!("  !! [commands] batch was registered twice; keeping the first");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use cena_platform::AnsweringSource;
    use cena_session::{Event, Outcome, Session};
    use std::time::Duration;

    const DEADLINE: Duration = Duration::from_secs(5);

    /// What the player was told, as text.
    fn told(events: &mut tokio::sync::broadcast::Receiver<Event>) -> Vec<String> {
        let mut said = Vec::new();
        while let Ok(event) = events.try_recv() {
            if let Event::Notice(notice) = event {
                said.push(format!("{:?}", notice.body));
            }
        }
        said
    }

    /// The author's live run: `;go2 bank` reached the game because nothing
    /// had claimed `;`. Installed at startup, it cannot: before travel is
    /// ready, after it, and for a word nobody knows.
    #[tokio::test(flavor = "current_thread", start_paused = true)]
    async fn nothing_marked_as_hydras_reaches_the_game() {
        let (source, transcript) = AnsweringSource::new(b"<prompt time=\"1\">&gt;</prompt>\n");
        let session = Session::new(source);
        let handle = session.handle();
        let (_, mut events) = session.subscribe();
        let generation = handle.generation();
        let commands = Commands::install(&handle);
        tokio::spawn(session.into_actor().run());

        // Before travel registers: its word is told to wait.
        let early = handle
            .send_manual_at(generation, ".go2 bank", DEADLINE)
            .await;
        assert_eq!(early, Outcome::Handled);
        assert!(
            told(&mut events)
                .iter()
                .any(|s| s.contains("still starting")),
            "a travel command before travel is ready is answered"
        );

        // A word nobody knows, before and after.
        assert_eq!(
            handle.send_manual_at(generation, ".nosuch", DEADLINE).await,
            Outcome::Handled
        );
        assert!(
            told(&mut events).iter().any(|s| s.contains(".help")),
            "a word nobody knows points at .help"
        );

        // Help, before anything else is ready, and nothing sent.
        assert_eq!(
            handle.send_manual_at(generation, ".help", DEADLINE).await,
            Outcome::Handled
        );
        assert!(
            told(&mut events).iter().any(|s| s.contains("hunt help")),
            ".help lists the families"
        );

        commands.travel(Arc::new(|line: &str| {
            line.starts_with("go2").then_some(Took::Done)
        }));
        assert_eq!(
            handle
                .send_manual_at(generation, ".go2 bank", DEADLINE)
                .await,
            Outcome::Handled
        );
        assert_eq!(
            handle.send_manual_at(generation, ".nosuch", DEADLINE).await,
            Outcome::Handled
        );

        // The game's own lines still go to the game.
        handle.send_manual_at(generation, "look", DEADLINE).await;
        assert_eq!(
            transcript.lines(),
            ["look"],
            "only the game's line was sent"
        );
    }

    /// `;stop` stops every family that has something running, says which,
    /// and says so when nothing was; the game hears none of it.
    #[tokio::test(flavor = "current_thread", start_paused = true)]
    async fn stop_stops_whatever_is_running_and_says_what() {
        let (source, transcript) = AnsweringSource::new(
            b"<prompt time=\"1\">&gt;</prompt>
",
        );
        let session = Session::new(source);
        let handle = session.handle();
        let (_, mut events) = session.subscribe();
        let generation = handle.generation();
        let commands = Commands::install(&handle);
        tokio::spawn(session.into_actor().run());
        let hunting = Arc::new(std::sync::atomic::AtomicBool::new(true));
        let running = Arc::clone(&hunting);
        commands.stops(
            "hunt",
            Arc::new(move || running.swap(false, std::sync::atomic::Ordering::Relaxed)),
        );
        commands.stops("go2", Arc::new(|| false));

        for expected in ["Stopped: hunt.", "Nothing was running."] {
            assert_eq!(
                handle.send_manual_at(generation, ".stop", DEADLINE).await,
                Outcome::Handled
            );
            let said = told(&mut events);
            assert!(said.iter().any(|s| s.contains(expected)), "{said:?}");
        }
        assert!(transcript.lines().is_empty(), "nothing reached the game");
    }
}
