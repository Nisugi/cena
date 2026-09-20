//! End of chunk: the hold, the orphan sinks, the emit.
//!
//! `processor.rb:1704-1786`, then `process`'s uid resolution
//! (`processor.rb:139-147`).

use std::collections::BTreeMap;

use crate::state::combat::event::{AttackEvent, ChunkFacts, EventTarget, Fact};

use super::cursor::Cursor;

impl Cursor<'_> {
    /// Close the chunk and produce its facts.
    pub fn finish(mut self) -> ChunkFacts {
        // The last event -- unless it is a bare gesture whose spell result
        // has not arrived: "You gesture at X." ends a chunk, and the lash
        // opens the next. Held for ONE chunk.
        if let Some(cur) = self.current {
            let e = self.ev(cur);
            if self.bare_cast(cur) && !e.held && e.attacker.is_none() {
                let mut held = e.clone();
                held.root = None;
                held.parent = None;
                self.tracker.held_cast = Some(held);
            } else if self.savable(cur) {
                self.save_event(cur);
            }
        }

        // Orphaned rolls no attack claimed (trailing rider maneuvers, a
        // bespoke initiation): a synthetic unknown bound to the creature
        // being fought. Measured at 21% of exchanges lost before this existed.
        if !self.pending_resolutions.is_empty() {
            let anchor = self.current_target.clone().or_else(|| {
                self.events
                    .last()
                    .and_then(|&id| self.ev(id).target.creature().cloned())
            });
            if let Some(anchor) = anchor.filter(|a| a.id.is_some()) {
                let mut ev = self.blank("unknown", EventTarget::Creature(anchor));
                ev.resolutions.append(&mut self.pending_resolutions);
                let id = self.open(ev);
                self.events.push(id);
            }
        }

        // The orphan sink: targetless, never applied, exists for recorders.
        if !self.orphan_hits.is_empty()
            || !self.orphan_outcomes.is_empty()
            || !self.pending_resolutions.is_empty()
        {
            let mut ev = self.blank("unknown", EventTarget::None);
            ev.hits.append(&mut self.orphan_hits);
            ev.outcomes.append(&mut self.orphan_outcomes);
            ev.resolutions.append(&mut self.pending_resolutions);
            ev.orphan = true;
            let id = self.open(ev);
            self.events.push(id);
        }

        // Pre-flares no swing claimed. Ones that resolved damage against a
        // known target are HELD for one chunk (dispel-on-nock); one already
        // held once is wrapped as its own event so the damage is not dropped.
        let pending = std::mem::take(&mut self.pending_flares);
        let (hold, wrap): (Vec<_>, Vec<_>) = pending
            .into_iter()
            .partition(|f| f.target.is_some() && !f.hits.is_empty() && !f.held);
        self.tracker.held_pre_flares = hold;
        for f in wrap {
            let Some(target) = f.target.clone() else {
                continue;
            };
            if f.hits.is_empty() {
                continue;
            }
            let mut ev = self.blank(&f.name, EventTarget::Creature(target));
            ev.weapon = f.weapon.as_ref().map(|w| w.name.clone());
            ev.flares.push(f);
            let id = self.open(ev);
            self.events.push(id);
        }

        // a deferred cast whose swing never saved still carries its facts
        for c in std::mem::take(&mut self.deferred_casts) {
            if !self.events.contains(&c) {
                self.events.push(c);
            }
        }

        self.emit()
    }

    /// Resolve pool ids to emit indices, as `process` resolves object
    /// references to uids: an unemitted root is self, an unemitted parent is
    /// none, an unemitted status event is none.
    fn emit(self) -> ChunkFacts {
        let index: BTreeMap<usize, usize> = self
            .events
            .iter()
            .enumerate()
            .map(|(i, &id)| (id, i))
            .collect();
        let mut pool = self.pool;
        let events: Vec<AttackEvent> = self
            .events
            .iter()
            .enumerate()
            .map(|(i, &id)| {
                let mut e = std::mem::take(&mut pool[id]);
                e.root = Some(e.root.and_then(|r| index.get(&r).copied()).unwrap_or(i));
                e.parent = e.parent.and_then(|p| index.get(&p).copied());
                e
            })
            .collect();
        let facts = self
            .facts
            .into_iter()
            .map(|f| match f {
                Fact::Status {
                    subject,
                    status,
                    action,
                    event,
                    flare_seq,
                    line,
                } => Fact::Status {
                    subject,
                    status,
                    action,
                    event: event.and_then(|e| index.get(&e).copied()),
                    flare_seq,
                    line,
                },
                other => other,
            })
            .collect();
        ChunkFacts { events, facts }
    }
}
