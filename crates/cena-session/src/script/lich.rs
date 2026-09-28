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
//! What the player types at a frontend
//! ([`SessionHandle::send_typed_at`]) is Hydra's first: a line with the
//! command symbol, or one a behavior takes bare (`crate::command::claimant`).
//! The rest goes to Lich instead of the game, so Lich's commands, aliases and
//! upstream hooks have it, and Lich sends what it makes of it. The author's
//! answer for the symbol both use (`plan/51` §6, question 3): *"if they're
//! running lich would probably change hydra's command character to . or
//! something"*. Hydra's own lines on the manual path (`;multi`'s, a relayed
//! `;to`) go to the game, as a Lich script's `put` never meets Lich's hooks.
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

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use tokio::sync::mpsc;

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

/// A Lich attached to a character: what it is handed.
#[derive(Debug)]
pub struct Attached {
    /// The game's bytes, as they came.
    pub wire: Wire,
    /// What the player types that Hydra does not take.
    pub typing: Typing,
}

/// The game's bytes, copied for a Lich as they arrive.
#[derive(Debug)]
pub struct Wire {
    bytes: mpsc::Receiver<Vec<u8>>,
    behind: Arc<AtomicBool>,
}

/// What the player types for a Lich, a line at a time.
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
    /// Attach a Lich from here on: the next chunk of the game's bytes is its
    /// first, and the next line the player types that Hydra does not take is
    /// its. `None` while another is attached: one per character. It stays
    /// attached until what this returns is dropped.
    #[must_use]
    pub fn attach(&self) -> Option<Attached> {
        let (bytes, wire) = mpsc::channel(WIRE_CHUNKS);
        let (typing, typed) = mpsc::channel(TYPED_LINES);
        let behind = Arc::new(AtomicBool::new(false));
        let tap = Tap {
            bytes,
            typing,
            behind: Arc::clone(&behind),
        };
        self.handle.attach_lich(tap).then_some(Attached {
            wire: Wire {
                bytes: wire,
                behind,
            },
            typing: Typing(typed),
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
    use crate::{Outcome, SessionHandle};

    /// No actor answers here, so a line sent to the game waits this long.
    const DEADLINE: Duration = Duration::from_millis(10);

    /// A character's handle, the publisher its chunks arrive through, and
    /// its actor's inbox, where a line sent to the game lands.
    fn character() -> (
        SessionHandle,
        EventPublisher,
        tokio::sync::mpsc::Receiver<Inbox>,
    ) {
        let (sender, inbox) = tokio::sync::mpsc::channel(8);
        let generation = GenerationCell::default();
        let publisher =
            EventPublisher::from_legacy(tokio::sync::broadcast::channel(8).0, generation.clone());
        let handle = SessionHandle::publishing_to(sender, generation, publisher.clone());
        (handle, publisher, inbox)
    }

    /// The line the actor was given for the game, if any.
    fn for_the_game(inbox: &mut tokio::sync::mpsc::Receiver<Inbox>) -> Option<String> {
        match inbox.try_recv().ok()? {
            Inbox::Command(envelope) => Some(envelope.line),
            _ => None,
        }
    }

    #[tokio::test]
    async fn the_game_bytes_reach_one_lich_per_character() {
        let (handle, publisher, _inbox) = character();
        let door = handle.lich_door();
        let mut lich = door.attach().expect("the first Lich");
        assert!(
            door.attach().is_none(),
            "a second, while the first is attached"
        );
        publisher.wire(b"<prompt time=\"1\">&gt;</prompt>\n");
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
        let (handle, publisher, _inbox) = character();
        let Attached { mut wire, .. } = handle.lich_door().attach().expect("the first Lich");
        for _ in 0..=WIRE_CHUNKS {
            publisher.wire(b"x");
        }
        let mut read = 0;
        while wire.next().await.is_some() {
            read += 1;
        }
        assert_eq!(read, WIRE_CHUNKS, "what it had, and then its end");
        assert!(wire.fell_behind());
    }

    /// Hydra's commands are Hydra's; the rest of what the player types is
    /// Lich's while it runs, and the game's once it stops. Hydra's own lines
    /// go to the game.
    #[tokio::test]
    async fn what_the_player_types_goes_to_lich_after_hydra() {
        let (handle, _publisher, mut inbox) = character();
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
        let typed = handle
            .send_typed_at(generation, ";e echo 1", DEADLINE)
            .await;
        assert_eq!(typed, Outcome::Handled);
        assert_eq!(
            *ran.lock().unwrap(),
            ["go2 bank"],
            "Hydra's, with its symbol"
        );
        assert_eq!(lich.typing.next().await.as_deref(), Some(";e echo 1"));

        handle.send_manual_at(generation, "look", DEADLINE).await;
        assert_eq!(
            for_the_game(&mut inbox).as_deref(),
            Some("look"),
            "Hydra's own line"
        );

        drop(lich);
        handle.send_typed_at(generation, "exp", DEADLINE).await;
        assert_eq!(
            for_the_game(&mut inbox).as_deref(),
            Some("exp"),
            "Lich stopped"
        );
    }
}
