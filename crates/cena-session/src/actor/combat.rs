//! Combat facts leave the model here: once per prompt, to everyone.
//!
//! `GameState::close_chunk` parses a closed chunk, applies it to the creature
//! registry and queues the result on the tracker. This drains that queue and
//! delivers each chunk's facts two ways, which are not alternatives:
//!
//! - to the **recorder**, over its own bounded queue, because a recorder
//!   that misses a chunk is wrong forever
//!   ([`crate::combat_recorder::worker`]);
//! - to **observers**, as [`Event::Combat`] on the broadcast ring, where a
//!   slow subscriber is told `Lagged` and that is an honest answer.
//!
//! The model has already applied the chunk by the time either sees it, which
//! is `actor.rs`'s rule for frames -- *"the model already is"* the consumer
//! that must not miss -- holding for combat too.

use std::sync::Arc;

use cena_platform::ByteSource;

use super::{Event, SessionActor};

impl<S: ByteSource> SessionActor<S> {
    /// Drain the tracker after a prompt. Never blocks and never fails: a
    /// recorder that cannot keep up costs its own rows, not the session.
    pub(super) fn publish_combat(&mut self) {
        let chunks = self.state.combat_mut().take_facts();
        if chunks.is_empty() {
            // A quiet prompt still tells the recorder the time, so a hunt
            // closes on the server's clock even if combat never resumes.
            if let (Some(recorder), Some(at)) = (&self.combat, self.state.game_time()) {
                recorder.tick(at);
            }
            return;
        }
        for facts in chunks {
            let facts = Arc::new(facts);
            if let Some(recorder) = &self.combat {
                // the refusal is counted by the handle; reported below
                let _ = recorder.offer(Arc::clone(&facts));
            }
            let _ = self.events.send(Event::Combat(facts));
        }
        self.report_recorder_refusals();
    }

    /// Say so in the session log when the recorder's refusal count moves.
    /// Once per movement, not once per chunk: a stalled disk during a hunt
    /// would otherwise write a line per prompt.
    fn report_recorder_refusals(&mut self) {
        let Some(recorder) = &self.combat else {
            return;
        };
        let stats = recorder.stats();
        let (dropped, untimed, failed) = (stats.dropped(), stats.untimed(), stats.failed());
        let total = dropped + untimed + failed;
        if total == self.combat_refusals_logged {
            return;
        }
        self.combat_refusals_logged = total;
        let detail = stats.last_error().unwrap_or_default();
        self.log(&format!(
            "combat recorder: {dropped} chunks dropped (queue full), {untimed} untimed, \
             {failed} writes failed {detail}"
        ));
    }
}
