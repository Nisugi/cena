//! The distiller: which dose a Survivalist's Kit's Liquid Extractor is
//! pointed at next (`distill`, `eherbs.lic:3026-3076`; `plan/36` Stage 3).
//!
//! eherbs' rule: a solid herb with no liquid form yet comes first; failing
//! one, the liquid the kit has least of among those it also holds solid.
//! Nothing while the extractor is already working, which the healer learns
//! from `analyze` before asking.

use cena_session::kit::KitHerb;

/// The dose to point the extractor at, by the name the listing gives it;
/// `None` when the kit holds no solid herb.
#[must_use]
pub fn distill_target(listing: &[KitHerb]) -> Option<String> {
    let solids: Vec<&KitHerb> = listing.iter().filter(|h| !h.liquid).collect();
    let liquids: Vec<&KitHerb> = listing.iter().filter(|h| h.liquid).collect();
    if let Some(solid) = solids
        .iter()
        .find(|s| !liquids.iter().any(|l| l.item.text == s.item.text))
    {
        return Some(solid.item.text.clone());
    }
    liquids
        .iter()
        .filter(|l| solids.iter().any(|s| s.item.text == l.item.text))
        .min_by_key(|l| l.count)
        .map(|l| l.item.text.clone())
}
