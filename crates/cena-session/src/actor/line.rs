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
    pub(super) fn publish_line(&mut self, line: Arc<Line>) {
        let heard = self
            .events
            .hears_lines()
            .then(|| self.events.numbered(Event::Heard(Arc::clone(&line))));
        let mut held = if heard.is_some() && self.events.hooks().display() {
            Some(Vec::new())
        } else {
            // Shown now, so whatever was held goes first.
            if self.held.is_waiting() {
                self.show_held(true);
            }
            None
        };
        let triggers = self.events.triggers();
        let main = line.stream.is_empty() || line.stream == "main";
        if main
            && self.events.sorts_containers()
            && let Some(sorted) = cena_model::sorter::sort(&line.runs)
        {
            for runs in sorted {
                let sorted = Arc::new(Line::new(line.stream.clone(), runs));
                self.publish_answered(&triggers, sorted, held.as_mut());
            }
        } else {
            self.publish_answered(&triggers, Arc::clone(&line), held.as_mut());
        }
        if let (Some(cursor), Some(shown)) = (heard, held) {
            self.hold(cursor, line, shown);
        }
    }

    /// Publish what `triggers` make of `line`, or add it to `held`, then what
    /// the ones that fired do beyond it: on a squelched line too.
    fn publish_answered(
        &mut self,
        triggers: &Matcher,
        line: Arc<Line>,
        held: Option<&mut Vec<Arc<Line>>>,
    ) {
        if triggers.triggers().is_empty() {
            self.show(vec![line], held);
            return;
        }
        let answer = triggers.answer(&line, &self.state);
        self.show(answer.lines.into_iter().map(Arc::new).collect(), held);
        self.fired(triggers, &answer.fired, answer.attention, answer.acts);
    }

    /// Publish `lines` to every viewer, or add them to `held`.
    fn show(&self, lines: Vec<Arc<Line>>, held: Option<&mut Vec<Arc<Line>>>) {
        match held {
            Some(held) => held.extend(lines),
            None => {
                for line in lines {
                    let _ = self.events.send(Event::Line(line));
                }
            }
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
        let admitted = self.events.admit(attention, acts, now);
        for called in admitted.attention {
            let _ = self.events.send(Event::Attention(Arc::new(called)));
        }
        for act in admitted.acts {
            let _ = self.events.send(Event::Act(Arc::new(act)));
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
