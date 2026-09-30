//! One character's story as its play window shows it (`plan/47` step 4):
//! the game's lines, its prompts, what the player typed, Hydra's own
//! messages in their own pane, and a trigger's banners.
//! How a prompt and an echo are shown is `prompt.rs`'s.
//!
//! The session already assembled, sorted and painted each line (`plan/45`
//! §4a); this only decides whether the story shows it. A line on a stream
//! whose window is closed -- and the play window opens none yet -- goes
//! where the game declared ([`Windows::route`](cena_session::stream_windows::Windows::route)):
//! to the story, to the story in a style, or nowhere, since some streams are
//! copies of what main already says. Inside a quiet command's window the
//! main stream is that command's report, and stays out, as Despana's pump
//! keeps it out (`cena-web/src/presentation/pending.rs`).

use std::collections::VecDeque;
use std::time::{Duration, Instant};

use cena_session::stream_windows::{Destination, MAIN};
use cena_session::{Event, Frame, GameState, Generation, Notice, ObservedEvent};
use cena_ui::{StyledRun, painted, story_lines};

/// Lines a story keeps, newest last.
pub(crate) const MAX_STORY: usize = 1000;
/// Hydra's messages a play window keeps.
pub(crate) const MAX_SAID: usize = 200;
/// A trigger's banners shown at once: `VellumFE`'s `MAX_CONCURRENT`, as
/// Despana's pump keeps.
pub(crate) const MAX_ALERTS: usize = 5;
/// How long a banner stays up.
pub(crate) const ALERT_FOR: Duration = Duration::from_secs(10);

/// One line of the story.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum Shown {
    /// A line the game sent, as the session painted it.
    Game(Vec<StyledRun>),
    /// A command echoed as sent: what the player typed here, after the
    /// prompt it followed, or what a behavior sent, after its name.
    Typed {
        /// The last prompt shown before it, `>` or `R>`; or the behavior's
        /// name as a prompt, `go2>`.
        prompt: String,
        /// What was typed.
        line: String,
    },
    /// The game's prompt, `>`: the end of what it said.
    Prompt(String),
    /// Lines were lost here: the feed fell behind.
    Gap,
    /// A line of another stream, which the game sends to the story while
    /// that stream's window is closed: kept apart from the story's own, so
    /// the story leaves it out while a widget of the stream is open.
    From(String, Vec<StyledRun>),
    /// Hydra's answer to something the player did, among the game's lines
    /// ([`cena_session::Notice::answer`]).
    Said(Notice),
}

/// What a play window shows of one character, kept by its feed.
#[derive(Debug, Default)]
pub(crate) struct Story {
    /// The story, oldest first, each line with when it arrived.
    pub(crate) lines: VecDeque<(Stamp, Shown)>,
    /// Hydra's messages, oldest first. Debug ones are not kept: a screen
    /// wants what a log may not (`cena_session::NoticeKind::Debug`).
    pub(crate) said: VecDeque<Notice>,
    /// A trigger's banners, with when each arrived.
    pub(crate) alerts: VecDeque<(Instant, String)>,
    /// Lines of the game's heard, ever: a tab not showing counts how many
    /// came since it last did (`plan/49` §2).
    pub(crate) heard: u64,
    /// Hydra's messages told, ever, for the same.
    pub(crate) told: u64,
    /// Each stream's own lines, for a widget of it (`streams.rs`).
    pub(crate) streams: streams::Streams,
    /// The game's last answer to a menu asked for on this connection, and
    /// how many came before it, for a request to know one after it.
    pub(crate) menu: Option<(u64, cena_session::Menu)>,
    /// Inside a quiet command's window.
    quiet: bool,
    /// The last prompt shown; `>` before any.
    prompt: Option<String>,
    /// The story has had a line since the last prompt.
    since_prompt: bool,
    /// The connection the last event came on.
    generation: Option<Generation>,
    /// Lines dropped from the front, ever: the first kept line's number.
    pub(crate) dropped: u64,
}

impl Story {
    /// Take in one event the session published: `state` is the character as
    /// its last snapshot had it, for where a stream's lines go.
    pub(crate) fn hear(&mut self, event: &ObservedEvent, state: Option<&GameState>) {
        if self.generation != Some(event.generation) {
            // A window never outlives its connection.
            self.generation = Some(event.generation);
            self.quiet = false;
            self.menu = None;
        }
        match &event.event {
            Event::Quiet(quiet) => self.quiet = *quiet,
            Event::Line(line) => {
                let main = is_main(&line.stream);
                let lines = story_lines(&line.stream, painted(line));
                if !main {
                    self.streams
                        .hear(&line.stream, lines.iter().map(|shown| shown.runs.clone()));
                }
                let style = match state.map_or(Destination::Main, |state| {
                    state.stream_windows().route(&line.stream, &|_| false)
                }) {
                    Destination::Dropped => return,
                    Destination::MainStyled(style) => Some(style),
                    Destination::Main | Destination::Window(_) => None,
                };
                if self.quiet && main {
                    return;
                }
                self.take_lines(&line.stream, lines, style.as_deref());
            }
            // The prompt a viewer shows, not the game's frame: while the
            // player's Lich runs, the frame comes before Lich's lines and
            // this after them (`cena_session::Event::Prompt`).
            Event::Prompt(text) => self.prompted(text),
            // A behavior's command, echoed with its name as Lich echoes a
            // script's (the author, 2026-09-29: *"go2>look"*). What the
            // player typed is echoed as it is sent (`typed`), not here.
            Event::Sent {
                line, by: Some(by), ..
            } => self.push(Shown::Typed {
                prompt: format!("{by}>"),
                line: line.clone(),
            }),
            Event::Frame(frame) => {
                if let Frame::MenuResponse(menu) = frame.as_ref() {
                    let count = self.menu.as_ref().map_or(0, |(count, _)| count + 1);
                    self.menu = Some((count, menu.clone()));
                }
            }
            Event::Notice(notice) => self.tell(notice.clone()),
            Event::Attention(call) => {
                if let Some(alert) = &call.alert {
                    self.alert(alert);
                }
            }
            _ => {}
        }
    }

    /// Lines were lost: mark the place, once, and show the story again, a
    /// quiet command's end perhaps among them: the snapshot does not say,
    /// and a story shown too much beats one silent for good (the crate
    /// review of 2026-09-28, R8).
    pub(crate) fn missed(&mut self) {
        self.quiet = false;
        if self.lines.back().map(|(_, shown)| shown) != Some(&Shown::Gap) {
            self.push(Shown::Gap);
        }
    }

    pub(super) fn push(&mut self, shown: Shown) {
        self.lines.push_back((Stamp::now(), shown));
        while self.lines.len() > MAX_STORY {
            self.lines.pop_front();
            self.dropped += 1;
        }
    }
}

/// Whether a line is on the main stream: the wire writes main's text with an
/// empty stream id and declares the window `main`.
fn is_main(stream: &str) -> bool {
    stream.is_empty() || stream == MAIN
}

pub(crate) mod inbox;
mod prompt;
mod said;
mod stamp;
mod streams;

pub(crate) use prompt::visible;
pub(crate) use stamp::{Hours, Stamp};

#[cfg(test)]
mod tests;
