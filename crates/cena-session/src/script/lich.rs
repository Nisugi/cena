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
//! The symbol both use (`plan/51` §6, question 3): *"if they're running lich
//! would probably change hydra's command character to . or something"*; until
//! then the line is Hydra's, and Lich's commands are out of reach.
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
//! (`LichText`).

use std::collections::VecDeque;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, PoisonError};
use std::time::Duration;

use cena_model::line::{Line, Unfinished};
use cena_protocol::{Frame, Parser};
use tokio::sync::mpsc;
use tokio::time::Instant;

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

/// How long a line of a quiet command's report waits to be left out of
/// Lich's text. Lich passes it on as it came, after whatever it was busy
/// with: at a login, its own scripts starting. A line Lich hid or changed is
/// not waited for past this, and the report's next lines are shown.
pub const QUIET_LAG: Duration = Duration::from_secs(30);

/// How many lines of quiet reports wait to be left out of Lich's text.
const QUIET_LINES: usize = 1_024;

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

/// What a Lich shows a frontend, its standard output as it wrote it,
/// carried to the session to be the character's text.
#[derive(Debug)]
pub struct Shown(mpsc::Sender<Vec<u8>>);

impl Shown {
    /// Carry `chunk`. False when it is not shown: the session has
    /// [`WIRE_CHUNKS`] unread, as when no connection has read it for a
    /// while, or it is gone.
    #[must_use]
    pub fn show(&self, chunk: Vec<u8>) -> bool {
        self.0.try_send(chunk).is_ok()
    }
}

/// What a Lich shows, read by the session: its bytes as they come, parsed
/// by a parser of their own and put together into lines by the model's own
/// [`Unfinished`], so a line of it ends where the game's would.
///
/// The session's, not a connection's: it waits between connections in the
/// session's publisher ([`Parked`]), and each connection's actor takes it.
#[derive(Debug)]
pub(crate) struct LichText {
    chunks: mpsc::Receiver<Vec<u8>>,
    parser: Parser,
    lines: Unfinished,
    /// The main stream's lines of Hydra's quiet commands' reports, each with
    /// when it came: left out of Lich's text as Lich passes them on.
    quiet: VecDeque<(Instant, String)>,
}

/// What a Lich showed, in the order it showed it.
#[derive(Debug)]
pub(crate) enum Showing {
    /// A finished line.
    Line(Line),
    /// The game's prompt, as Lich passed it on.
    Prompt(String),
}

impl LichText {
    fn new(chunks: mpsc::Receiver<Vec<u8>>) -> Self {
        Self {
            chunks,
            parser: Parser::new(),
            lines: Unfinished::default(),
            quiet: VecDeque::new(),
        }
    }

    /// The next chunk Lich wrote; `None` once it has stopped.
    pub(crate) async fn next(&mut self) -> Option<Vec<u8>> {
        self.chunks.recv().await
    }

    /// The lines `chunk` finishes, and its prompts, in order.
    pub(crate) fn read(&mut self, chunk: &[u8]) -> Vec<Showing> {
        let mut showing = Vec::new();
        for frame in self.parser.push_bytes(chunk) {
            match frame {
                Frame::Text(text) => {
                    if let Some(runs) = self.lines.push(&text) {
                        showing.push(Showing::Line(Line::new(text.stream.clone(), runs)));
                    }
                }
                Frame::ClearStream { id } => self.lines.clear_stream(&id),
                Frame::Prompt { text, .. } => showing.push(Showing::Prompt(text)),
                _ => {}
            }
        }
        showing
    }

    /// A line of main in a quiet command's report, `text`: to be left out
    /// when Lich passes it on.
    pub(crate) fn expect_quiet(&mut self, text: String) {
        if self.quiet.len() == QUIET_LINES {
            self.quiet.pop_front();
        }
        self.quiet.push_back((Instant::now(), text));
    }

    /// Whether a line of main that Lich showed, `text`, is the next line of
    /// a quiet report, and so left out. Only the next: the report's lines
    /// come back in the order the game sent them, and a line of Lich's own
    /// that says the same as one further on is still shown.
    pub(crate) fn was_quiet(&mut self, text: &str) -> bool {
        let now = Instant::now();
        while self
            .quiet
            .front()
            .is_some_and(|(came, _)| now.duration_since(*came) > QUIET_LAG)
        {
            self.quiet.pop_front();
        }
        let next = self.quiet.front().is_some_and(|(_, next)| next == text);
        if next {
            self.quiet.pop_front();
        }
        next
    }
}

/// Where a Lich's text waits for the actor that shows it: a new Lich's
/// until the actor's next turn, and the one a connection's actor showed
/// until the next connection's.
#[derive(Debug, Default)]
pub(crate) struct Parked {
    text: Mutex<Option<LichText>>,
    /// Something is parked. Read on the actor's every turn, so the lock is
    /// taken only when there is.
    waiting: AtomicBool,
}

impl Parked {
    /// A new Lich's text. What was parked was a Lich's that is gone.
    pub(crate) fn park(&self, text: LichText) {
        let mut parked = self.text.lock().unwrap_or_else(PoisonError::into_inner);
        *parked = Some(text);
        self.waiting.store(true, Ordering::Release);
    }

    /// What an ending connection's actor showed, for the next connection's:
    /// unless a newer Lich's is waiting.
    pub(crate) fn put_back(&self, text: LichText) {
        let mut parked = self.text.lock().unwrap_or_else(PoisonError::into_inner);
        if parked.is_none() {
            *parked = Some(text);
            self.waiting.store(true, Ordering::Release);
        }
    }

    /// What waits, if anything.
    pub(crate) fn take(&self) -> Option<LichText> {
        if !self.waiting.swap(false, Ordering::Acquire) {
            return None;
        }
        self.text
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .take()
    }
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
    /// first, the next line typed for it is its, and what it shows is the
    /// character's text in place of the game's from that chunk on. `None`
    /// while another is attached: one per character. It stays attached until
    /// what this returns is dropped.
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
        around.publisher.wire(b"<prompt time=\"1\">&gt;</prompt>\n");
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
