//! The hunt driver's fold: each event from the session, applied to the
//! hunt's own state, with what the hunt hears and watches for taken on the
//! way, and a dropped connection marked and waited out (`plan/30` §7: "a
//! reconnect mid-hunt keeps the hunt").
//!
//! Moved down out of `drive.rs`, which had passed its cap (`plan/05` Rule 4.4:
//! a split parent's growth goes into its submodules).

use cena_session::{CommandId, Event, Frame, GameState, Notice, NoticeKind, State};

use super::Driver;
use crate::error::BehaviorError;
use crate::travel::TravelNotes;

impl<F: FnMut() -> CommandId, W: FnMut(&TravelNotes), L: FnMut(&[String])> Driver<'_, F, W, L> {
    pub(super) fn fold(&mut self, event: &Event) -> Result<(), BehaviorError> {
        match event {
            Event::StateChanged(State::Reconnecting) => self.link_lost(),
            Event::StateChanged(State::Ready) if self.down => {
                self.down = false;
                self.handle.say(Notice::line(
                    NoticeKind::Info,
                    "Hunt: reconnected; hunting on.".to_owned(),
                ));
            }
            _ => {}
        }
        if let Event::Frame(frame) = event
            && let Frame::Text(text) = &**frame
        {
            if text.stream.is_empty() {
                self.transcript.push_str(&text.content);
                let now = self.state.game_time_now();
                self.machine.heard(&text.content, now);
            }
            // The monitor reads whole lines, from every window.
            self.line.push_str(&text.content);
            if text.ends_line {
                self.machine.watched(&self.line);
                self.line.clear();
                for alert in self.machine.take_alerts() {
                    self.handle.say(Notice::line(
                        NoticeKind::Warn,
                        format!("Hunt alert: {alert}"),
                    ));
                }
            }
        }
        fold_into(&mut self.state, event)
    }

    /// The connection dropped: what it made stale is forgotten, and the
    /// hunt waits for the session to be ready again (`plan/30` §7: "a
    /// reconnect mid-hunt keeps the hunt").
    pub(super) fn link_lost(&mut self) {
        if self.down {
            return;
        }
        self.down = true;
        self.state.invalidate_for_reconnect();
        self.machine.link_lost();
        self.party_link_lost();
        self.handle.say(Notice::line(
            NoticeKind::Info,
            "Hunt: the connection dropped; waiting for it to come back.".to_owned(),
        ));
    }
}

/// Fold one event into a state: a frame is applied; a reconnect invalidates
/// what a reconnect invalidates and is waited out; a close ends the behavior;
/// a flag a trigger set is set here too, so a step's `flag` guard reads it
/// (`plan/45` Stage 2).
pub(super) fn fold_into(state: &mut GameState, event: &Event) -> Result<(), BehaviorError> {
    match event {
        Event::Frame(frame) => {
            state.apply(frame);
            Ok(())
        }
        // A drop is waited out, holding the authority (SE-4 (c)); the
        // driver marks it. Invalidating twice is harmless.
        Event::StateChanged(State::Reconnecting) => {
            state.invalidate_for_reconnect();
            Ok(())
        }
        Event::StateChanged(State::Closed) => Err(BehaviorError::Dead),
        Event::Flag(change) => {
            state.flags.apply(change);
            Ok(())
        }
        _ => Ok(()),
    }
}
