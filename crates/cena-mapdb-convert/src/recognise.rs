//! Upstream scripts this converter knows how to say in steps (`plan/21` §5
//! step 7). This file and [`crate::shape`] are the only places Ruby is read.
//!
//! # An arm is a template, not a parser
//!
//! Each arm is the upstream script **verbatim**, with holes where the edges
//! that share the shape differ. A script matches an arm exactly or it does not
//! match -- there is no Ruby grammar here and no partial understanding. When
//! upstream edits a script by one character the arm stops matching, the exit
//! goes back to `unported`, and the ratchet fails the build. That is the
//! intended failure: loud, and on the converter's side of the pipeline.
//!
//! Arms are added in the order that opens the most rooms
//! (`research/mapdb-inventory/chokepoints.py`), not the order of most edges.

use cena_map::{Action, Cond, Crossing, Step};

/// The steps for an upstream crossing script, if an arm knows it.
#[must_use]
pub fn crossing(script: &str) -> Option<Crossing> {
    icy_path(script)
}

/// Match `script` against literal parts with a hole between each pair, and
/// return what filled the holes. `["a", "b"]` has one hole.
fn holes<'s>(script: &'s str, parts: &[&str]) -> Option<Vec<&'s str>> {
    let (first, rest) = parts.split_first()?;
    let mut remaining = script.strip_prefix(first)?;
    let mut found = Vec::with_capacity(rest.len());
    for (index, part) in rest.iter().enumerate() {
        let at = if index + 1 == rest.len() {
            // The last part must end the script, so a hole cannot swallow a
            // second statement that happens to end the same way.
            remaining.len().checked_sub(part.len())?
        } else {
            remaining.find(part)?
        };
        let (hole, after) = remaining.split_at_checked(at)?;
        found.push(hole);
        remaining = after.strip_prefix(part)?;
    }
    remaining.is_empty().then_some(found)
}

/// A hole that is one quoted word: no quote, no statement separator.
fn is_plain_argument(hole: &str) -> bool {
    !hole.is_empty() && !hole.contains(['\'', '"', ';', '\n', '#'])
}

