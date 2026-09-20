//! Apply a chunk's combat facts to the registry: Lich's `persist_event`,
//! `apply_crit`, `apply_crit_statuses`, `apply_status_to_target` and
//! `apply_ucs_to_target` (`processor.rb:1793-2408`), in that file's order.
//!
//! # Order, and why it is the same as Lich's
//!
//! Lich applies message statuses and UCS facts WHILE parsing, then the
//! events' damage, wounds and crit statuses AFTER. The `Fact`s here are
//! already in line order, so applying every fact first and every event
//! second reproduces that -- including the one place the order shows: a
//! knockdown crit is judged against a stand-up message by LINE, not by
//! application order (`position_recovered?`, `processor.rb:2023-2036`).
//!
//! # What this adds to the chunk's facts, and in what order
//!
//! A crit's statuses are facts too: Lich emits `:stun`, `:roundtime` and
//! `:status` while it persists each event (`processor.rb:2258-2296`), then
//! the death sweep's `dead`, and only then the parse-phase facts it deferred
//! (`process`, `processor.rb:117-172`). The chunk's facts leave here in that
//! order -- crit-derived, deaths, parsed -- because a recorder reads them as
//! a stream. A knockdown the recovery rule suppressed is not a fact and is
//! not emitted, as in Lich.

use super::Creatures;
use super::body::BodyPart;
use crate::state::combat::event::{AttackEvent, ChunkFacts, Crit, Fact, Subject, UcsKind};
use crate::state::combat::status::{StatusAction, StatusName};
use crate::state::combat::target::Actor;

/// The three floor positions, one mutually exclusive channel.
const POSITIONS: [StatusName; 3] = [StatusName::Prone, StatusName::Kneeling, StatusName::Sitting];

impl Creatures {
    /// Apply one chunk's facts, then sweep the death watch and append a
    /// [`Fact::Dead`] for every creature newly flagged dead.
    pub(crate) fn apply_chunk(&mut self, facts: &mut ChunkFacts, at: Option<u32>) {
        self.begin_chunk();
        for f in &facts.facts {
            match f {
                Fact::Status {
                    subject: Subject::Creature(actor),
                    status,
                    action,
                    line,
                    ..
                } => {
                    let id = match actor.id {
                        Some(_) => self.ensure(actor, at),
                        None => self.only_named(&actor.name),
                    };
                    if let Some(id) = id {
                        self.apply_status(id, *status, *action, Some(*line), at);
                    }
                }
                Fact::Ucs { creature, kind } => {
                    if let Some(id) = self.ensure(creature, at) {
                        self.apply_ucs(id, kind, at);
                    }
                }
                // `Stun`, `Roundtime` and `Dead` are this function's own
                // output; a parsed chunk carries none.
                Fact::Status { .. }
                | Fact::SpellLoss { .. }
                | Fact::Stun { .. }
                | Fact::Roundtime { .. }
                | Fact::Dead { .. } => {}
            }
        }
        let mut derived = Vec::new();
        for (index, event) in facts.events.iter().enumerate() {
            self.persist_event(index, event, at, &mut derived);
        }
        for creature in self.sweep_deaths() {
            derived.push(Fact::Dead { creature });
        }
        derived.append(&mut facts.facts);
        facts.facts = derived;
    }

    /// A message status (`apply_status_to_target`).
    fn apply_status(
        &mut self,
        id: i64,
        status: StatusName,
        action: StatusAction,
        line: Option<usize>,
        now: Option<u32>,
    ) {
        let Some(c) = self.instances.get_mut(&id) else {
            return;
        };
        match action {
            StatusAction::Remove => {
                if status.is_position() {
                    // Position is one channel: a creature that stood up is in
                    // no floor position, whichever one the pattern named.
                    for p in POSITIONS {
                        c.remove_status(p);
                    }
                    self.note_recovery(id, line);
                } else {
                    c.remove_status(status);
                }
            }
            StatusAction::Add => {
                if status.is_position() {
                    for p in POSITIONS {
                        if p != status {
                            c.remove_status(p);
                        }
                    }
                    self.clear_recovery(id);
                }
                if let Some(c) = self.instances.get_mut(&id) {
                    c.add_status(status, now, None);
                }
            }
        }
    }

    /// A UCS fact (`apply_ucs_to_target`).
    fn apply_ucs(&mut self, id: i64, kind: &UcsKind, now: Option<u32>) {
        let Some(c) = self.instances.get_mut(&id) else {
            return;
        };
        match kind {
            UcsKind::Position(tier) => c.set_ucs_position(*tier, now),
            // Per-swing metadata: the creature's tier against us is a fact
            // for a recorder, not creature state.
            UcsKind::PositionInbound(_) => {}
            UcsKind::Tierup(attack) => c.set_ucs_tierup(*attack, now),
            UcsKind::SmiteOn => c.smite(now),
            UcsKind::SmiteOff => c.clear_smote(now),
        }
    }

