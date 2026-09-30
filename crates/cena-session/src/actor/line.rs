//! A finished line leaves the model here: once, to every viewer.
//!
//! `route_text` is where the model's lines are finished (`cena-model`'s
//! `state/streams.rs`, by [`Unfinished`](cena_model::line::Unfinished), the
//! one place a frame boundary becomes a line boundary), and `lines_seen`
//! moving is how it says it just made one. The player log already took its
//! line from that point; this publishes the same line as
//! [`Event::Line`](super::Event::Line), so a viewer draws the line the
//! classifiers and the log read, rather than assembling a second one from
//! frames (`plan/45` §4a). While the player's Lich runs, a viewer is shown
//! Lich's copy instead (`lich_text.rs`).
//!
//! It is also the point M8's triggers run at (`plan/45` §4): once per line, in
//! the session, before any viewer, so every viewer and a session nobody
//! watches agree on what a line said and what it set off.
//!
//! The line itself is `cena-model`'s ([`Line`]), since the model is what
//! makes it and what the triggers answer.
//!
//! A trigger that fires may set a flag, which changes the session's state
//! and is published ([`Event::Flag`](super::Event::Flag)), and may call for
//! attention, which is published for others to play and show
//! ([`Event::Attention`](super::Event::Attention)). The conditions, which
//! watch the character rather than a line, are read here too, at each
//! prompt: the one moment every frame of a chunk has been applied.

use std::sync::Arc;

use cena_model::line::Line;
use cena_model::state::flags::FlagChange;
use cena_model::trigger::{Act, Attention, Matcher, SEND_WINDOW};
use cena_platform::ByteSource;
use cena_protocol::Frame;

use super::{Event, SessionActor};

