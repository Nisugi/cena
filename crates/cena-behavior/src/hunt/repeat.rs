//! Steps that run more than once (`bigshot.lic:3988-3998`): `eachtarget
//! <step>`, the step against every creature worth attacking in turn, and
//! `force <step> till N`, the step again until a roll of N or more.
//!
//! | | bigshot |
//! |---|---|
//! | `eachtarget <step>`: each creature worth attacking, in the room's order: `target #id` unless the game targets it already, then the step against it, its guards read against it; then back to the hunt's own target | `cmd_eachtarget`, `:4199-4209` |
//! | `force <step> till N` (or `until`): the step again, until a result of N or more follows one of the character's own lines, a failure line, 30 s, or the creature gone | `cmd_force`, `:5698-5743` |
//!
//! Both run ahead of the hunt's own targeting, since the sweep targets
//! other creatures and the engine would otherwise take its own back at the
//! next tick. The return to the hunt's target is the engine's ordinary
//! `target #id`, once the sweep is over.
//!
//! The results `force` reads are bigshot's two: a spell's `== +N` and a
//! maneuver's `[SMR result: N` (or `SSR`, `Roll`). A plain attack's roll,
//! `= +N` with one sign, is not one of them, in bigshot or here.
//!
//! Two narrowings, each simpler than bigshot and said here: a swept step is
//! a verb or a game command ([`super::verbs`]), not `ambush` or `wand`; and
//! a forced step that will not go now (cooling, unaffordable) ends the
//! force, where bigshot tries it again for the rest of its 30 s.

use std::collections::VecDeque;

use cena_session::GameState;

use super::engine::{Here, Hunt};
use super::guard::Facts;
use super::profile::Step;
use super::said::Said;
use super::verbs::Go;

/// How long a force runs (`cmd_force`, `Time.now - start > 30`).
const FORCE_FOR: u32 = 30;

/// What bigshot's force gives up on (`cmd_force`'s `failure_regex`).
const FORCE_FAILS: &[&str] = &[
    "As you focus on your magic, your vision swims with a swirling haze of crimson",
    "You do not have enough stamina to attempt this maneuver.",
    "Your magic fizzles ineffectually.",
    "You are stunned.",
    "You are still stunned.",
];

/// The steps running more than once, and what they have left.
#[derive(Debug, Default)]
pub(super) struct Repeats {
    /// `eachtarget`: the creatures still to go, each with the step and
    /// whether it has been targeted.
    sweep: VecDeque<(i64, Step, bool)>,
    /// The creature whose swept step is still sending its lines.
    on: Option<i64>,
    /// `force`: the step, its goal and creature, and when it began.
    forcing: Option<Forcing>,
    /// `resonance`: the spell cast last, not cast twice running.
    pub(super) resonance: Option<u16>,
    /// `hide N`: the tries left after the first (`cmd_hide`, `:6126-6137`).
    hiding: Option<u32>,
}

#[derive(Debug)]
struct Forcing {
    send: String,
    key: String,
    goal: u32,
    target: i64,
    since: Option<u32>,
    done: bool,
}

impl Hunt {
    /// Start `step` repeating, if it is an `eachtarget` or a `force`,
    /// against the hunt's target `target`: whether it was one.
    pub(super) fn repeat(
        &mut self,
        step: &Step,
        state: &GameState,
        target: i64,
        now: Option<u32>,
    ) -> bool {
        let key = step.to_string();
        if let Some(inner) = strip_word(&step.send, "eachtarget") {
            let inner = Step {
                send: inner.to_owned(),
                when: step.when.clone(),
                held: None,
            };
            self.repeats.sweep = self
                .fightable(state)
                .map(|creature| (creature.id, inner.clone(), false))
                .collect();
            self.repeats.on = None;
            self.used.record(&key, Some(target), now);
            return true;
        }
        if let Some((send, goal)) = force(&step.send) {
            self.repeats.forcing = Some(Forcing {
                send: send.to_owned(),
                key: key.clone(),
                goal,
                target,
                since: now,
                done: false,
            });
            self.used.record(&key, Some(target), now);
            return true;
        }
        false
    }

    /// The next line of a sweep or a force, ahead of the hunt's own
    /// targeting; `None` when neither has anything left.
    pub(super) fn repeating(
        &mut self,
        state: &GameState,
        here: Here<'_>,
        now: Option<u32>,
    ) -> Option<Said> {
        if let Some(said) = self.hiding(state) {
            return Some(said);
        }
        if let Some(said) = self.sweeping(state, here, now) {
            return Some(said);
        }
        self.forcing(state, now)
    }

    /// `hide N` sent its first try: up to `tries` more, each in the wander
    /// stance, until hidden.
    pub(super) fn hide_tries(&mut self, tries: u32) {
        self.repeats.hiding = Some(tries);
    }

    fn hiding(&mut self, state: &GameState) -> Option<Said> {
        let left = self.repeats.hiding?;
        if left == 0 || state.status.known().hidden() == Some(true) {
            self.repeats.hiding = None;
            return None;
        }
        if let Some(line) = Self::stance_for(self.profile.stance.wander.as_deref(), state) {
            return Some(Said::Send { line, target: None });
        }
        self.repeats.hiding = Some(left - 1);
        Some(Said::Send {
            line: "hide".to_owned(),
            target: None,
        })
    }

