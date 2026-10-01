//! The player's own Lich, through the relay (`plan/51`): what it may do to
//! the character it runs for, through [`LichDoor`], which `cena-agent`'s
//! relay holds and never the handle.
//!
//! Hydra keeps the game's connection. Lich, started in pipe mode and
//! pointed at a port the relay holds, takes the relay as its game. Once
//! [attached](LichDoor::attach), it is handed the game's bytes as they came,
//! and what the player types that Hydra does not take; what it writes to
//! that port comes here as lines to send ([`LichDoor::send`]).
//!
//! # The bytes, not the lines
//!
//! Lich parses the stream itself, markup and all, so it is handed exactly
//! what the game sent, as the recorder is: copied where a chunk arrives,
//! before the parser, and read by nothing here. Nothing above
//! `cena-protocol` reads a raw byte (Rule 2.1); these are carried, as the
//! socket's are.
//!
//! # The player's typing
//!
//! The symbol a line starts with says whose it is (the author, 2026-09-28):
//! *"command starting with the lich command character ; get sent to lich.
//! commands sent with the hydra command character . get sent to hydra."* And
//! the rest of what the player types goes to Lich too, as it does in Lich,
//! for its aliases and hooks to have it: *"damn I guess commands have to go
//! to lich then"*.
//!
//! So on the manual path ([`SessionHandle::send_manual_at`]), a line is
//! Hydra's first: its symbol, or a behavior's taking it bare
//! (`crate::command::claimant`). Then:
//!
//! - a line with [`LICH_SYMBOL`] is Lich's, handed to its standard input,
//!   and never the game's, as a line with Hydra's symbol is never the
//!   game's: with no Lich running, the player is told;
//! - what the player typed ([`SessionHandle::send_typed_at`]) is Lich's while
//!   one runs, and Lich sends what it makes of it; with none, the game's;
//! - Hydra's own lines (`;multi`'s, a relayed `;to`) go to the game, past
//!   Lich's hooks, as a Lich script's `put` never meets them: an alias
//!   turning a line into a `;multi` of itself would otherwise never end.
//!
//! Hydra's symbol starts as `.` ([`DEFAULT_SYMBOL`], the author's default
//! since 2026-09-29) and Lich's is `;`, so each reaches its own. Hydra's is
//! tried first: a player who sets Hydra's to `;` puts Lich's commands out of
//! reach, and the relay warns of it as Lich starts (`plan/51` §6, question 3:
//! *"if they're running lich would probably change hydra's command character
//! to . or something"*).
//!
//! # Kept up, and let go
//!
//! The Lich attached is the session's, not a connection's, so it stays up
//! through a reconnect and sees the new login as more of the stream (the
//! author, 2026-09-28: *"we can keep lich running sure, but it's scripts
//! aren't gonna magically keep working with no game connection."*). While
//! there is no connection its lines are not sent, and the relay says so.
//!
//! A Lich that stops reading is let go rather than waited on, or fed a
//! stream with a hole in it: the copy holds [`WIRE_CHUNKS`] chunks, and
//! the first that finds it full ends it ([`Wire::fell_behind`]). That is
//! Lich's own rule for its own queue (`reference/lich-5/lib/games.rb:385`):
//! *"Dropping records or blocking the socket reader would both make
//! recovery unsafe."*
//!
//! # What it shows
//!
//! What Lich would show a frontend, its standard output, is carried back
//! ([`Shown`]) and is the character's text while it runs (`plan/51` §6,
//! question 1): the session parses it and shows it in place of the game's,
//! so Lich's squelches and its scripts' messages show as they would in any
//! frontend. The session's own parse of the game is still everything else:
//! the model, the log, a script runner's lines, what the triggers do.
//! Carried as bytes, as the game's are, and read by a parser of their own
//! (`text.rs`).
//!
//! # Started late
//!
//! A Lich attached after the login is handed a login first, built from what
//! the session knows as it attaches (`GameState::login`); then the live
//! stream. The author, 2026-09-28: *"We know what the login blob consists
//! of, so we can just build it in the moment and send accurate info."* The
//! session's actor builds it, from its model, between one chunk and the
//! next, so the Lich misses nothing and is told nothing twice.

mod text;

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use tokio::sync::mpsc;

pub use text::Shown;
pub(crate) use text::{LichText, Parked, Showing};

use super::Sending;
use crate::command::Claimed;
use crate::command::claimant::DEFAULT_SYMBOL;
use crate::{Gate, Notice, Origin, Sent, SessionHandle};

/// How many chunks of the game's bytes wait for a Lich that has not read
/// them, as many records as Lich's own queue holds
/// (`reference/lich-5/lib/games.rb:385`). They also hold the login while a
/// Lich starts, which takes seconds (`plan/51` §3).
pub const WIRE_CHUNKS: usize = 4_096;

