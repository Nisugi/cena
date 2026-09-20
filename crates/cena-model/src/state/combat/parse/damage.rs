//! Damage lines, the coup kill line, and the crit lookahead.
//!
//! `processor.rb:1582-1690`. ONE record per landed hit, damage bound to the
//! crit it produced: the binding only exists here, where both are in scope.

use crate::crit::Location;
use crate::state::chunks::ChunkLine;
use crate::state::combat::attack::coup_kill_location;
use crate::state::combat::damage::DamageLine;
use crate::state::combat::event::{Crit, Hit};

use super::cursor::Cursor;
use super::{LineCtx, SINGLE_HIT_ATTACKS};

impl Cursor<'_> {
    /// The damage branch: reached while seeking damage or while a flare
    /// cursor is open (pre-flare damage before any swing).
    pub fn handle_damage(&mut self, ctx: &LineCtx, line: &ChunkLine) {
        // A coup de grace prints no damage line: its success line is the
        // killing blow, recorded as a zero-damage FATAL hit.
        if let Some(cur) = self.current.filter(|&c| self.ev(c).name == "coup_de_grace")
            && let Some(loc) = coup_kill_location(line)
        {
            let location = Location::parse(&loc).unwrap_or(Location::Chest);
            let e = self.ev_mut(cur);
            e.hits.push(Hit {
                damage: 0,
                crit: Some(Crit::coup_de_grace(location, ctx.index)),
                line: ctx.index,
            });
            if let Some(id) = e.target.id() {
                self.chunk_deaths.push(id);
            }
            return;
        }
        let Some(dmg) = DamageLine::classify(line) else {
            return;
        };
        // a one-hit side effect already has its hit: this damage is the
        // interrupted attack's
        if self.flare_ctx.is_none()
            && let (Some(cur), Some(shp)) = (self.current, self.single_hit_parent)
        {
            let e = self.ev(cur);
            if SINGLE_HIT_ATTACKS.contains(&e.name.as_str()) && !e.hits.is_empty() {
                self.save_event(cur);
                self.single_hit_parent = None;
                self.resume(shp);
            }
        }
        let mut hit = Hit {
            damage: dmg.amount,
            crit: None,
            line: ctx.index,
        };
        // Look ahead up to three lines for the crit this damage produced. A
        // second damage line ends the search: holy fire prints "ravaged for
        // 65" then "... 5 points of damage!", and only the 5 carries the crit.
        // Skipped, and recorded as `crit: None`, when no tables were given.
        if let Some(tables) = self.tracker.crit_tables() {
            for offset in 1..=3 {
                let Some(next) = self.lines.get(ctx.index + offset) else {
                    break;
                };
                if DamageLine::classify(next).is_some() {
                    break;
                }
                if let Some(entry) = tables.parse(&next.text()).first() {
                    hit.crit = Some(Crit::from_entry(entry, ctx.index + offset));
                    if entry.fatal {
                        let victim = match self.flare_ctx {
                            Some(fc) => self.flare(fc).target.as_ref().and_then(|a| a.id),
                            None => self.cur().and_then(|e| e.target.id()),
                        };
                        if let Some(id) = victim {
                            self.chunk_deaths.push(id);
                        }
                    }
                    break;
                }
            }
        }
        match (self.flare_ctx, self.current) {
            (Some(fc), _) => self.flare_mut(fc).hits.push(hit),
            (None, Some(cur)) => self.ev_mut(cur).hits.push(hit),
            // Seeking damage with no event: only reachable after a superseded
            // cast left nothing open. Lich would raise here; sink it instead.
            (None, None) => self.orphan_hits.push(hit),
        }
    }
}