    fn sweeping(&mut self, state: &GameState, here: Here<'_>, now: Option<u32>) -> Option<Said> {
        if let Some(id) = self.repeats.on {
            if let Some(line) = self.followups.pop_front() {
                return Some(Said::Send {
                    line,
                    target: Some(id),
                });
            }
            self.repeats.on = None;
        }
        while let Some((id, step, targeted)) = self.repeats.sweep.pop_front() {
            if !self.fightable(state).any(|creature| creature.id == id) {
                continue;
            }
            if !targeted && state.targeting.current() != Some(id) {
                self.repeats.sweep.push_front((id, step, true));
                return Some(Said::Send {
                    line: format!("target #{id}"),
                    target: Some(id),
                });
            }
            let key = step.to_string();
            let facts = Facts {
                state,
                target: Some(id),
                tags: here.room.map(|_| here.tags),
                used: Some(&self.used),
                step: &key,
            };
            if !step.when.iter().all(|c| c.holds(&facts) == Some(true)) {
                continue;
            }
            match self.verb_step(&step.send, &key, id, state, now) {
                Go::Send(line) => {
                    self.repeats.on = Some(id);
                    return Some(Said::Send {
                        line,
                        target: Some(id),
                    });
                }
                Go::Said(said) => return Some(said),
                Go::Skip => {}
            }
        }
        None
    }

    fn forcing(&mut self, state: &GameState, now: Option<u32>) -> Option<Said> {
        let forcing = self.repeats.forcing.as_ref()?;
        let target = forcing.target;
        let over = forcing.done
            || !self.fightable(state).any(|creature| creature.id == target)
            || forcing
                .since
                .zip(now)
                .is_some_and(|(since, now)| now.saturating_sub(since) > FORCE_FOR);
        if over {
            self.repeats.forcing = None;
            return None;
        }
        if let Some(line) = self.followups.pop_front() {
            return Some(Said::Send {
                line,
                target: Some(target),
            });
        }
        let (send, key) = (forcing.send.clone(), forcing.key.clone());
        match self.verb_step(&send, &key, target, state, now) {
            Go::Send(line) => Some(Said::Send {
                line,
                target: Some(target),
            }),
            Go::Said(said) => Some(said),
            Go::Skip => {
                self.repeats.forcing = None;
                None
            }
        }
    }

    /// What the game answered a forced step with: a result at the goal, or
    /// a failure, ends the force.
    pub(super) fn force_replied(&mut self, lines: &[&str]) {
        let Some(forcing) = self.repeats.forcing.as_mut() else {
            return;
        };
        let failed = lines.iter().any(|line| {
            FORCE_FAILS.iter().any(|fail| line.starts_with(fail))
                || (line.contains(" is lying down -- attempting to ")
                    && line.contains(" would be a rather awkward proposition."))
        });
        let reached = lines.iter().enumerate().any(|(at, line)| {
            line.starts_with("You")
                && lines
                    .iter()
                    .skip(at)
                    .take(4)
                    .find_map(|line| result(line))
                    .is_some_and(|roll| roll >= forcing.goal)
        });
        if failed || reached {
            forcing.done = true;
        }
    }

    /// The hunt's target went, or the link did: nothing repeats.
    pub(super) fn repeats_gone(&mut self) {
        let resonance = self.repeats.resonance;
        self.repeats = Repeats {
            resonance,
            ..Repeats::default()
        };
    }
}

/// `text` after its first word, when that word is `word`.
fn strip_word<'a>(text: &'a str, word: &str) -> Option<&'a str> {
    let (first, rest) = text.trim().split_once(char::is_whitespace)?;
    (first.eq_ignore_ascii_case(word) && !rest.trim().is_empty()).then(|| rest.trim())
}

/// `force <step> till N` (or `until`): the step and N.
fn force(send: &str) -> Option<(&str, u32)> {
    let rest = strip_word(send, "force")?;
    let lower = rest.to_ascii_lowercase();
    let at = lower.rfind(" till ").or_else(|| lower.rfind(" until "))?;
    let (step, goal) = rest.split_at(at);
    let goal = goal.split_whitespace().nth(1)?.parse().ok()?;
    let step = step.trim();
    (!step.is_empty()).then_some((step, goal))
}

/// A result bigshot's force reads: `== +N`, or `[SMR result: N` and its
/// kin.
fn result(line: &str) -> Option<u32> {
    let digits = |from: &str| -> Option<u32> {
        let end = from
            .find(|c: char| !c.is_ascii_digit())
            .unwrap_or(from.len());
        from.get(..end)?.parse().ok()
    };
    if let Some(at) = line.find("== +") {
        return digits(line.get(at + 4..)?);
    }
    ["[Roll result: ", "[SMR result: ", "[SSR result: "]
        .iter()
        .find_map(|head| line.strip_prefix(head))
        .and_then(digits)
}