/// How many typed lines wait for a Lich: those typed while it starts. Past
/// them the player's line is refused, to be typed again.
pub const TYPED_LINES: usize = 64;

/// What starts a line for the player's Lich: Lich's own `$lich_char`. A
/// player who changed Lich's changes nothing here yet.
pub const LICH_SYMBOL: char = ';';

/// The only way the player's Lich acts on a session.
///
/// Built by [`SessionHandle::lich_door`], for `cena-agent`, which holds this
/// and never the handle.
#[derive(Clone, Debug)]
pub struct LichDoor {
    handle: SessionHandle,
}

/// Who began a line Lich sends.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LineFrom {
    /// One of Lich's scripts put it: [`Origin::Lich`].
    Lich,
    /// The player typed it, and Lich passed it on, after its own hooks:
    /// [`Origin::Manual`], and the player is there.
    Player,
}

/// A Lich attached to a character: what it is handed, and where what it
/// shows goes.
#[derive(Debug)]
pub struct Attached {
    /// The game's bytes, as they came.
    pub wire: Wire,
    /// What the player types that Hydra does not take, and Hydra's own
    /// lines with [`LICH_SYMBOL`].
    pub typing: Typing,
    /// Where what Lich shows a frontend goes: the character's text.
    pub shown: Shown,
}

/// The game's bytes, copied for a Lich as they arrive.
#[derive(Debug)]
pub struct Wire {
    bytes: mpsc::Receiver<Vec<u8>>,
    behind: Arc<AtomicBool>,
}

/// The lines for a Lich's standard input, one at a time.
#[derive(Debug)]
pub struct Typing(mpsc::Receiver<String>);

/// The session's end of an [`Attached`].
#[derive(Debug)]
pub(crate) struct Tap {
    bytes: mpsc::Sender<Vec<u8>>,
    typing: mpsc::Sender<String>,
    behind: Arc<AtomicBool>,
}

impl Tap {
    /// Whether a Lich is still attached.
    pub(crate) fn is_open(&self) -> bool {
        !self.bytes.is_closed()
    }

    /// Copy `chunk`. False when the copy ends here: the Lich stopped taking
    /// it, or has [`WIRE_CHUNKS`] unread.
    pub(crate) fn copy(&self, chunk: &[u8]) -> bool {
        match self.bytes.try_send(chunk.to_vec()) {
            Ok(()) => true,
            Err(mpsc::error::TrySendError::Full(_)) => {
                self.behind.store(true, Ordering::Relaxed);
                false
            }
            Err(mpsc::error::TrySendError::Closed(_)) => false,
        }
    }

    /// Hand the Lich a line the player typed. False when it has
    /// [`TYPED_LINES`] waiting.
    pub(crate) fn hand(&self, line: &str) -> bool {
        self.typing.try_send(line.to_owned()).is_ok()
    }
}

/// The player's Lich as the session holds it: the one attached, and one
/// attached since the actor's last turn, waiting to be handed its login.
#[derive(Debug, Default)]
pub(crate) struct Slot {
    tap: Option<Tap>,
    pending: Option<Tap>,
}

impl Slot {
    /// Attach a Lich, to be handed its login at the actor's next turn. False,
    /// and `tap` unused, while another is attached: one per character.
    pub(crate) fn attach(&mut self, tap: Tap) -> bool {
        if self.attached() {
            return false;
        }
        self.pending = Some(tap);
        true
    }

    /// Hand the Lich attached since the actor's last turn `login`, and copy
    /// the game's bytes to it from now on. False with none waiting.
    pub(crate) fn begin(&mut self, login: &[u8]) -> bool {
        let Some(tap) = self.pending.take() else {
            return false;
        };
        if !login.is_empty() {
            tap.copy(login);
        }
        self.tap = Some(tap);
        true
    }

    /// Whether a Lich is attached, or waits to be handed its login.
    pub(crate) fn attached(&self) -> bool {
        self.tap.iter().chain(&self.pending).any(Tap::is_open)
    }

    /// Copy a chunk of the game's bytes to the Lich attached: whether one
    /// took it. One that stopped, or fell behind, is let go here.
    pub(crate) fn wire(&mut self, chunk: &[u8]) -> bool {
        let Some(tap) = &self.tap else {
            return false;
        };
        let took = tap.copy(chunk);
        if !took {
            self.tap = None;
        }
        took
    }

    /// Hand a typed line to the Lich attached: `None` with none attached,
    /// and `Some(false)` when it has too many waiting.
    pub(crate) fn hand(&self, line: &str) -> Option<bool> {
        self.tap
            .iter()
            .chain(&self.pending)
            .find(|tap| tap.is_open())
            .map(|tap| tap.hand(line))
    }
}

