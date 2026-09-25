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

use cena_session::command::claimant::{Claimed, Desk, Runner};
use cena_session::{Notice, NoticeKind, SessionHandle};
use std::sync::{Arc, OnceLock};

/// A family of commands: `Some` when the line was its own, `None` when the
/// word is not one it knows.
pub(crate) type Handler = Arc<dyn Fn(&str) -> Option<Claimed> + Send + Sync>;

/// The handlers the command line routes to, filled as each becomes ready.
#[derive(Clone, Default)]
pub(crate) struct Commands {
    travel: Arc<OnceLock<Handler>>,
    hunt: Arc<OnceLock<Handler>>,
    loot: Arc<OnceLock<Handler>>,
}

impl Commands {
    /// Put the command line on `handle`, with the default symbol until the
    /// character's own is known (`SessionHandle::set_command_symbol`).
    pub(crate) fn install(handle: &SessionHandle) -> Self {
        let commands = Self::default();
        let travel = Arc::clone(&commands.travel);
        let hunt = Arc::clone(&commands.hunt);
        let loot = Arc::clone(&commands.loot);
        let told = handle.clone();
        let runner: Runner = Arc::new(move |line: &str| {
            // Each family answers `Some` for its own words and `None` for
            // the rest; the first to answer has the line.
            for family in [&travel, &hunt, &loot] {
                if let Some(handler) = family.get()
                    && let Some(claimed) = handler(line)
                {
                    return claimed;
                }
            }
            let starting = if cena_behavior::travel::parse_command(line).is_some() {
                Some("Travel")
            } else if cena_behavior::hunt::parse_command(line).is_some() {
                Some("Hunt")
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
            Claimed::Unknown
        });
        if !handle.set_desk(Desk::new(None, runner)) {
            eprintln!("  !! [commands] something already runs this session's commands");
        }
        commands
    }

    /// Route travel's words to `handler` from now on. Once: a second call is
    /// ignored, as the session's own `set_desk` is.
    pub(crate) fn travel(&self, handler: Handler) {
        if self.travel.set(handler).is_err() {
            eprintln!("  !! [commands] travel was registered twice; keeping the first");
        }
    }

    /// Route hunt's words to `handler` from now on. Once, as for travel.
    pub(crate) fn hunt(&self, handler: Handler) {
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
            .send_manual_at(generation, ";go2 bank", DEADLINE)
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
            handle.send_manual_at(generation, ";nosuch", DEADLINE).await,
            Outcome::Handled
        );

        commands.travel(Arc::new(|line: &str| {
            line.starts_with("go2").then_some(Claimed::Done)
        }));
        assert_eq!(
            handle
                .send_manual_at(generation, ";go2 bank", DEADLINE)
                .await,
            Outcome::Handled
        );
        assert_eq!(
            handle.send_manual_at(generation, ";nosuch", DEADLINE).await,
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
}
