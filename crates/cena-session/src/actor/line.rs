//! A finished line leaves the model here: once, to every viewer.
//!
//! `route_text` is the one place a frame boundary becomes a line boundary
//! (`cena-model`'s `state/streams.rs`), and `lines_seen` moving is how it says
//! it just made one. The player log already took its line from that point;
//! this publishes the same line as [`Event::Line`](super::Event::Line), so a
//! viewer draws the line the classifiers and the log read, rather than
//! assembling a second one from frames (`plan/45` §4a).
//!
//! It is also the point M8's triggers run at (`plan/45` §4): once per line, in
//! the session, before any viewer, so every viewer and a session nobody
//! watches agree on what a line said and what it set off.
//!
//! The line itself is `cena-model`'s ([`Line`]), since the model is what
//! makes it and what the triggers answer.
//!
//! A trigger that fires may set a flag, which changes the session's state
//! and is published ([`Event::Flag`](super::Event::Flag)). The conditions,
//! which watch the character rather than a line, are read here too, at each
//! prompt: the one moment every frame of a chunk has been applied.

use std::sync::Arc;

use cena_model::line::Line;
use cena_model::state::flags::FlagChange;
use cena_model::trigger::Matcher;
use cena_platform::ByteSource;
use cena_protocol::Frame;

use super::{Event, SessionActor};

impl<S: ByteSource> SessionActor<S> {
    /// The line `frame` finished, if it finished one.
    ///
    /// `before` is [`lines_seen`](cena_model::GameState::lines_seen) read
    /// before the frame was applied. Only a text frame with `ends_line` moves
    /// it, and it moves by one, so the line is the last in its stream.
    pub(super) fn finished_line(&self, frame: &Frame, before: u64) -> Option<Arc<Line>> {
        let Frame::Text(text) = frame else {
            return None;
        };
        if self.state.lines_seen() == before {
            return None;
        }
        let runs = self.state.stream(&text.stream).last()?;
        Some(Arc::new(Line::new(text.stream.clone(), runs.clone())))
    }

    /// Publish a finished line to every viewer, one [`Event::Line`] for
    /// each line it becomes: with `;sorter` on, a main-stream container look
    /// becomes the lines it sorts into (`cena_model::sorter`), and each line
    /// is then answered by the character's triggers ([`Matcher::respond`]):
    /// substituted, painted, redirected, or not published at all.
    ///
    /// Sorting first is what lets the triggers match each sorted line, as
    /// `VellumFE` sorts before it highlights (`plan/45` §4a). The model's
    /// scrollback and the player log keep the game's text either way.
    pub(super) fn publish_line(&mut self, line: Arc<Line>) {
        let triggers = self.events.triggers();
        let main = line.stream.is_empty() || line.stream == "main";
        if main
            && self.events.sorts_containers()
            && let Some(sorted) = cena_model::sorter::sort(&line.runs)
        {
            for runs in sorted {
                self.publish_answered(&triggers, Arc::new(Line::new(line.stream.clone(), runs)));
            }
            return;
        }
        self.publish_answered(&triggers, line);
    }

    /// Publish what `triggers` make of `line`, then set the flags of the
    /// ones that fired: on a squelched line too.
    fn publish_answered(&mut self, triggers: &Matcher, line: Arc<Line>) {
        if triggers.triggers().is_empty() {
            let _ = self.events.send(Event::Line(line));
            return;
        }
        let answer = triggers.answer(&line, &self.state);
        for shown in answer.lines {
            let _ = self.events.send(Event::Line(Arc::new(shown)));
        }
        let now = self.state.game_time_now();
        for rank in answer.fired {
            if let Some(flag) = triggers
                .triggers()
                .get(rank)
                .and_then(|t| t.rule.flag.as_ref())
            {
                self.set_flag(&flag.change(now));
            }
        }
    }

    /// At a prompt: each condition that became true sets its flag.
    pub(super) fn fire_conditions(&mut self) {
        for change in self.events.fire_conditions(&self.state) {
            self.set_flag(&change);
        }
    }

    /// Make a trigger's flag change, and publish it if it changed anything.
    fn set_flag(&mut self, change: &FlagChange) {
        if self.state.flags.apply(change) {
            let _ = self.events.send(Event::Flag(change.clone()));
        }
    }
}
