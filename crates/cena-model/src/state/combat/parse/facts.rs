//! The always-on fact handlers: statuses, spell losses, UCS.
//!
//! `processor.rb:520-661`. These run on every line, in or out of combat,
//! before the event branches -- and each yields a [`Fact`] rather than a
//! mutation, because the creature registry is the consumer above this one.

use crate::state::chunks::ChunkLine;
use crate::state::combat::event::{Fact, LossCause, Subject, UcsKind};
use crate::state::combat::flare::FlareLine;
use crate::state::combat::spell_loss::SpellLoss;
use crate::state::combat::status::StatusLine;
use crate::state::combat::target::{self, Actor};
use crate::state::combat::ucs::UcsLine;

use super::LineCtx;
use super::cursor::{Cursor, FlareRef, starts_your_or_you};

impl Cursor<'_> {
    /// Which of the open event's flares a status rides on, 1-based
    /// (`status_flare_seq`, `processor.rb:530-566`).
    fn status_flare_seq(&self, cid: i64, line: &ChunkLine) -> Option<usize> {
        let cur_id = self.current?;
        let cur = self.ev(cur_id);
        // A line that is itself a flare AND a status rides the flare it
        // announces -- the NEXT position, since the flare branch has not
        // appended it yet. Not when the flare is about to resume an
        // interrupted swing: the positions belong to another event.
        if let Some(lf) = FlareLine::classify(line)
            && !(cur.inbound && self.interrupted_own.is_some())
        {
            let lf_id = lf.target.as_ref().and_then(|a| a.id);
            if lf_id == Some(cid) || (lf_id.is_none() && cur.target.id() == Some(cid)) {
                return Some(cur.flares.len() + 1);
            }
        }
        let mut f = None;
        if let Some(fc) = self.flare_ctx {
            let ft = self.flare(fc).target.as_ref();
            if let Some(ft) = ft {
                if ft.id == Some(cid) {
                    f = Some(fc);
                }
            } else if cur.target.id() == Some(cid) {
                f = Some(fc);
            }
        }
        // Only flares that printed AFTER the swing line: a status right
        // after the swing's crit is the swing's, even when a pre-flare named
        // the creature first.
        let idx = match f {
            Some(FlareRef::OnEvent(id, i)) if id == cur_id => Some(i),
            Some(_) => None,
            None => cur
                .flares
                .iter()
                .rposition(|x| !x.pre && x.target.as_ref().and_then(|a| a.id) == Some(cid)),
        };
        idx.map(|i| i + 1)
    }

    /// The event a status rides on -- ONLY when that event touched the
    /// subject (`status_event_for`, `processor.rb:575-582`).
    fn status_event_for(&self, cid: i64, seq: Option<usize>) -> Option<usize> {
        let cur_id = self.current?;
        if seq.is_some() {
            return Some(cur_id);
        }
        let cur = self.ev(cur_id);
        let touched = cur.target.id() == Some(cid)
            || cur.attacker.as_ref().and_then(|a| a.id) == Some(cid)
            || cur
                .flares
                .iter()
                .any(|f| f.target.as_ref().and_then(|a| a.id) == Some(cid));
        touched.then_some(cur_id)
    }

    /// A status line, if this is one.
    pub fn handle_status(&mut self, ctx: &mut LineCtx, line: &ChunkLine) {
        let Some(st) = StatusLine::classify(line) else {
            return;
        };
        let target_named = st.target_text.as_deref().filter(|t| !target::is_self(t));
        if let Some(lt) = ctx.line_target.clone().filter(|a| a.id.is_some()) {
            // ID-based: the most reliable
            let cid = lt.id.unwrap_or_default();
            ctx.line_status_id = Some(cid);
            let seq = self.status_flare_seq(cid, line);
            let event = self.status_event_for(cid, seq);
            self.facts.push(Fact::Status {
                subject: Subject::Creature(lt),
                status: st.status,
                action: st.action,
                event,
                flare_seq: seq,
                line: ctx.index,
            });
        } else if let Some(name) = target_named {
            // Name-based fallback: bound to nothing.
            //
            // Lich takes this branch for ANY captured target, including a
            // second-person "you", and then drops it in the registry's
            // name lookup. A self capture falls through to the 2p branch
            // here instead, so the fact survives (Rule 2.2).
            self.facts.push(Fact::Status {
                subject: Subject::Creature(Actor::unlinked(name)),
                status: st.status,
                action: st.action,
                event: None,
                flare_seq: None,
                line: ctx.index,
            });
        } else if !starts_your_or_you(&ctx.text) {
            // Pronoun status lines ("It is knocked to the ground!") describe
            // the creature we are fighting; during an INBOUND event an
            // active flare's own target still binds (shield-spike knockdown).
            let subject = self
                .current_target
                .clone()
                .filter(|a| a.id.is_some())
                .or_else(|| self.flare_ctx_target().cloned());
            if let Some(subject) = subject {
                let cid = subject.id.unwrap_or_default();
                ctx.line_status_id = subject.id;
                let seq = self.status_flare_seq(cid, line);
                let event = self.status_event_for(cid, seq);
                self.facts.push(Fact::Status {
                    subject: Subject::Creature(subject),
                    status: st.status,
                    action: st.action,
                    event,
                    flare_seq: seq,
                    line: ctx.index,
                });
            }
        } else {
            // 2p: the status is OURS. Never a creature application, but it
            // IS a fact -- inbound attacks stun US.
            self.facts.push(Fact::Status {
                subject: Subject::Us,
                status: st.status,
                action: st.action,
                event: None,
                flare_seq: None,
                line: ctx.index,
            });
        }
    }

    /// A spell wear-off line, with its cause when the chunk shows one.
    pub fn handle_spell_loss(&mut self, line: &ChunkLine) {
        let Some(loss) = SpellLoss::classify(line) else {
            return;
        };
        let id = loss.target.id;
        let cause = if self.chunk_dispels.iter().any(|d| d.is_none() || *d == id) {
            Some(LossCause::Dispel)
        } else if id.is_some_and(|i| self.chunk_deaths.contains(&i)) {
            // Lich also consults the registry's dead flag here; that
            // arrives with the registry.
            Some(LossCause::Death)
        } else {
            None
        };
        self.facts.push(Fact::SpellLoss {
            subject: loss.target,
            spell: loss.spell,
            spell_name: loss.spell_name,
            cause,
        });
    }

    /// A UCS line (`apply_ucs_to_target`, `processor.rb:2304-2339`).
    pub fn handle_ucs(&mut self, line: &ChunkLine) {
        let Some(u) = UcsLine::classify(line) else {
            return;
        };
        let (creature, kind) = match u {
            UcsLine::Position { tier, target } => (target, UcsKind::Position(tier)),
            UcsLine::PositionInbound { tier, attacker } => {
                (attacker, UcsKind::PositionInbound(tier))
            }
            // A tierup names no creature: it is against the current target.
            UcsLine::Tierup { attack } => (self.current_target.clone(), UcsKind::Tierup(attack)),
            UcsLine::SmiteApplied { target } | UcsLine::SmiteHeld { target } => {
                (target, UcsKind::SmiteOn)
            }
            UcsLine::SmiteRemoved { target } => (target, UcsKind::SmiteOff),
        };
        if let Some(creature) = creature.filter(|a| a.id.is_some()) {
            self.facts.push(Fact::Ucs { creature, kind });
        }
    }
}
