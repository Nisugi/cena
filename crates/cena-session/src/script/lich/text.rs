//! What the player's Lich shows, read by the session (`plan/51` §7, step 3),
//! and what it is expected to pass on and not to show: a quiet command's
//! report, and the login a Lich started late was handed (step 4).

use std::collections::VecDeque;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Mutex, PoisonError};

use cena_model::line::{Line, Unfinished};
use cena_protocol::{Frame, Parser};
use tokio::sync::mpsc;

/// How far ahead of the next expected line or prompt one Lich showed is
/// looked for: those before it Lich hid, as a squelch does.
const LOOKAHEAD: usize = 16;

/// How many lines and prompts Lich shows, none of them expected, before
/// what is still expected is taken as hidden by Lich, and let go.
const MISSES: usize = 64;

/// The most lines and prompts expected at once.
const ECHOES: usize = 8_192;

/// What a Lich shows a frontend, its standard output as it wrote it,
/// carried to the session to be the character's text.
#[derive(Debug)]
pub struct Shown(pub(super) mpsc::Sender<Vec<u8>>);

impl Shown {
    /// Carry `chunk`. False when it is not shown: the session has
    /// [`WIRE_CHUNKS`](super::WIRE_CHUNKS) unread, as when no connection has
    /// read it for a while, or it is gone.
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
    reading: Reading,
    echoes: Echoes,
}

/// What a Lich showed, in the order it showed it.
#[derive(Debug)]
pub(crate) enum Showing {
    /// A finished line.
    Line(Line),
    /// The game's prompt, as Lich passed it on.
    Prompt(String),
}

/// A parser and the lines it has not finished.
#[derive(Debug, Default)]
struct Reading {
    parser: Parser,
    lines: Unfinished,
}

impl Reading {
    /// The lines `chunk` finishes, and its prompts, in order.
    fn read(&mut self, chunk: &[u8]) -> Vec<Showing> {
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
}

impl LichText {
    pub(super) fn new(chunks: mpsc::Receiver<Vec<u8>>) -> Self {
        Self {
            chunks,
            reading: Reading::default(),
            echoes: Echoes::default(),
        }
    }

    /// The next chunk Lich wrote; `None` once it has stopped.
    pub(crate) async fn next(&mut self) -> Option<Vec<u8>> {
        self.chunks.recv().await
    }

    /// The lines `chunk` finishes, and its prompts, in order.
    pub(crate) fn read(&mut self, chunk: &[u8]) -> Vec<Showing> {
        self.reading.read(chunk)
    }

    /// A line the game sent, of a quiet command's report: to be left out
    /// when Lich passes it on.
    pub(crate) fn expect_quiet(&mut self, line: &Line) {
        self.echoes
            .expect(Echo::Line(line.stream.clone(), line.text()));
    }

    /// The login a Lich started late is handed, built from what the session
    /// knows: every line and prompt of it to be left out when Lich passes it
    /// on, since the character's text already showed what it tells.
    pub(crate) fn expect_login(&mut self, login: &[u8]) {
        for showing in Reading::default().read(login) {
            self.echoes.expect(Echo::of(&showing));
        }
    }

    /// Whether `showing` is what Lich was expected to pass on and not to
    /// show, and so left out.
    pub(crate) fn echoes(&mut self, showing: &Showing) -> bool {
        self.echoes.echoes(showing)
    }
}

/// A line or prompt expected back from Lich: a line by its stream and text.
#[derive(Debug, PartialEq, Eq)]
enum Echo {
    Line(String, String),
    Prompt(String),
}

impl Echo {
    fn of(showing: &Showing) -> Self {
        match showing {
            Showing::Line(line) => Self::Line(line.stream.clone(), line.text()),
            Showing::Prompt(text) => Self::Prompt(text.clone()),
        }
    }

