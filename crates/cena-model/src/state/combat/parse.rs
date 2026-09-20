//! The combat state machine over one chunk: Lich's `parse_events`.
//!
//! `processor.rb:361-1786`, 1,470 lines of Ruby that turn a prompt-bounded
//! chunk into attack events. The port keeps its shape -- one pass over the
//! lines, the same branches in the same order, the same names for the same
//! state -- because that order IS the behaviour: the status handler runs
//! before the flare branch so a flare-and-status line files on the flare it
//! announces; the switcher runs before the outcome branch so an outcome
//! lands on the target it names; the attack branch runs last so a same-line
//! outcome belongs to the event the line opens. Every reordering Lich tried
//! is recorded there as a bug, and the citations travel with the branches.
//!
//! Each branch is a method on [`cursor::Cursor`] in its own module, split
//! by what the branch handles rather than by size:
//!
//! | Module | `processor.rb` | Handles |
//! |---|---|---|
//! | [`cursor`] | the ~26 locals | the working state, the pool, the predicates |
//! | [`facts`] | 520-661 | statuses, spell losses, UCS |
//! | [`flares`] | 663-816 | flare announces, sequence and assault brackets |
//! | [`switch`] | 818-916 | target switching |
//! | [`rolls`] | 918-1135 | roll lines, outcome lines |
//! | [`attack`] | 1137-1580 | initiations, and every claim they make |
//! | [`damage`] | 1582-1690 | damage, the coup, the crit lookahead |
//! | [`finish`] | 1704-1786 | the hold, the orphan sinks, the emit |
//!
//! # What the port does NOT carry
//!
//! - **Settings.** Every `Tracker.settings[:track_*]` gate is on, and
//!   `include_attack_events` is `true`: the recorder is always subscribed.
//! - **Ingestion provenance** (`source`, `combined_observation_source`).
//!   A Cena chunk belongs to one session's one connection by construction.
//! - **The `<component id=` skip.** Room components never reach the chunk;
//!   they are routed to the room model by the stream layer.
//! - **The registry.** `persist_event`, the death sweep and position
//!   recovery apply these events to creatures; they are the consumer above
//!   this one and arrive with it. `LossCause::Death` is therefore decided
//!   from this chunk's kills alone until then.
//!
//! # One deviation, marked
//!
//! A status def that captures a second-person target (`You are stunned!`
//! through a `(?<target>...)` group) took Lich's name-lookup branch and was
//! dropped by the registry. It is emitted here as `Subject::Us`
//! (`facts.rs`), per Rule 2.2.

mod attack;
mod cursor;
mod damage;
mod facts;
mod finish;
mod flares;
mod rolls;
mod switch;

use crate::state::chunks::{Chunk, ChunkLine};
use crate::state::combat::attack::{AmbushPrefix, AttackLine, RedirectPrefix};
use crate::state::combat::defs::defs;
use crate::state::combat::event::{ChunkFacts, Redirect};
use crate::state::combat::outcome::OutcomeKind;
use crate::state::combat::target::{self, Actor};
use crate::state::combat::tracker::CombatTracker;

use cursor::Cursor;

/// Flares whose announce spawns a fresh own swing of the same attack.
pub const ECHO_FLARES: [&str; 2] = ["mirror_image", "hunters_afterimage"];
/// Flares that release an imbedded spell as a separate attack.
pub const SPELL_RELEASING_FLARES: [&str; 1] = ["weapon_cast"];
/// Effect ticks that are the creature's damage but not our deal unless our
/// own cast of that spell on that victim opened them.
pub const UNOWNED_TICK_ATTACKS: [&str; 6] = [
    "pestilence",
    "web",
    "bleed",
    "rot",
    "spiritual_malady",
    "poison_tick",
];
/// Attacks whose initiation line prints a total the next line repeats.
pub const SUMMARY_DAMAGE_ATTACKS: [&str; 1] = ["spiritual_malady"];
/// Side effects with exactly one hit, after which the interrupted attack
/// resumes.
pub const SINGLE_HIT_ATTACKS: [&str; 1] = ["mount_collapse"];
/// The dispel family, for spell-loss cause attribution.
pub const DISPEL_FLARES: [&str; 4] = ["dispel", "sigil_dispel", "dispel_flux", "sigil_bane"];

/// What one line established before the branches ran.
pub(super) struct LineCtx {
    pub index: usize,
    pub text: String,
    /// ONE attack scan per line, shared by every consumer.
    pub line_attack: Option<AttackLine>,
    /// The line's bolded creature; `None` on an inbound line, whose only
    /// creature link is the attacker.
    pub line_target: Option<Actor>,
    pub line_redirect: Option<RedirectPrefix>,
    /// The id a status applied to on THIS line.
    pub line_status_id: Option<i64>,
    /// An outcome printed ON an initiation line.
    pub same_line_outcome: Option<OutcomeKind>,
    /// This line was a flare announce.
    pub line_flare: bool,
}