/// The icy paths: 171 exits in two upstream shapes.
///
/// **What upstream does**, and where this departs from it. The common shape
/// (150) waits four seconds when the walker is heavy or unskilled and not
/// hasted. The other (21, the glacier) waits six on a simpler test, and
/// otherwise casts Sigil of Resolve if it can.
///
/// **One profile setting, `ice_mode`: `run`, `wait` or `auto`** -- go2's
/// values, kept by the author's choice. `run` does nothing special: no cast,
/// no wait, just the move. `wait` always waits. `auto` waits when the shape's
/// own test says the walker is likely to slip.
///
/// **The author's rule: all 171 cast Sigil of Resolve when it is known and
/// affordable -- unless `ice_mode` is `run`.** So both shapes become the same three steps -- cast,
/// wait, move -- each keeping its own wait and its own test for it. The cast
/// comes first and does not depend on the wait: Resolve is what makes the
/// crossing likelier to succeed either way.
///
/// Dropped: the `echo`, which talks to a Lich user; and the glacier shape's
/// reaction to `Rushing heedlessly` (cast Haste, stand, replan) -- a fall is
/// the `move` step's to recover from, as it is on any other exit.
///
/// **A guard that cannot be answered does not fire** (`cena_map::cond`), so a
/// walker whose skills are unknown neither casts nor waits. That errs toward a
/// fall, which the walk recovers from, rather than toward refusing a route.
fn icy_path(script: &str) -> Option<Crossing> {
    const TRAIL: &str = ";e if (UserVars.mapdb_ice_mode == 'wait') or \
        ((UserVars.mapdb_ice_mode != 'run') and ((XMLData.encumbrance_value > 50) or \
        ((Skills.survival < 50) and not Spell['Haste'].active?))); \
        sleep 0.2; echo 'trying not to slip...'; sleep 4; end; move '";
    const GLACIER: &str = ";e \n\t\tresolve=Spell['Sigil of Resolve']\n\t\thaste=Spell['Haste']\n\t\t\
        if UserVars.mapdb_ice_mode == 'wait' || Skills.survival < 50 || \
        XMLData.encumbrance_value >= 50\n\t\t\techo 'trying not to slip...'; sleep 6\n\t\t\
        elsif resolve.known? && resolve.affordable? && !resolve.active?\n\t\t\tresolve.cast\n\t\t\
        end\n\t\tresult = fput '";
    const GLACIER_AFTER: &str = "'\n\t\tif result =~ /^Rushing heedlessly/\n\t\t\t\
        haste.cast if haste.known? && haste.affordable? && !haste.active?\n\t\t\t\
        fput 'stand'\n\t\t\t$go2_restart = true\n\t\tend\n\t";

    // go2's own words, kept (author, 2026-09-20: "more informative" than
    // off/on). The third value, `auto`, is never tested for: it is what is left.
    const RUN: &str = "run";
    const WAIT: &str = "wait";
    let setting = |value: &str| Cond::Setting("ice_mode".to_owned(), value.to_owned());
    let unskilled = Cond::SkillUnder("survival".to_owned(), 50);
    let (direction, pause, slippery) = if let Some(found) = holes(script, &[TRAIL, "'"]) {
        let heavy_or_slow = Cond::Any(vec![
            Cond::EncumbranceOver(50),
            Cond::All(vec![
                unskilled,
                Cond::Not(Box::new(Cond::SpellActive("Haste".to_owned()))),
            ]),
        ]);
        let unless_running = Cond::All(vec![Cond::Not(Box::new(setting(RUN))), heavy_or_slow]);
        let slippery = Cond::Any(vec![setting(WAIT), unless_running]);
        (*found.first()?, 4200, slippery)
    } else {
        let found = holes(script, &[GLACIER, GLACIER_AFTER])?;
        // `>= 50` upstream, and whole percents: over 49.
        let slippery = Cond::Any(vec![setting(WAIT), unskilled, Cond::EncumbranceOver(49)]);
        (*found.first()?, 6000, slippery)
    };
    if !is_plain_argument(direction) {
        return None;
    }

    let resolve = || "Sigil of Resolve".to_owned();
    let can_cast = Cond::All(vec![
        Cond::Not(Box::new(setting(RUN))),
        Cond::SpellKnown(resolve()),
        Cond::SpellAffordable(resolve()),
        Cond::Not(Box::new(Cond::SpellActive(resolve()))),
    ]);
    let step = |action, when| Step { action, when };
    Some(Crossing::Steps(vec![
        step(Action::Cast(resolve()), Some(can_cast)),
        step(Action::Pause(pause), Some(slippery)),
        step(Action::Move(direction.to_owned()), None),
    ]))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn holes_are_exact_at_both_ends() {
        assert_eq!(holes("move 'west'", &["move '", "'"]), Some(vec!["west"]));
        // The last part anchors at the end, so a trailing statement lands IN
        // the hole -- where `is_plain_argument` refuses it.
        let smuggled = holes("move 'west'; fput 'x'", &["move '", "'"]).unwrap();
        assert_eq!(smuggled, ["west'; fput 'x"]);
        assert!(!is_plain_argument(smuggled[0]));
        assert_eq!(holes("xmove 'west'", &["move '", "'"]), None);
        assert_eq!(holes("a=1;b=2;", &["a=", ";b=", ";"]), Some(vec!["1", "2"]));
        assert_eq!(holes("anything", &[]), None);
    }

    #[test]
    fn an_argument_cannot_smuggle_a_second_statement() {
        assert!(is_plain_argument("go bridge"));
        assert!(!is_plain_argument("west'; fput 'quit"));
        assert!(!is_plain_argument(""));
    }
}
