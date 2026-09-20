//! Attack initiations: the branch that opens events.
//!
//! `processor.rb:1137-1580`. Runs in both states -- a new attack while
//! seeking damage closes the previous event -- and is where every modifier
//! armed by an earlier line (ambush, redirect, nock, held pre-flares, held
//! rolls) is claimed. One Ruby branch, in the phases its comments already
//! divide it into.

use crate::state::chunks::ChunkLine;
use crate::state::combat::attack::{AttackLine, TargetKind, attackerless_line, swing_weapon};
use crate::state::combat::damage::DamageLine;
use crate::state::combat::event::{Confidence, EventTarget, Hit, ParentFlare};
use crate::state::combat::resolution::ResolutionKind;

use super::cursor::{Cursor, EvId, ReleasedParent, starts_you};
use super::{LineCtx, SINGLE_HIT_ATTACKS, SUMMARY_DAMAGE_ATTACKS, UNOWNED_TICK_ATTACKS};

/// A switch artifact's inherited lineage, carried to the event that replaces
/// it on the same line.
struct Lineage {
    root: Option<EvId>,
    parent: Option<EvId>,
    confidence: Option<Confidence>,
}

impl Cursor<'_> {
    /// The `(spell, victim)` key of the `DoT` ownership table.
    fn cast_owner_key(name: &str, target: &EventTarget) -> (String, String) {
        let victim = match target {
            EventTarget::Creature(a) => a.id.map_or_else(|| a.name.clone(), |i| i.to_string()),
            EventTarget::Foreign(n) => n.clone(),
            EventTarget::None => String::new(),
        };
        (name.to_owned(), victim)
    }

    /// The attack branch. `atk` is the line's initiation, already gated:
    /// `None` when an ambush or redirect prefix took the line.
    pub fn handle_attack(&mut self, ctx: &LineCtx, line: &ChunkLine, atk: AttackLine) {
        let superseded_cast = self.supersede_bare_cast(ctx, &atk);
        self.note_single_hit_parent(ctx, &atk);
        self.close_previous(ctx, line, &atk);
        let artifact_lineage = self.take_artifact_lineage(ctx);
        let id = self.open_attack_event(ctx, line, atk, superseded_cast);
        let name = self.ev(id).name.clone();
        self.stamp_lineage(id, &ctx.text, artifact_lineage);

        // Record ownership of a DoT from its CAST line. Cast lines carry no
        // inline damage, and inline damage is added below, so this always
        // runs; Lich's `hits.empty?` guard is a tautology there too.
        if UNOWNED_TICK_ATTACKS.contains(&name.as_str()) {
            let e = self.ev(id);
            let owner = if e.foreign_caster {
                Some(false)
            } else if e.attacker.is_none() && starts_you(&ctx.text) {
                Some(true)
            } else {
                None
            };
            self.cast_owner
                .insert(Self::cast_owner_key(&name, &e.target), owner);
        }

        // Claimed: the ambush belongs to this attack only.
        self.pending_ambush = None;
        self.current_target = self
            .ev(id)
            .target
            .creature()
            .filter(|a| a.id.is_some())
            .cloned();
        self.bind_assault(id, &name);

        // Inline damage: DoT ticks are the whole event, message and damage in
        // one. Summary-damage attacks print the total the next line repeats.
        if !SUMMARY_DAMAGE_ATTACKS.contains(&name.as_str())
            && let Some(inline) = DamageLine::classify(line)
        {
            self.ev_mut(id).hits.push(Hit {
                damage: inline.amount,
                crit: None,
                line: ctx.index,
            });
        }
        if let Some(o) = ctx.same_line_outcome {
            self.ev_mut(id).outcomes.push(o);
        }

        // A new swing claims held pre-flares and held rolls. An inbound
        // attack claims neither: both were produced by OUR weapon.
        if !self.ev(id).inbound {
            self.claim_pending_flares(id);
            let mut rolls = std::mem::take(&mut self.pending_resolutions);
            self.ev_mut(id).resolutions.append(&mut rolls);
        }
        self.claim_maneuver_rolls(id);
        self.flare_ctx = None;
        self.seeking_damage = true;
    }

    /// A bare gesture `cast` is the WRAPPER for the spell-specific
    /// initiation that follows: hand its rolls and fact-less flares to the
    /// new event and discard it. A HELD cast is superseded only by a
    /// spell-result line, never by a fresh 2p initiation of our own.
    fn supersede_bare_cast(&mut self, ctx: &LineCtx, atk: &AttackLine) -> bool {
        let Some(cur) = self.current else {
            return false;
        };
        let e = self.ev(cur);
        let same_attacker = match (&atk.attacker, &e.attacker) {
            (None, _) => true,
            (Some(a), Some(c)) => a.name == c.name,
            (Some(_), None) => false,
        };
        let supersedes =
            self.bare_cast(cur) && same_attacker && (!e.held || !starts_you(&ctx.text));
        if !supersedes {
            return false;
        }
        let e = self.ev_mut(cur);
        let mut rolls = std::mem::take(&mut e.resolutions);
        let mut flares = std::mem::take(&mut e.flares);
        rolls.append(&mut self.pending_resolutions);
        self.pending_resolutions = rolls;
        self.pending_flares.append(&mut flares);
        self.current = None;
        true
    }

    /// A one-hit side effect remembers the event it cut in front of; its
    /// own line names ITS victim, so the switcher has already saved the
    /// real parent and left a same-line artifact in `current`.
    fn note_single_hit_parent(&mut self, ctx: &LineCtx, atk: &AttackLine) {
        self.single_hit_parent = None;
        if !SINGLE_HIT_ATTACKS.contains(&atk.name.as_str()) {
            return;
        }
        let cand = match self.current {
            Some(c) if self.ev(c).born_line == Some(ctx.index) => self.events.last().copied(),
            other => other,
        };
        if let Some(c) = cand {
            let e = self.ev(c);
            if !e.inbound && !e.foreign_caster && !e.is_foreign_target() {
                self.single_hit_parent = Some(c);
            }
        }
    }

    /// Save the previous event -- unless the switcher created it on this
    /// very line -- and remember what the new line does to it.
    fn close_previous(&mut self, ctx: &LineCtx, line: &ChunkLine, atk: &AttackLine) {
        let Some(cur) = self.current else {
            return;
        };
        if !self.savable(cur) || self.ev(cur).born_line == Some(ctx.index) {
            return;
        }
        self.save_event(cur);
        let e = self.ev(cur);
        let ours = !(e.inbound || e.foreign_caster || e.is_foreign_target());
        // an inbound attack cutting into our own open swing
        if atk.inbound && ours {
            self.interrupted_own = Some(cur);
        }
        // our swing carrying an UNCLAIMED spell-releasing flare: a weaponless
        // CAST opening now is that flare's spell. One flare releases one spell.
        if !atk.inbound && ours && atk.weapon.is_none() && swing_weapon(line).is_none() {
            let e = self.ev_mut(cur);
            if let Some(rel) = e
                .flares
                .iter_mut()
                .find(|f| Self::releasing(&f.name) && !f.release_claimed)
            {
                rel.release_claimed = true;
                let (flare, weapon) = (rel.name.clone(), rel.weapon.clone());
                self.interrupted_own = Some(cur);
                self.released_parent = Some(ReleasedParent {
                    event: cur,
                    flare,
                    weapon,
                });
            }
        }
        let e = self.ev(cur);
        if e.inbound && e.attacker.is_some() {
            let attacker = e.attacker.clone();
            self.last_inbound_attacker = attacker;
        }
    }

    /// A same-line switch artifact may have claimed held rolls and inherited
    /// lineage: carry both across the replacement.
    fn take_artifact_lineage(&mut self, ctx: &LineCtx) -> Option<Lineage> {
        let cur = self.current?;
        if self.ev(cur).born_line != Some(ctx.index) {
            return None;
        }
        let e = self.ev_mut(cur);
        if !e.resolutions.is_empty() {
            let mut rolls = std::mem::take(&mut e.resolutions);
            rolls.append(&mut self.pending_resolutions);
            self.pending_resolutions = rolls;
        }
        let e = self.ev(cur);
        e.root.is_some().then_some(Lineage {
            root: e.root,
            parent: e.parent,
            confidence: e.parent_confidence,
        })
    }

    /// Build the event from the line, and make it current.
    fn open_attack_event(
        &mut self,
        ctx: &LineCtx,
        line: &ChunkLine,
        atk: AttackLine,
        superseded_cast: bool,
    ) -> EvId {
        let text = &ctx.text;
        // Foreign-attacker latch: a 2p "You..." attack reclaims ownership; a
        // foreign_caster attack arms it, so the actor-less swings of a nearby
        // player's AoE inherit their ownership.
        let our_2p = starts_you(text) && atk.attacker.is_none() && !atk.foreign_caster;
        if our_2p {
            self.foreign_latch = None;
        }
        if atk.foreign_caster
            && let Some(a) = &atk.attacker
        {
            self.foreign_latch = Some(a.name.clone());
        }
        let eff_foreign = atk.foreign_caster
            || (self.foreign_latch.is_some() && atk.attacker.is_none() && !our_2p);

        let target = match atk.target {
            TargetKind::Creature(a) => EventTarget::Creature(a),
            TargetKind::Foreign(n) => EventTarget::Foreign(n),
            TargetKind::None => EventTarget::None,
        };
        // A DoT tick with no owning cast of that spell on THIS victim is
        // unowned: real damage to the creature, not our deal.
        let unowned = UNOWNED_TICK_ATTACKS.contains(&atk.name.as_str())
            && atk.attacker.is_none()
            && !eff_foreign
            && !starts_you(text)
            && self
                .cast_owner
                .get(&Self::cast_owner_key(&atk.name, &target))
                .copied()
                .flatten()
                != Some(true);
        let mut ev = self.blank(&atk.name, target);
        ev.attacker.clone_from(&atk.attacker);
        ev.inbound = atk.inbound;
        ev.foreign_caster = eff_foreign;
        ev.unowned = unowned;
        ev.ambush = self.pending_ambush.as_ref().is_some_and(|a| a.hidden);
        ev.attack_kind = self.pending_ambush.as_ref().map(|a| a.kind);
        ev.redirect = self.pending_redirect.take();
        ev.aimed = atk.aimed;
        ev.weapon = swing_weapon(line).or(atk.weapon);
        ev.via_cast = superseded_cast;
        ev.parent_flare = self.active_spawn.as_ref().map(|s| ParentFlare {
            flare: s.flare.clone(),
            weapon: s.weapon.clone(),
        });
        ev.attack_born = true;
        let id = self.open(ev);
        self.current = Some(id);

        // Only OUR own attack can be the swing the nock announced.
        if !(atk.inbound || eff_foreign) {
            self.nocked = None;
        }
        // A creature's warding-spell effect line that follows its cast is
        // that caster's.
        if atk.inbound
            && atk.attacker.is_none()
            && !attackerless_line(line)
            && let Some(lia) = self.last_inbound_attacker.clone()
        {
            self.ev_mut(id).attacker = Some(lia);
        }
        id
    }

    /// Spawn-tree lineage: only what can be asserted, never a positional
    /// guess. Foreign/inbound/unowned/orphan events never join our tree.
    fn stamp_lineage(&mut self, id: EvId, text: &str, artifact: Option<Lineage>) {
        let not_ours = Self::not_ours(self.ev(id));
        let name = self.ev(id).name.clone();
        if let Some(l) = artifact.filter(|_| !not_ours) {
            // Same-line AoE per-target line: the same attack striking another
            // creature sits at the SAME spawn-tree node as its sibling.
            let e = self.ev_mut(id);
            e.root = l.root;
            e.parent = l.parent;
            e.parent_confidence = l.confidence;
        } else if let (Some(_), Some(root), false) = (&self.active_spawn, self.spawn_root, not_ours)
        {
            // blink's bracketed cast: a child DECLARED by the game
            let e = self.ev_mut(id);
            e.root = Some(root);
            e.parent = Some(root);
            e.parent_confidence = Some(Confidence::Bracket);
        } else if let (Some(rp), false) = (self.released_parent.take(), not_ours) {
            // Child of the swing whose flare released it: asserted by
            // adjacency, the same count constraint an echo uses.
            let owner_root = self.ev(rp.event).root.unwrap_or(rp.event);
            let e = self.ev_mut(id);
            e.root = Some(owner_root);
            e.parent = Some(rp.event);
            e.parent_confidence = Some(Confidence::Count);
            e.parent_flare = Some(ParentFlare {
                flare: rp.flare,
                weapon: rp.weapon,
            });
            e.released = true;
        } else if !not_ours
            && self.spawn_root.is_some()
            && starts_you(text)
            && self.pending_echoes.front().is_some_and(|echo| {
                let owner = echo.owner.or(self.spawn_root).unwrap_or_default();
                self.ev(owner).name == name
            })
        {
            // A mirror/afterimage ECHO: N echo flares, N echo swings, paired
            // FIFO. The flare is the attack (owner ruling 2026-09-07).
            let echo = self
                .pending_echoes
                .pop_front()
                .unwrap_or_else(|| unreachable!());
            let owner = echo.owner.or(self.spawn_root).unwrap_or_default();
            let root = self.spawn_root;
            let e = self.ev_mut(id);
            e.root = root;
            e.parent = Some(owner);
            e.parent_confidence = Some(Confidence::Count);
            e.parent_flare = Some(ParentFlare {
                flare: echo.flare,
                weapon: echo.weapon,
            });
        } else {
            // every other own attack becomes the root of its OWN tree
            if !not_ours {
                self.spawn_root = Some(id);
            }
            self.ev_mut(id).root = Some(id);
        }
        // A weaponless cast opening while a spell-releasing pre-flare is
        // still unclaimed IS that flare's spell (proc -> spell -> swing).
        if !not_ours
            && !self.ev(id).released
            && self.ev(id).weapon.is_none()
            && self.pending_flares.iter().any(|f| Self::releasing(&f.name))
        {
            self.ev_mut(id).released = true;
            self.pending_release_cast = Some(id);
        }
    }

    /// A creature MANEUVER rolls before its line prints: that held maneuver
    /// roll is its own, not our next swing's -- unless we are mid-sequence
    /// (volley, barrage), which announce themselves. Every OTHER assault
    /// rolls AFTER its round line, so an open one must not block the claim.
    fn claim_maneuver_rolls(&mut self, id: EvId) {
        let barrage_open = self
            .tracker
            .active_assault
            .as_ref()
            .is_some_and(|a| a.name.rolls_before_round());
        if !self.ev(id).inbound
            || self.pending_resolutions.is_empty()
            || self.active_sequence.is_some()
            || self.nocked.is_some()
            || barrage_open
        {
            return;
        }
        let (mut mine, rest): (Vec<_>, Vec<_>) = std::mem::take(&mut self.pending_resolutions)
            .into_iter()
            .partition(|r| {
                matches!(
                    r.kind,
                    ResolutionKind::Smr | ResolutionKind::Ssr | ResolutionKind::ManeuverRoll
                )
            });
        self.pending_resolutions = rest;
        self.ev_mut(id).resolutions.append(&mut mine);
    }

    /// Assault binding: while an assault is open, its own targetless rounds
    /// strike the assault target by definition. Any OTHER outbound attack
    /// def mid-assault means the bracket state is stale: drop it.
    fn bind_assault(&mut self, id: EvId, name: &str) {
        let Some(aa) = self.tracker.active_assault.clone() else {
            return;
        };
        let e = self.ev(id);
        if e.inbound || e.is_foreign_target() {
            return;
        }
        if name == aa.name.as_str() || name == "ambush" {
            if self.current_target.is_none() {
                if let Some(t) = aa.target.filter(|a| a.id.is_some()) {
                    self.ev_mut(id).target = EventTarget::Creature(t.clone());
                    self.current_target = Some(t);
                }
            } else if aa.target.is_none()
                && let Some(open) = self.tracker.active_assault.as_mut()
            {
                // barrage's opener names no target: the first named round
                // inside the bracket backfills it
                open.target.clone_from(&self.current_target);
            }
        } else if e.attacker.is_none() && !matches!(name, "unknown" | "companion") && !e.released {
            self.tracker.active_assault = None;
        }
    }

    /// The pre-flare claim (`processor.rb:1520-1573`).
    ///
    /// A bow's pre-flare names the BOW while the shot names the ARROW, so a
    /// weapon match cannot be required: a pre-flare belongs to this swing
    /// unless a flare from a DIFFERENT weapon already sits on it. A
    /// spell-RELEASING pre-flare rode a swing, never the spell it released:
    /// only an attack that names a weapon may claim it.
    fn claim_pending_flares(&mut self, id: EvId) {
        let pending = std::mem::take(&mut self.pending_flares);
        let mut still = Vec::new();
        for mut f in pending {
            let releasing = Self::releasing(&f.name);
            let e = self.ev(id);
            let claim = if releasing {
                e.weapon.is_some() && !Self::flare_contradicts_weapon(&f, e)
            } else {
                Self::flare_matches_weapon(&f, e.weapon.as_deref())
                    || !Self::flare_contradicts_weapon(&f, e)
            };
            if !claim {
                still.push(f);
                continue;
            }
            // fired BEFORE the swing line: never the cause of a status the
            // swing's own crit inflicts afterwards
            f.pre = true;
            let cast = if releasing {
                self.pending_release_cast.take()
            } else {
                None
            };
            if cast.is_some() {
                f.release_claimed = true;
            }
            let (flare, weapon) = (f.name.clone(), f.weapon.clone());
            self.ev_mut(id).flares.push(f);
            // ...and the cast that flare released, which printed before this
            // swing, is this swing's child: re-emitted after the swing.
            if let Some(cast) = cast {
                let root = self.ev(id).root.unwrap_or(id);
                let c = self.ev_mut(cast);
                c.root = Some(root);
                c.parent = Some(id);
                c.parent_confidence = Some(Confidence::Count);
                c.parent_flare = Some(ParentFlare { flare, weapon });
                self.events.retain(|&e| e != cast);
                self.released_children.entry(id).or_default().push(cast);
                self.deferred_casts.push(cast);
            }
        }
        self.pending_flares = still;
    }
}