impl Wire {
    /// The next chunk, as the game sent it. `None` once the copy has ended:
    /// the session is gone, or the Lich [fell behind](Self::fell_behind).
    pub async fn next(&mut self) -> Option<Vec<u8>> {
        self.bytes.recv().await
    }

    /// Whether the copy ended because the Lich had [`WIRE_CHUNKS`] unread.
    #[must_use]
    pub fn fell_behind(&self) -> bool {
        self.behind.load(Ordering::Relaxed)
    }
}

impl Typing {
    /// The next line the player typed for Lich. `None` once the session is
    /// gone.
    pub async fn next(&mut self) -> Option<String> {
        self.0.recv().await
    }
}

impl SessionHandle {
    /// The door the player's Lich acts on this session through.
    #[must_use]
    pub fn lich_door(&self) -> LichDoor {
        LichDoor {
            handle: self.clone(),
        }
    }
}

impl LichDoor {
    /// Attach a Lich from here on. Before the next chunk of the game's bytes
    /// it is handed a login built from what the session knows then, which
    /// the character's text does not show again: nothing, before the game
    /// has named the character, when the login itself is still to come. Then
    /// that chunk and those after it, the next line typed for it, and what
    /// it shows is the character's text in place of the game's. `None` while
    /// another is attached: one per character. It stays attached until what
    /// this returns is dropped.
    #[must_use]
    pub fn attach(&self) -> Option<Attached> {
        let (bytes, wire) = mpsc::channel(WIRE_CHUNKS);
        let (typing, typed) = mpsc::channel(TYPED_LINES);
        let (text, text_read) = mpsc::channel(WIRE_CHUNKS);
        let behind = Arc::new(AtomicBool::new(false));
        let tap = Tap {
            bytes,
            typing,
            behind: Arc::clone(&behind),
        };
        self.handle
            .attach_lich(tap, LichText::new(text_read))
            .then_some(Attached {
                wire: Wire {
                    bytes: wire,
                    behind,
                },
                typing: Typing(typed),
                shown: Shown(text),
            })
    }

    /// What marks a line the player types as Hydra's: Lich's `;` unless
    /// the player chose another.
    #[must_use]
    pub fn symbol(&self) -> char {
        self.handle.command_symbol().unwrap_or(DEFAULT_SYMBOL)
    }

    /// Send `line`, which Lich wrote to its game: Hydra's own command when it
    /// starts with the command symbol, as a script's line is
    /// ([`super::Door::send`]); otherwise to the game at once, with no
    /// roundtime gate and no queue, as `Game.puts` writes straight to the
    /// socket.
    pub async fn send(&self, line: &str, from: LineFrom) -> Sending {
        let origin = match from {
            LineFrom::Lich => Origin::Lich,
            LineFrom::Player => Origin::Manual,
        };
        match self.handle.typed(line) {
            Some(Claimed::Unknown) => Sending::Unknown,
            Some(_) => Sending::Ran,
            None => match self.handle.send_now(line, origin, Gate::None).await {
                Sent::Ok { cursor, .. } => Sending::Sent { cursor },
                Sent::Refused(refusal) => Sending::Refused(refusal),
                Sent::Dead | Sent::Interrupted => Sending::Lost,
            },
        }
    }

    /// Tell the player something.
    pub fn say(&self, notice: Notice) {
        self.handle.say(notice);
    }
}

#[cfg(test)]
mod tests {
    use std::sync::{Arc, Mutex};
    use std::time::Duration;

    use super::{Attached, WIRE_CHUNKS};
    use crate::command::Inbox;
    use crate::command::claimant::{Claimed, Desk};
    use crate::lifecycle::GenerationCell;
    use crate::observation::EventPublisher;
    use crate::{Event, Outcome, SessionHandle};

    /// No actor answers here, so a line sent to the game waits this long.
    const DEADLINE: Duration = Duration::from_millis(10);

    /// Around a character with no actor: where its chunks arrive, what the
    /// actor is given for the game, and what the player is told.
    struct Around {
        publisher: EventPublisher,
        inbox: tokio::sync::mpsc::Receiver<Inbox>,
        told: tokio::sync::broadcast::Receiver<Event>,
    }

    fn character() -> (SessionHandle, Around) {
        let (sender, inbox) = tokio::sync::mpsc::channel(8);
        let generation = GenerationCell::default();
        let (events, told) = tokio::sync::broadcast::channel(8);
        let publisher = EventPublisher::from_legacy(events, generation.clone());
        let handle = SessionHandle::publishing_to(sender, generation, publisher.clone());
        (
            handle,
            Around {
                publisher,
                inbox,
                told,
            },
        )
    }