    fn is_blank(&self) -> bool {
        match self {
            Self::Line(_, text) | Self::Prompt(text) => text.trim().is_empty(),
        }
    }
}

/// What Lich is expected to pass on and not to show, in the order the game
/// sent it.
///
/// Lich passes the game's lines on as they came, with its own among them, in
/// order: so what Lich shows is looked for among the next few expected.
/// Those before the one found, Lich hid. A blank line is only ever the next,
/// since Lich's own blank lines would otherwise skip what it did not hide.
/// Once Lich has shown [`MISSES`] lines and prompts with none expected
/// among them, what is still expected is let go, so a line Lich hid is not
/// waited for.
#[derive(Debug, Default)]
struct Echoes {
    expected: VecDeque<Echo>,
    misses: usize,
}

impl Echoes {
    fn expect(&mut self, echo: Echo) {
        if self.expected.len() == ECHOES {
            self.expected.pop_front();
        }
        self.expected.push_back(echo);
    }

    fn echoes(&mut self, showing: &Showing) -> bool {
        if self.expected.is_empty() {
            return false;
        }
        let shown = Echo::of(showing);
        let reach = if shown.is_blank() { 1 } else { LOOKAHEAD };
        let found = self
            .expected
            .iter()
            .take(reach)
            .position(|expected| *expected == shown);
        if let Some(at) = found {
            self.expected.drain(..=at);
            self.misses = 0;
            return true;
        }
        self.misses += 1;
        if self.misses > MISSES {
            self.expected.clear();
            self.misses = 0;
        }
        false
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

#[cfg(test)]
mod tests {
    use super::{Echo, Echoes, LOOKAHEAD, MISSES, Reading, Showing};

    /// A line of main saying `text`, as Lich would show it.
    fn line(text: &str) -> Showing {
        Showing::Line(cena_model::line::Line::new(
            "",
            Reading::default()
                .read(format!("{text}\n").as_bytes())
                .into_iter()
                .find_map(|showing| match showing {
                    Showing::Line(line) => Some(line.runs),
                    Showing::Prompt(_) => None,
                })
                .unwrap_or_default(),
        ))
    }

    fn expecting(texts: &[&str]) -> Echoes {
        let mut echoes = Echoes::default();
        for text in texts {
            echoes.expect(Echo::of(&line(text)));
        }
        echoes
    }

    /// Lich's own lines among the expected ones are shown; the expected are
    /// not, and one Lich hid is passed over.
    #[test]
    fn what_lich_passes_on_is_left_out_and_its_own_is_shown() {
        let mut echoes = expecting(&["one", "two", "three"]);
        assert!(!echoes.echoes(&line("Lich's own")));
        assert!(echoes.echoes(&line("one")));
        // `two`, hidden by Lich.
        assert!(echoes.echoes(&line("three")));
        assert!(!echoes.echoes(&line("one")), "nothing still expected");
    }

    /// A blank line of Lich's own does not pass over what it did not hide.
    #[test]
    fn a_blank_line_is_only_ever_the_next() {
        let mut echoes = expecting(&["one", "", "two"]);
        assert!(!echoes.echoes(&line("")), "Lich's own blank line");
        assert!(echoes.echoes(&line("one")));
        assert!(echoes.echoes(&line("")));
        assert!(echoes.echoes(&line("two")));
    }

    /// What Lich hid wholly is let go once it has shown enough else, and
    /// is not looked for past the next few.
    #[test]
    fn what_lich_hid_is_let_go() {
        let far: Vec<String> = (0..=LOOKAHEAD).map(|n| format!("line {n}")).collect();
        let mut echoes = expecting(&far.iter().map(String::as_str).collect::<Vec<_>>());
        assert!(
            !echoes.echoes(&line(&format!("line {LOOKAHEAD}"))),
            "past the next {LOOKAHEAD}"
        );
        let mut echoes = expecting(&["hidden"]);
        for _ in 0..=MISSES {
            assert!(!echoes.echoes(&line("Lich's own")));
        }
        assert!(!echoes.echoes(&line("hidden")), "let go");
    }
}