/// Where the lines a finished line becomes go, and whether the triggers that
/// fire on it act.
enum Show<'a> {
    /// To every viewer, now; the triggers act. The game's line.
    Now,
    /// Held for a script runner's display hooks; the triggers act.
    Held(&'a mut Vec<Arc<Line>>),
    /// Nowhere, since the player's Lich shows its copy; the triggers act.
    Not,
    /// To every viewer, now; the triggers do not act, having acted on the
    /// game's line. A line the player's Lich showed.
    Lichs,
}

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
    ///
    /// While a script runner listens, the line goes first as the game sent
    /// it ([`Event::Heard`](super::Event::Heard)): what a script reads is
    /// never what the player's triggers made of it. While the runner has
    /// display hooks, what a viewer is shown of it waits for their answer
    /// (`hooked.rs`); what the triggers do beyond the line does not.
    ///
    /// `lich`: the player's Lich took this line's chunk, and shows it
    /// (`lich_text.rs`). Its copy of the line is what a viewer is shown; this
    /// one is what the triggers act on.
    pub(super) fn publish_line(&mut self, line: Arc<Line>, lich: bool) {
        let heard = self
            .events
            .hears_lines()
            .then(|| self.events.numbered(Event::Heard(Arc::clone(&line))));
        if lich {
            self.expect_quiet(&line);
            self.publish_sorted(line, Show::Not);
            return;
        }
        let mut held = if heard.is_some() && self.events.hooks().display() {
            Some(Vec::new())
        } else {
            // Shown now, so whatever was held goes first.
            if self.held.is_waiting() {
                self.show_held(true);
            }
            None
        };
        let show = held.as_mut().map_or(Show::Now, Show::Held);
        self.publish_sorted(Arc::clone(&line), show);
        if let (Some(cursor), Some(shown)) = (heard, held) {
            self.hold(cursor, line, shown);
        }
    }

    /// Publish a line the player's Lich showed, as the game's would be:
    /// sorted and painted, with nothing the triggers do beyond the look,
    /// which they did on the game's line.
    pub(super) fn show_lichs_line(&mut self, line: Arc<Line>) {
        if self.held.is_waiting() {
            self.show_held(true);
        }
        self.publish_sorted(line, Show::Lichs);
    }

    /// Publish each line `line` sorts into, as `show` says.
    fn publish_sorted(&mut self, line: Arc<Line>, mut show: Show<'_>) {
        if self.hides(&line.runs.plain()) {
            return;
        }
        let triggers = self.events.triggers();
        if super::lich_text::is_main(&line.stream)
            && self.events.sorts_containers()
            && let Some(sorted) = cena_model::sorter::sort(&line.runs)
        {
            for runs in sorted {
                let sorted = Arc::new(Line::new(line.stream.clone(), runs));
                self.publish_answered(&triggers, sorted, &mut show);
            }
        } else {
            self.publish_answered(&triggers, line, &mut show);
        }
    }

    /// Publish what `triggers` make of `line`, as `show` says, then what the
    /// ones that fired do beyond it: on a squelched line too.
    fn publish_answered(&mut self, triggers: &Matcher, line: Arc<Line>, show: &mut Show<'_>) {
        if triggers.triggers().is_empty() {
            self.show(vec![line], show);
            return;
        }
        let answer = triggers.answer(&line, &self.state);
        self.show(answer.lines.into_iter().map(Arc::new).collect(), show);
        if !matches!(show, Show::Lichs) {
            self.fired(triggers, &answer.fired, answer.attention, answer.acts);
        }
    }

    /// Publish `lines` to every viewer, add them to what is held, or neither.
    fn show(&self, lines: Vec<Arc<Line>>, show: &mut Show<'_>) {
        // Each creature's tag after its name, when shown: after the triggers
        // read the line, so a trigger on `kobold swings` still fires
        // (`cena_model::targetid`).
        let lines = if self.events.tags_creatures() {
            lines
                .into_iter()
                .map(|line| cena_model::targetid::tagged(&line).map_or(line, Arc::new))
                .collect()
        } else {
            lines
        };
        match show {
            Show::Held(held) => held.extend(lines),
            Show::Now | Show::Lichs => {
                for line in lines {
                    let _ = self.events.send(Event::Line(line));
                }
            }
            Show::Not => {}
        }
    }

    /// At a prompt: each condition that became true does what it does.
    pub(super) fn fire_conditions(&mut self) {
        let (triggers, fired) = self.events.fire_conditions(&self.state);
        let attention = fired
            .iter()
            .filter_map(|&rank| triggers.condition_attention(rank))
            .collect();
        let acts = fired
            .iter()
            .filter_map(|&rank| triggers.condition_act(rank))
            .collect();
        self.fired(&triggers, &fired, attention, acts);
    }

    /// Set the flags of the triggers `fired`, by rank, and publish the
    /// `attention` they called for and the `acts` they send, past each
    /// trigger's cooldown and the character's pace; say the sends held back.
    fn fired(
        &mut self,
        triggers: &Matcher,
        fired: &[usize],
        attention: Vec<Attention>,
        acts: Vec<Act>,
    ) {
        let now = self.state.game_time_now();
        for &rank in fired {
            if let Some(flag) = triggers
                .triggers()
                .get(rank)
                .and_then(|t| t.rule.flag.as_ref())
            {
                self.set_flag(&flag.change(now));
            }
        }
        // Not while the session is learning the character: a send is made
        // on what the character is doing now, and a login burst replays
        // what it was doing (the crate review of 2026-09-28, R3). Dropped,
        // not held for `Ready`, and before the pace, which it never used.
        let acts = if self.lifecycle.behaviors_may_run() {
            acts
        } else {
            Vec::new()
        };
        let admitted = self.events.admit(attention, acts, now);
        for called in admitted.attention {
            let _ = self.events.send(Event::Attention(Arc::new(called)));
        }
        for act in admitted.acts {
            let _ = self.events.send(Event::Act {
                act: Arc::new(act),
                generation: self.generation,
            });
        }
        if admitted.say_held {
            let held: Vec<String> = admitted
                .held
                .iter()
                .map(|act| format!("`{}` ({})", act.line, act.trigger))
                .collect();
            let _ = self.events.send(Event::Notice(crate::notice::Notice::line(
                crate::notice::NoticeKind::Warn,
                format!(
                    "Triggers: not sent, {} lines in {SEND_WINDOW} seconds already: {}. \
                     A trigger may be answering its own line.",
                    cena_model::trigger::MAX_SENDS,
                    held.join(", ")
                ),
            )));
        }
    }

    /// Make a trigger's flag change, and publish it if it changed anything.
    fn set_flag(&mut self, change: &FlagChange) {
        if self.state.flags.apply(change) {
            let _ = self.events.send(Event::Flag(change.clone()));
        }
    }
}