    impl Around {
        /// The line the actor was given for the game, if any.
        fn for_the_game(&mut self) -> Option<String> {
            match self.inbox.try_recv().ok()? {
                Inbox::Command(envelope) => Some(envelope.line),
                // A typed line is written at once.
                Inbox::SendNow { line, .. } => Some(line),
                _ => None,
            }
        }

        /// What the player was last told, if anything.
        fn told(&mut self) -> Option<String> {
            let mut last = None;
            while let Ok(event) = self.told.try_recv() {
                if let Event::Notice(notice) = event {
                    last = notice.lines().first().cloned();
                }
            }
            last
        }
    }

    #[tokio::test]
    async fn the_game_bytes_reach_one_lich_per_character() {
        let (handle, around) = character();
        let door = handle.lich_door();
        let mut lich = door.attach().expect("the first Lich");
        assert!(
            door.attach().is_none(),
            "a second, while the first is attached"
        );
        around.publisher.wire(b"missed");
        // The actor's next turn: it is handed its login, then the game.
        around.publisher.begin_lich(b"<app char=\"Tester\"/>\n");
        around.publisher.wire(b"<prompt time=\"1\">&gt;</prompt>\n");
        assert_eq!(
            lich.wire.next().await.as_deref(),
            Some(&b"<app char=\"Tester\"/>\n"[..]),
            "its login first, and nothing from before it was handed one"
        );
        assert_eq!(
            lich.wire.next().await.as_deref(),
            Some(&b"<prompt time=\"1\">&gt;</prompt>\n"[..]),
            "as it came"
        );
        drop(lich);
        assert!(door.attach().is_some(), "another, once the first stopped");
    }

    #[tokio::test]
    async fn a_lich_that_falls_behind_is_let_go_not_waited_on() {
        let (handle, around) = character();
        let Attached { mut wire, .. } = handle.lich_door().attach().expect("the first Lich");
        around.publisher.begin_lich(b"");
        for _ in 0..=WIRE_CHUNKS {
            around.publisher.wire(b"x");
        }
        let mut read = 0;
        while wire.next().await.is_some() {
            read += 1;
        }
        assert_eq!(read, WIRE_CHUNKS, "what it had, and then its end");
        assert!(wire.fell_behind());
    }

    /// `.` is Hydra's; what else the player types is Lich's while it runs;
    /// Hydra's own lines reach Lich only with `;`. With no Lich, a `;` line
    /// goes nowhere and the player is told, and the rest goes to the game.
    #[tokio::test]
    async fn the_symbol_and_the_typist_say_whose_a_line_is() {
        let (handle, mut around) = character();
        let ran = Arc::new(Mutex::new(Vec::<String>::new()));
        let running = Arc::clone(&ran);
        let desk = Desk::new(
            Some('.'),
            Arc::new(move |line: &str| {
                running.lock().unwrap().push(line.to_owned());
                Claimed::Done
            }),
        );
        assert!(handle.set_desk(desk));
        let generation = handle.generation();
        let mut lich = handle.lich_door().attach().expect("the first Lich");

        let typed = handle
            .send_typed_at(generation, ".go2 bank", DEADLINE)
            .await;
        assert_eq!(typed, Outcome::Handled);
        assert_eq!(*ran.lock().unwrap(), ["go2 bank"], "Hydra's");
        for line in [";e echo 1", "gg"] {
            let typed = handle.send_typed_at(generation, line, DEADLINE).await;
            assert_eq!(typed, Outcome::Handled);
            assert_eq!(lich.typing.next().await.as_deref(), Some(line));
        }
        assert_eq!(around.for_the_game(), None, "both Lich's");
        // Hydra's own lines: `;` for Lich (a relayed `.to Name ;go2 bank`),
        // the rest past it.
        handle
            .send_manual_at(generation, ";go2 bank", DEADLINE)
            .await;
        assert_eq!(lich.typing.next().await.as_deref(), Some(";go2 bank"));
        handle.send_manual_at(generation, "look", DEADLINE).await;
        assert_eq!(around.for_the_game().as_deref(), Some("look"));

        drop(lich);
        let typed = handle
            .send_typed_at(generation, ";e echo 2", DEADLINE)
            .await;
        assert_eq!(typed, Outcome::Handled);
        assert_eq!(around.for_the_game(), None, "never the game's");
        assert!(
            around
                .told()
                .is_some_and(|told| told.contains("Lich is not running")),
        );
        handle.send_typed_at(generation, "exp", DEADLINE).await;
        assert_eq!(around.for_the_game().as_deref(), Some("exp"), "no Lich");
    }
}
