//! Roll lines and outcome lines.
//!
//! `processor.rb:918-1135`. Both attach to whatever the cursor points at --
//! an active flare owns its own SMR, the swing owns its AS/DS -- and run
//! AFTER target switching, because an outcome names its target.

use crate::state::chunks::ChunkLine;
use crate::state::combat::bracket::SequenceName;
use crate::state::combat::event::EventTarget;
use crate::state::combat::outcome::Outcome;
use crate::state::combat::resolution::{Resolution, ResolutionKind, crit_rider_line};

use super::LineCtx;
use super::cursor::{Cursor, Sink};

impl Cursor<'_> {
    /// Route a roll line to its sink.
    ///
    /// Roll routing differs by roll class (fixture-verified): SMR/SSR/
    /// maneuver rolls PRECEDE their per-target line, so one arriving on a
    /// settled event belongs to the NEXT target and is held; AS/DS-class
    /// rolls always FOLLOW their attack line, so a multi-strike keeps every
    /// roll on the attack-born event.
    fn handle_resolution(&mut self, ctx: &LineCtx, res: Resolution) {
        // A PHYSICAL roll arriving on a flare-released spell that has ALREADY
        // resolved is the interrupted swing's: resume the swing first. A
        // released BOLT's own AS/DS is the first roll after its cast line.
        if let (Some(cur), Some(io)) = (self.current, self.interrupted_own) {
            let e = self.ev(cur);
            if e.released
                && matches!(res.kind, ResolutionKind::AsDs | ResolutionKind::UafUdf)
                && (!e.resolutions.is_empty() || !e.hits.is_empty() || !e.outcomes.is_empty())
            {
                self.save_event(cur);
                self.interrupted_own = None;
                self.resume(io);
                self.seeking_damage = true;
                self.flare_ctx = None;
            }
        }
        // A damaging flare claims a roll only while it has no damage yet; one
        // that already dealt its damage is complete (an acid proc must not
        // steal the next swing's AS/DS).
        let settled_flare = self.flare_ctx.filter(|fc| !self.flare(*fc).hits.is_empty());
        if settled_flare.is_some() {
            self.flare_ctx = None;
        }
        let mut sink = self.flare_ctx.map(Sink::Flare);
        // Crit RIDER roll: a knockdown crit rolls its own SMR after the damage
        // and narrates the fall on the very next line. It belongs to the hit
        // that caused it.
        if sink.is_none()
            && matches!(res.kind, ResolutionKind::Smr | ResolutionKind::ManeuverRoll)
            && self.lines.get(ctx.index + 1).is_some_and(crit_rider_line)
        {
            sink = settled_flare.map(Sink::Flare).or_else(|| {
                self.current
                    .filter(|&c| !self.ev(c).hits.is_empty())
                    .map(Sink::Event)
            });
        }
        if sink.is_none()
            && let Some(cur) = self.current
        {
            let e = self.ev(cur);
            if res.kind.precedes_its_line() {
                // On an ATTACK-BORN event only damage settles it (cripple:
                // init -> resisted -> SMR). On a switch-born event an
                // outcome or a STATUS settles too (eviscerate rolls once
                // per onlooker, each BEFORE the creature it applies to).
                let settled = !e.hits.is_empty()
                    || (!e.attack_born && (!e.outcomes.is_empty() || e.had_status));
                if !settled {
                    sink = Some(Sink::Event(cur));
                }
            } else if e.attack_born || (e.hits.is_empty() && e.outcomes.is_empty()) {
                sink = Some(Sink::Event(cur));
            }
        }
        match sink {
            Some(Sink::Flare(fr)) => self.flare_mut(fr).resolutions.push(res),
            Some(Sink::Event(id)) => self.ev_mut(id).resolutions.push(res),
            None => self.pending_resolutions.push(res),
        }
    }

    /// Route an outcome line.
    fn handle_outcome(&mut self, ctx: &mut LineCtx, out: &Outcome) {
        // A pre-emptive evade ("bounds to safety as you move to attack it")
        // is OUR swing resolving. Scoped to a live nock: only there do we
        // know an outbound swing is outstanding.
        let preempts_open_inbound = self.nocked.is_some()
            && self.cur().is_some_and(|e| e.inbound)
            && self.flare_ctx.is_none()
            && ctx.line_target.as_ref().is_some_and(|a| a.id.is_some())
            && !out.names_attacker;
        if ctx.line_attack.is_some() {
            // An outcome ON an initiation line belongs to the event that line
            // opens below.
            ctx.same_line_outcome = Some(out.kind);
            return;
        }
        if let Some(fc) = self.flare_ctx {
            self.flare_mut(fc).outcomes.push(out.kind);
            return;
        }
        if let Some(cur) = self.current.filter(|_| !preempts_open_inbound) {
            self.ev_mut(cur).outcomes.push(out.kind);
            return;
        }
        let Some(lt) = ctx.line_target.clone().filter(|a| a.id.is_some()) else {
            // No event, no named target: an initiation we have no def for.
            self.orphan_outcomes.push(out.kind);
            return;
        };
        // Reached with an open event only via preempts_open_inbound: the
        // creature's swing that interleaved between our nock and the evade.
        // Save it before opening ours.
        if let Some(cur) = self.current
            && self.savable(cur)
        {
            self.save_event(cur);
        }
        let hidden = self.pending_ambush.as_ref().is_some_and(|a| a.hidden);
        let kind = self.pending_ambush.as_ref().map(|a| a.kind);
        if out.names_attacker {
            // The line names who attacked US: an INBOUND unknown with the
            // creature as attacker (a wholly-negated ambush is this shape).
            let mut ev = self.blank(if hidden { "ambush" } else { "unknown" }, EventTarget::None);
            ev.inbound = true;
            ev.attacker = Some(lt);
            ev.outcomes.push(out.kind);
            ev.ambush = hidden;
            ev.attack_kind = kind;
            ev.resolutions.append(&mut self.pending_resolutions);
            self.pending_ambush = None;
            let id = self.open(ev);
            self.current = Some(id);
            self.seeking_damage = true;
        } else {
            // Inside a volley bracket the round IS a volley arrow; a nocked
            // bow with no fire line printed means the outcome IS the fire.
            let round_name = self
                .active_sequence
                .filter(|s| s.rounds_are_attacks())
                .map(SequenceName::as_str);
            let nock_fire = self.nocked.is_some() && !hidden && round_name.is_none();
            let name = if hidden {
                "ambush"
            } else {
                round_name.unwrap_or(if nock_fire { "fire" } else { "unknown" })
            };
            let mut ev = self.blank(name, EventTarget::Creature(lt.clone()));
            ev.weapon = if nock_fire { self.nocked.clone() } else { None };
            ev.outcomes.push(out.kind);
            ev.ambush = hidden;
            ev.attack_kind = kind;
            ev.resolutions.append(&mut self.pending_resolutions);
            let id = self.open(ev);
            if round_name.is_some() {
                // lineage as the attack branch would stamp it
                let root = *self.spawn_root.get_or_insert(id);
                self.ev_mut(id).root = Some(root);
            }
            self.pending_ambush = None;
            if nock_fire {
                self.nocked = None;
            }
            self.current_target = Some(lt);
            self.current = Some(id);
            self.seeking_damage = true;
        }
    }

    /// The roll-or-outcome branch.
    pub fn handle_rolls(&mut self, ctx: &mut LineCtx, line: &ChunkLine) {
        if let Some(res) = Resolution::classify(line) {
            self.handle_resolution(ctx, res);
        } else if let Some(out) = Outcome::classify(line) {
            self.handle_outcome(ctx, &out);
        }
    }
}
