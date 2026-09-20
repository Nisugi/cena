//! Target switching, for multi-target attacks.
//!
//! `processor.rb:818-916`. An inbound event is aimed at us and has no
//! creature target by construction: it must never adopt one, or a creature's
//! own emote later in the chunk hands it the damage it dealt US (GSIV-Bodegap
//! 2025-09-17, an ogre's "laughs hysterically"). A foreign-target event is
//! bound to a non-creature for the same reason. A flare announce that names
//! ITS OWN target is not a switch either: the damage that follows is the
//! flare's, creature-attributed.

use crate::state::combat::event::EventTarget;

use super::LineCtx;
use super::cursor::Cursor;

impl Cursor<'_> {
    pub fn handle_target_switch(&mut self, ctx: &LineCtx) {
        let Some(lt) = ctx.line_target.clone() else {
            return;
        };
        if ctx.line_redirect.is_some() || !self.seeking_damage {
            return;
        }
        let Some(cur_id) = self.current else {
            return;
        };
        let cur = self.ev(cur_id);
        let flare_owned_target = ctx.line_flare
            || (ctx.line_attack.is_none()
                && cur
                    .flares
                    .iter()
                    .any(|f| f.target.as_ref().and_then(|a| a.id) == lt.id));
        if flare_owned_target || cur.inbound || cur.is_foreign_target() || cur.foreign_caster {
            return;
        }

        match &self.current_target {
            Some(ct) if ct.id != lt.id => {
                // A real switch: save the previous event if it has data, and
                // open an inherited one -- same attack name, same lineage,
                // another AoE target sits at the SAME spawn-tree node.
                if self.savable(cur_id) {
                    self.save_event(cur_id);
                }
                let src = self.ev(cur_id);
                let mut ev = self.blank(&src.name, EventTarget::Creature(lt.clone()));
                ev.weapon.clone_from(&src.weapon);
                ev.parent_flare.clone_from(&src.parent_flare);
                ev.root = src.root;
                ev.parent = src.parent;
                ev.parent_confidence = src.parent_confidence;
                // A line can be BOTH a switch and a new attack: mark the
                // birth line so the attack branch can tell an artifact.
                ev.born_line = Some(ctx.index);
                // A held roll belongs to the target this line names: volley's
                // per-arrow SMR precedes the arrow line.
                ev.resolutions.append(&mut self.pending_resolutions);
                let id = self.open(ev);
                self.current = Some(id);
                self.flare_ctx = None;
                self.current_target = Some(lt);
            }
            None => {
                // First target for the event. A creature can never be its own
                // victim: on a targetless 3p initiation the only link is the
                // ATTACKER (a triton assassin's ambush, 2024-11-21).
                let attacker_id = cur.attacker.as_ref().and_then(|a| a.id);
                if !(attacker_id.is_some() && attacker_id == lt.id) {
                    self.ev_mut(cur_id).target = EventTarget::Creature(lt.clone());
                    self.current_target = Some(lt);
                }
            }
            Some(_) => {} // same target
        }
    }
}
