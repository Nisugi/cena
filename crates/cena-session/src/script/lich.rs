//! The player's own Lich, through the relay (`plan/51`): what it may do to
//! the character it runs for, through [`LichDoor`], which `cena-agent`'s
//! relay holds and never the handle.
//!
//! Hydra keeps the game's connection. Lich, started in pipe mode and
//! pointed at a port the relay holds, takes the relay as its game: it is
//! handed the game's bytes as they came ([`LichDoor::wire`]), and what it
//! writes to that port comes here as lines to send ([`LichDoor::send`]).
//!
//! # The bytes, not the lines
//!
//! Lich parses the stream itself, markup and all, so it is handed exactly
//! what the game sent, as the recorder is: copied where a chunk arrives,
//! before the parser, and read by nothing here. Nothing above
//! `cena-protocol` reads a raw byte (Rule 2.1); these are carried, as the
//! socket's are.
//!
//! # Kept up, and let go
//!
//! The copy is the session's, not a connection's, so Lich stays up through
//! a reconnect and sees the new login as more of the stream (the author,
//! 2026-09-28: *"we can keep lich running sure, but it's scripts aren't gonna
//! magically keep working with no game connection."*). While there is no
//! connection its lines are not sent, and the relay says so.
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
use crate::{Gate, Notice, Origin, Sent, SessionHandle};

/// How many chunks of the game's bytes wait for a Lich that has not read
/// them, as many records as Lich's own queue holds
/// (`reference/lich-5/lib/games.rb:385`). They also hold the login while a
/// Lich starts, which takes seconds (`plan/51` §3).
pub const WIRE_CHUNKS: usize = 4_096;

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

/// The game's bytes, copied for a Lich as they arrive.
#[derive(Debug)]
pub struct Wire {
    bytes: mpsc::Receiver<Vec<u8>>,
    behind: Arc<AtomicBool>,
}

/// The session's end of a [`Wire`].
#[derive(Debug)]
pub(crate) struct Tap {
    bytes: mpsc::Sender<Vec<u8>>,
    behind: Arc<AtomicBool>,
}

impl Tap {
    /// Whether a Lich still takes the bytes.
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
    /// Copy the game's bytes to a Lich from here on, the next chunk first.
    /// `None` while another Lich takes them: one per character.
    #[must_use]
    pub fn wire(&self) -> Option<Wire> {
        let (bytes, receiver) = mpsc::channel(WIRE_CHUNKS);
        let behind = Arc::new(AtomicBool::new(false));
        let tap = Tap {
            bytes,
            behind: Arc::clone(&behind),
        };
        self.handle.tap_wire(tap).then_some(Wire {
            bytes: receiver,
            behind,
        })
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
    use super::WIRE_CHUNKS;
    use crate::SessionHandle;
    use crate::lifecycle::GenerationCell;
    use crate::observation::EventPublisher;

    /// A character's handle, and the publisher its chunks arrive through.
    fn character() -> (SessionHandle, EventPublisher) {
        let (sender, _) = tokio::sync::mpsc::channel(1);
        let generation = GenerationCell::default();
        let publisher =
            EventPublisher::from_legacy(tokio::sync::broadcast::channel(8).0, generation.clone());
        let handle = SessionHandle::publishing_to(sender, generation, publisher.clone());
        (handle, publisher)
    }

    #[tokio::test]
    async fn the_game_bytes_reach_one_lich_per_character() {
        let (handle, publisher) = character();
        let door = handle.lich_door();
        let mut wire = door.wire().expect("the first Lich");
        assert!(
            door.wire().is_none(),
            "a second, while the first takes them"
        );
        publisher.wire(b"<prompt time=\"1\">&gt;</prompt>\n");
        assert_eq!(
            wire.next().await.as_deref(),
            Some(&b"<prompt time=\"1\">&gt;</prompt>\n"[..]),
            "as it came"
        );
        drop(wire);
        assert!(door.wire().is_some(), "another, once the first stopped");
    }

    #[tokio::test]
    async fn a_lich_that_falls_behind_is_let_go_not_waited_on() {
        let (handle, publisher) = character();
        let mut wire = handle.lich_door().wire().expect("the first Lich");
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
}
