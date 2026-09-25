//! Town: selling during the rest (`plan/31` Stage 4).
//!
//! eloot's `Sell` module (1,943 lines) as the same two layers the hunt and
//! the loot have: a pure planner over the state and the profile's `[town]`
//! settings ([`plan::Seller`]), and a thin driver inside the hunt's own
//! (`hunt/drive.rs`, `sell`). The author, 2026-09-24: *"sells typically
//! happen during the rest"*, so the round runs on arriving at the resting
//! room, before the rest commands, and ends back there.
//!
//! What the game answers is read once, by the ledger's classifier
//! (`plan/34`): the driver's own fold of the stream queues each prompt's
//! `LootFact`s and hands them to the planner. [`reply`] reads the few lines
//! that are not loot facts.
//!
//! Stage 4a builds the gem shop and the pawnshop; 4b the furrier, the
//! collectibles counter, the Chronomage and the bank; 4c the locksmith pool.

pub mod plan;
pub mod reply;
pub mod settings;

pub use plan::{Seller, Shop, Step};
pub use reply::{Reply, classify};
pub use settings::Town;