/// Parse one chunk into its events and facts.
pub(super) fn parse_chunk(
    tracker: &mut CombatTracker,
    chunk: &Chunk,
    at: Option<u32>,
) -> ChunkFacts {
    let lines: &[ChunkLine] = chunk.lines();
    let mut c = Cursor::new(tracker, lines, at);

    // A bare gesture held over from the previous chunk: re-open it so the
    // spell-result line that begins THIS chunk can supersede it.
    if let Some(mut held) = c.tracker.held_cast.take() {
        held.held = true;
        held.born_line = None;
        let id = c.open(held);
        c.ev_mut(id).root = Some(id);
        c.resume(id);
        c.seeking_damage = true;
    }
    // Pre-flares held over: re-seeded so this chunk's swing claims them.
    let mut held_flares = std::mem::take(&mut c.tracker.held_pre_flares);
    for f in &mut held_flares {
        f.held = true;
    }
    c.pending_flares.append(&mut held_flares);

    for (index, line) in lines.iter().enumerate() {
        let text = line.text();
        if text.trim().is_empty() || defs().is_narration(&text) {
            continue;
        }
        let line_attack = AttackLine::classify(line);
        let inbound_line = line_attack.as_ref().is_some_and(|a| a.inbound);
        let mut ctx = LineCtx {
            index,
            line_target: if inbound_line {
                None
            } else {
                target::bolded_link(line)
            },
            line_redirect: RedirectPrefix::classify(line),
            line_attack,
            line_status_id: None,
            same_line_outcome: None,
            line_flare: false,
            text,
        };

        c.handle_status(&mut ctx, line);
        c.handle_spell_loss(line);
        c.handle_ucs(line);
        c.handle_flare(&mut ctx, line);
        c.handle_sequence(line);
        c.handle_assault(&ctx, line);
        c.handle_target_switch(&ctx);
        c.handle_rolls(&mut ctx, line);

        // Prefixes are modifiers on the attack line that follows, never
        // events of their own: arm and move on.
        let amb = AmbushPrefix::classify(line);
        if let Some(a) = amb.clone() {
            c.pending_ambush = Some(a);
        }
        if let Some(rdr) = ctx.line_redirect.clone() {
            c.handle_redirect(&ctx, rdr);
        }
        let attack = if amb.is_some() || ctx.line_redirect.is_some() {
            None
        } else {
            ctx.line_attack.take()
        };

        if let Some(w) = defs().nock_weapon(&ctx.text) {
            c.nocked = Some(w);
        }
        if let Some(atk) = attack {
            c.handle_attack(&ctx, line, atk);
        } else if c.flare_ctx.is_some() || c.seeking_damage {
            c.handle_damage(&ctx, line);
        } else if let Some(d) = crate::state::combat::damage::DamageLine::classify(line) {
            // Damage while seeking an attack: its initiation had no def, or
            // lived in a prior chunk. Sink it rather than drop the fact.
            c.orphan_hits.push(crate::state::combat::event::Hit {
                damage: d.amount,
                crit: None,
                line: index,
            });
        }

        // The line's status belongs to whichever event now holds that
        // target: flag it so the save predicate counts it as a fact.
        if let (Some(sid), Some(cur)) = (ctx.line_status_id, c.current)
            && c.ev(cur).target.id() == Some(sid)
        {
            c.ev_mut(cur).had_status = true;
        }
    }
    c.finish()
}

impl Cursor<'_> {
    /// A guardian redirect prefix. UAC shape (corpus: 21 of 130): the
    /// announce comes AFTER the attack line and the roll still lands on the
    /// intended victim -- stamp the open attack as unhonored and arm nothing.
    fn handle_redirect(&mut self, ctx: &LineCtx, rdr: RedirectPrefix) {
        let interceptor = ctx
            .line_target
            .clone()
            .unwrap_or_else(|| Actor::unlinked(&rdr.interceptor));
        let unhonored = self.current.filter(|&cur| {
            let e = self.ev(cur);
            let open_noun = e.target.creature().map(|a| {
                a.noun
                    .clone()
                    .unwrap_or_else(|| a.name.rsplit(' ').next().unwrap_or("").to_owned())
            });
            e.redirect.is_none()
                && e.attack_born
                && open_noun.as_deref().is_none_or(|n| n == rdr.intended)
                && e.resolutions.is_empty()
                && e.hits.is_empty()
                && e.outcomes.is_empty()
        });
        let redirect = Redirect {
            interceptor,
            intended: rdr.intended,
            honored: unhonored.is_none(),
        };
        match unhonored {
            Some(cur) => self.ev_mut(cur).redirect = Some(redirect),
            None => self.pending_redirect = Some(redirect),
        }
    }
}
