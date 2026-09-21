//! Arms for slice d of the long tail (`research/mapdb-inventory/tail/slice_d_costs.tsv`).
//!
//! Same rules as every arm (`super`): the upstream script verbatim, with
//! holes; an exact match or none. This file's tests live at its foot.

use cena_map::Cost;

/// The gate for a cost script in this slice, if an arm here knows it.
pub(super) fn cost(script: &str) -> Option<Cost> {
    let _ = script;
    None
}
