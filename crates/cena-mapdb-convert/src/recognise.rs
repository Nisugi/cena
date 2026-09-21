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

use cena_map::{Action, Cost, Crossing, Pass, Step};

mod costs;
mod facts;
mod moves;
mod routines;
mod tail_a;
mod tail_b;
mod tail_c;
mod tail_d;
#[cfg(test)]
mod tests;

use costs::{
    only_when_travelling, profession, remembered, setting_or_month, trinket_named, urchins,
};
use moves::{
    arctic_waters, event_transport, hands_free_move, icy_path, inn_table, move_and_forget,
    plain_move, portmaster, put_then_move, puts_then_move, resolve_then_move,
};
use routines::{confluence, minotaur_maze, patrol};

/// The steps for an upstream crossing script, if an arm knows it. `from` is
/// the room the exit leaves and `to` the room it reaches, which some scripts
/// name and some only imply.
#[must_use]
pub fn crossing(script: &str, from: u32, to: u32) -> Option<Crossing> {
    if script == ";e true" {
        return Some(Crossing::PassThrough(Pass));
    }
    icy_path(script)
        .or_else(|| plain_move(script))
        .or_else(|| put_then_move(script))
        .or_else(|| event_transport(script, from))
        .or_else(|| move_and_forget(script))
        .or_else(|| hands_free_move(script))
        .or_else(|| puts_then_move(script))
        .or_else(|| inn_table(script))
        .or_else(|| moves::pedal_boat(script))
        .or_else(|| facts::crossing(script, from))
        .or_else(|| portmaster(script))
        .or_else(|| resolve_then_move(script))
        .or_else(|| arctic_waters(script))
        .or_else(|| confluence(script, to))
        .or_else(|| minotaur_maze(script, to))
        .or_else(|| patrol(script))
        .or_else(|| tail_a::crossing(script, from, to))
        .or_else(|| tail_b::crossing(script, from, to))
        .or_else(|| tail_c::crossing(script, from, to))
}

/// The gate for an upstream cost script, if an arm knows it.
#[must_use]
pub fn cost(script: &str, climate: Option<&str>) -> Option<Cost> {
    profession(script)
        .or_else(|| facts::cost(script, climate))
        .or_else(|| urchins(script))
        .or_else(|| only_when_travelling(script))
        .or_else(|| trinket_named(script))
        .or_else(|| remembered(script))
        .or_else(|| setting_or_month(script))
        .or_else(|| tail_d::cost(script))
}

pub(super) fn always(action: Action) -> Step {
    Step { action, when: None }
}

/// `holes`, trying the template as written and then with `"` for `'`:
/// upstream quotes both ways and means nothing by it.
pub(super) fn quoted<'s>(script: &'s str, parts: &[&str]) -> Option<Vec<&'s str>> {
    holes(script, parts).or_else(|| {
        let doubled: Vec<String> = parts.iter().map(|part| part.replace('\'', "\"")).collect();
        let doubled: Vec<&str> = doubled.iter().map(String::as_str).collect();
        holes(script, &doubled)
    })
}

/// Match `script` against literal parts with a hole between each pair, and
/// return what filled the holes. `["a", "b"]` has one hole.
pub(super) fn holes<'s>(script: &'s str, parts: &[&str]) -> Option<Vec<&'s str>> {
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

/// A hole that is an identifier: a variable's name, a profession.
pub(super) fn is_word(hole: &str) -> bool {
    !hole.is_empty() && hole.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'_')
}

/// A hole that is one quoted word: no quote, no statement separator.
pub(super) fn is_plain_argument(hole: &str) -> bool {
    !hole.is_empty() && !hole.contains(['\'', '"', ';', '\n', '#'])
}