    /// One event's damage, wounds and crit statuses (`persist_event`).
    fn persist_event(
        &mut self,
        index: usize,
        event: &AttackEvent,
        at: Option<u32>,
        out: &mut Vec<Fact>,
    ) {
        // A nearby player's attack resolves onto a creature we can see, but
        // its damage belongs to that player: never applied.
        if event.foreign_caster {
            return;
        }
        let target = event.target.creature().and_then(|a| self.ensure(a, at));
        // Nothing to apply: no swing creature and no flare could name one.
        if target.is_none() && event.flares.iter().all(|f| f.hits.is_empty()) {
            return;
        }
        if let Some(id) = target {
            for hit in &event.hits {
                if let Some(c) = self.instances.get_mut(&id) {
                    c.add_damage(hit.damage);
                }
            }
            for crit in event.hits.iter().filter_map(|h| h.crit.as_ref()) {
                self.apply_crit(id, crit);
            }
        }
        // Flare damage and crits apply to the flare's own target when its
        // announce line named one -- an AoE flare can strike a different
        // creature than the swing -- else the swing's.
        for flare in event.flares.iter().filter(|f| !f.hits.is_empty()) {
            let Some(id) = self.flare_target(flare.target.as_ref(), target, at) else {
                continue;
            };
            for hit in &flare.hits {
                if let Some(c) = self.instances.get_mut(&id) {
                    c.add_damage(hit.damage);
                }
            }
            for crit in flare.hits.iter().filter_map(|h| h.crit.as_ref()) {
                self.apply_crit(id, crit);
            }
        }
        // Crit statuses: the swing's on its creature, each flare's on its own.
        if let Some(id) = target {
            for crit in event.hits.iter().filter_map(|h| h.crit.as_ref()) {
                self.apply_crit_statuses(id, crit, at, (index, None), out);
            }
        }
        for (seq, flare) in event.flares.iter().enumerate() {
            let Some(id) = self.flare_target(flare.target.as_ref(), target, at) else {
                continue;
            };
            for crit in flare.hits.iter().filter_map(|h| h.crit.as_ref()) {
                self.apply_crit_statuses(id, crit, at, (index, Some(seq + 1)), out);
            }
        }
        // Watch every creature this event touched for the room feed's death.
        if let Some(id) = target {
            self.watch_for_death(id);
        }
        for flare in &event.flares {
            if let Some(id) = flare.target.as_ref().and_then(|a| a.id)
                && self.instances.contains_key(&id)
            {
                self.watch_for_death(id);
            }
        }
    }

    fn flare_target(
        &mut self,
        named: Option<&Actor>,
        swing: Option<i64>,
        at: Option<u32>,
    ) -> Option<i64> {
        match named {
            Some(a) if a.id.is_some() => self.ensure(a, at),
            _ => swing,
        }
    }

    /// Primary wound, secondary wound, amputation, fatal (`apply_crit`).
    fn apply_crit(&mut self, id: i64, crit: &Crit) {
        let Some(c) = self.instances.get_mut(&id) else {
            return;
        };
        if let Some(rank) = crit.wound_rank.filter(|&r| r > 0)
            && let Some(part) = BodyPart::from_location(crit.location)
        {
            c.add_injury(part, rank);
        }
        if let Some(secondary) = &crit.secondary_wound
            && secondary.wound_rank > 0
        {
            for part in BodyPart::from_wound(secondary.location) {
                c.add_injury(part, secondary.wound_rank);
            }
        }
        if crit.amputated
            && let Some(part) = BodyPart::from_location(crit.location)
        {
            c.amputate(part);
        }
        if crit.fatal {
            c.mark_fatal_crit();
        }
    }

    /// The statuses a crit carries (`apply_hit_crit_statuses`). Units differ
    /// inside one entry: `stunned` is rounds, `roundtime` is seconds. Each
    /// one applied is also a fact in `out`, tied to `(event, flare_seq)`.
    fn apply_crit_statuses(
        &mut self,
        id: i64,
        crit: &Crit,
        at: Option<u32>,
        (event, flare_seq): (usize, Option<usize>),
        out: &mut Vec<Fact>,
    ) {
        let recovered = self.recovered_after(id, Some(crit.line));
        let Some(c) = self.instances.get_mut(&id) else {
            return;
        };
        let creature = Actor {
            id: Some(c.id),
            noun: c.noun.clone(),
            name: c.name.clone(),
        };
        let mut statuses = Vec::new();
        if crit.stunned > 0 {
            c.add_status(StatusName::Stunned, at, None);
            c.add_stun_estimate(crit.stunned, at);
            out.push(Fact::Stun {
                creature: creature.clone(),
                rounds: crit.stunned,
                event,
                flare_seq,
            });
        }
        if crit.roundtime > 0 {
            c.add_status(StatusName::Roundtime, at, Some(u32::from(crit.roundtime)));
            out.push(Fact::Roundtime {
                creature: creature.clone(),
                seconds: crit.roundtime,
                event,
                flare_seq,
            });
        }
        // A recovery message later in this chunk already said the creature
        // is up: the EARLIER knockdown must not overwrite the LATER stand-up.
        if let Some(pos) = crit.position
            && !recovered
            && let Some(status) = StatusName::parse(pos.as_str())
        {
            for p in POSITIONS {
                if p != status {
                    c.remove_status(p);
                }
            }
            statuses.push(status);
        }
        for (flag, status) in [
            (crit.silenced, StatusName::Silenced),
            (crit.slowed, StatusName::Slowed),
            (crit.dazed, StatusName::Dazed),
            (crit.sleeping, StatusName::Sleeping),
            (crit.crippled, StatusName::Crippled),
            (crit.limb_favored, StatusName::LimbFavored),
        ] {
            if flag {
                statuses.push(status);
            }
        }
        for status in statuses {
            c.add_status(status, at, None);
            out.push(Fact::Status {
                subject: Subject::Creature(creature.clone()),
                status,
                action: StatusAction::Add,
                event: Some(event),
                flare_seq,
                line: crit.line,
            });
        }
    }
}
