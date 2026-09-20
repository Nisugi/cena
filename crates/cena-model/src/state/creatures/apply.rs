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
                Fact::Status { .. } | Fact::SpellLoss { .. } | Fact::Dead { .. } => {}
            }
        }
        for event in &facts.events {
            self.persist_event(event, at);
        }
        for creature in self.sweep_deaths() {
            facts.facts.push(Fact::Dead { creature });
        }
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
    fn persist_event(&mut self, event: &AttackEvent, at: Option<u32>) {
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
                self.apply_crit_statuses(id, crit, at);
            }
        }
        for flare in &event.flares {
            let Some(id) = self.flare_target(flare.target.as_ref(), target, at) else {
                continue;
            };
            for crit in flare.hits.iter().filter_map(|h| h.crit.as_ref()) {
                self.apply_crit_statuses(id, crit, at);
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
    /// inside one entry: `stunned` is rounds, `roundtime` is seconds.
    fn apply_crit_statuses(&mut self, id: i64, crit: &Crit, at: Option<u32>) {
        let recovered = self.recovered_after(id, Some(crit.line));
        let Some(c) = self.instances.get_mut(&id) else {
            return;
        };
        if crit.stunned > 0 {
            c.add_status(StatusName::Stunned, at, None);
            c.add_stun_estimate(crit.stunned, at);
        }
        if crit.roundtime > 0 {
            c.add_status(StatusName::Roundtime, at, Some(u32::from(crit.roundtime)));
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
            c.add_status(status, at, None);
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
                c.add_status(status, at, None);
            }
        }
    }
}
