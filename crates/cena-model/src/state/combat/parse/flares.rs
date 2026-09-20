//! Flare announces, sequence brackets, assault brackets.
//!
//! `processor.rb:663-816`.

use crate::state::chunks::ChunkLine;
use crate::state::combat::bracket::{assault_end, assault_start, sequence_end, sequence_start};
use crate::state::combat::damage::DamageLine;
use crate::state::combat::event::{FlareEvent, Hit};
use crate::state::combat::flare::FlareLine;
use crate::state::combat::tracker::ActiveAssault;

use super::cursor::{ActiveSpawn, Cursor, Echo, FlareRef};
use super::{DISPEL_FLARES, ECHO_FLARES, LineCtx, SINGLE_HIT_ATTACKS};

impl Cursor<'_> {
    /// A flare announce line.
    ///
    /// A flare belongs to the attack that is open when it fires. Weapon info
    /// DISAMBIGUATES rather than gates: 63% of attacks name no weapon at all,
    /// and requiring a match orphaned 229 flares in a 60-file replay.
    pub fn handle_flare(&mut self, ctx: &mut LineCtx, line: &ChunkLine) {
        let Some(fl) = FlareLine::classify(line) else {
            return;
        };
        ctx.line_flare = true;
        let mut flare = FlareEvent {
            name: fl.name,
            damaging: fl.damaging,
            aoe: fl.aoe,
            spawns: fl.spawns,
            // `target_info` is the LINE's bolded creature, not the capture
            target: ctx.line_target.clone(),
            weapon: fl.weapon,
            attacker: fl.attacker,
            hits: Vec::new(),
            outcomes: Vec::new(),
            resolutions: Vec::new(),
            pre: false,
            line: ctx.index,
            held: false,
            release_claimed: false,
        };
        // arm spell_loss cause attribution for the rest of the chunk
        if DISPEL_FLARES.contains(&flare.name.as_str()) {
            self.chunk_dispels
                .push(ctx.line_target.as_ref().and_then(|a| a.id));
        }
        // Some announces carry damage INLINE ("the miasma around X flares
        // causing 58 points of damage!")
        if let Some(inline) = DamageLine::classify(line) {
            flare.hits.push(Hit {
                damage: inline.amount,
                crit: None,
                line: ctx.index,
            });
        }

        // A one-hit side effect (mount collapse) has its hit: the flare that
        // follows is the interrupted swing's.
        if let (Some(cur), Some(shp)) = (self.current, self.single_hit_parent) {
            let e = self.ev(cur);
            if SINGLE_HIT_ATTACKS.contains(&e.name.as_str()) && !e.hits.is_empty() {
                self.save_event(cur);
                self.single_hit_parent = None;
                self.resume(shp);
            }
        }
        // Our own flare arriving while a creature's inbound attack (or a
        // released cast) is the open event belongs to the swing that attack
        // interrupted: resume it. Anything the swing could own resumes it --
        // breeze and arcane reflex name no weapon.
        if let (Some(cur), Some(io)) = (self.current, self.interrupted_own) {
            let e = self.ev(cur);
            if (e.inbound || e.released) && !Self::flare_contradicts_weapon(&flare, e) {
                self.save_event(cur);
                self.interrupted_own = None;
                self.resume(io);
                self.seeking_damage = true;
            }
        }
        // A spell-RELEASING flare fires at the START of an attack. One
        // arriving on a swing that has already landed or resolved -- or on an
        // inbound event -- is the NEXT swing's, and is held for it.
        let releasing = Self::releasing(&flare.name);
        let attach = self.cur().is_some_and(|e| {
            let settled = e.inbound || !e.hits.is_empty() || !e.outcomes.is_empty();
            !(Self::flare_contradicts_weapon(&flare, e) || (releasing && settled))
        });
        let damaging = flare.damaging;
        let spawns = flare.spawns;
        let is_echo = ECHO_FLARES.contains(&flare.name.as_str());
        let (name, weapon) = (flare.name.clone(), flare.weapon.clone());
        let placed = if attach {
            let cur = self.current.unwrap_or_default();
            let e = self.ev_mut(cur);
            e.flares.push(flare);
            FlareRef::OnEvent(cur, e.flares.len() - 1)
        } else {
            self.pending_flares.push(flare);
            FlareRef::Pending(self.pending_flares.len() - 1)
        };
        // Only damaging flares own subsequent damage lines; a buff flare
        // claiming the cursor would steal the parent swing's damage.
        self.flare_ctx = damaging.then_some(placed);
        if spawns {
            self.spawn_pending = Some((name.clone(), weapon.clone()));
        }
        // An echo flare is the spawn point of the bare 2p swing that follows
        // it in this chunk.
        if is_echo {
            self.pending_echoes.push_back(Echo {
                flare: name,
                weapon,
                owner: self.current,
            });
        }
    }

    /// Sequence brackets: a spawn-class flare's imbedded cast unfolds inside
    /// one, and any bracket names the rounds inside it.
    pub fn handle_sequence(&mut self, line: &ChunkLine) {
        if let Some(seq) = sequence_start(line) {
            self.active_sequence = Some(seq.name);
            if let Some((flare, weapon)) = self.spawn_pending.take() {
                self.active_spawn = Some(ActiveSpawn {
                    flare,
                    sequence: seq.name,
                    weapon,
                });
            }
        } else if let Some(end) = sequence_end(line) {
            if self.active_sequence == Some(end) {
                self.active_sequence = None;
            }
            if self
                .active_spawn
                .as_ref()
                .is_some_and(|s| s.sequence == end)
            {
                self.active_spawn = None;
            }
        }
    }

    /// Assault brackets: the opener names the ONLY target the rounds can
    /// strike. Tracker state, because the rounds arrive in later chunks; the
    /// end line also prints when the target dies mid-assault.
    pub fn handle_assault(&mut self, ctx: &LineCtx, line: &ChunkLine) {
        if let Some(a) = assault_start(line) {
            self.tracker.active_assault = Some(ActiveAssault {
                name: a.name,
                target: ctx.line_target.clone(),
            });
        } else if let Some(open) = &self.tracker.active_assault
            && assault_end(line) == Some(open.name)
        {
            self.tracker.active_assault = None;
        }
    }
}
