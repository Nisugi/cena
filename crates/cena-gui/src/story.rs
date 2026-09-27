//! One character's story as its play window shows it (`plan/47` step 4):
//! the game's lines, what the player typed, Hydra's own messages in their
//! own pane, and a trigger's banners.
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
use cena_session::{Event, GameState, Generation, Notice, NoticeKind, ObservedEvent};
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
    /// What the player typed here, echoed as sent.
    Typed(String),
    /// Lines were lost here: the feed fell behind.
    Gap,
    /// A line of another stream, which the game sends to the story while
    /// that stream's window is closed: kept apart from the story's own, so
    /// the story leaves it out while a widget of the stream is open.
    From(String, Vec<StyledRun>),
}

/// What a play window shows of one character, kept by its feed.
#[derive(Debug, Default)]
pub(crate) struct Story {
    /// The story, oldest first.
    pub(crate) lines: VecDeque<Shown>,
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
    /// Inside a quiet command's window.
    quiet: bool,
    /// The connection the last event came on.
    generation: Option<Generation>,
}

impl Story {
    /// Take in one event the session published: `state` is the character as
    /// its last snapshot had it, for where a stream's lines go.
    pub(crate) fn hear(&mut self, event: &ObservedEvent, state: Option<&GameState>) {
        if self.generation != Some(event.generation) {
            // A window never outlives its connection.
            self.generation = Some(event.generation);
            self.quiet = false;
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
                for shown in lines {
                    let mut runs = shown.runs;
                    if let Some(style) = &style {
                        for run in runs.iter_mut().filter(|run| run.preset.is_none()) {
                            run.preset = Some(style.clone());
                        }
                    }
                    self.push(if main {
                        Shown::Game(runs)
                    } else {
                        Shown::From(line.stream.clone(), runs)
                    });
                    self.heard += 1;
                }
            }
            Event::Notice(notice) => self.tell(notice.clone()),
            Event::Attention(call) => {
                if let Some(alert) = &call.alert {
                    self.alerts.push_back((Instant::now(), alert.clone()));
                    while self.alerts.len() > MAX_ALERTS {
                        self.alerts.pop_front();
                    }
                }
            }
            _ => {}
        }
    }

    /// Lines were lost: mark the place, once.
    pub(crate) fn missed(&mut self) {
        if self.lines.back() != Some(&Shown::Gap) {
            self.push(Shown::Gap);
        }
    }

    /// The player typed `line` here.
    pub(crate) fn typed(&mut self, line: &str) {
        self.push(Shown::Typed(line.to_owned()));
    }

    /// Hydra says `notice` to the player, in the messages pane.
    pub(crate) fn tell(&mut self, notice: Notice) {
        if notice.kind == NoticeKind::Debug {
            return;
        }
        self.said.push_back(notice);
        self.told += 1;
        while self.said.len() > MAX_SAID {
            self.said.pop_front();
        }
    }

    /// The banners still up at `now`, oldest first.
    pub(crate) fn alerts_at(&self, now: Instant) -> impl Iterator<Item = &str> {
        self.alerts
            .iter()
            .filter(move |(at, _)| now.saturating_duration_since(*at) < ALERT_FOR)
            .map(|(_, text)| text.as_str())
    }

    fn push(&mut self, shown: Shown) {
        self.lines.push_back(shown);
        while self.lines.len() > MAX_STORY {
            self.lines.pop_front();
        }
    }
}

/// Whether a line is on the main stream: the wire writes main's text with an
/// empty stream id and declares the window `main`.
fn is_main(stream: &str) -> bool {
    stream.is_empty() || stream == MAIN
}

mod streams;

#[cfg(test)]
mod tests;
