//! Arms for slice a of the long tail (`research/mapdb-inventory/tail/slice_a.tsv`).
//!
//! Same rules as every arm (`super`): the upstream script verbatim, with
//! holes; an exact match or none. This file's tests live at its foot.

use cena_map::Crossing;

/// The steps for a crossing script in this slice, if an arm here knows it.
pub(super) fn crossing(script: &str, from: u32, to: u32) -> Option<Crossing> {
    let _ = (script, from, to);
    None
}
