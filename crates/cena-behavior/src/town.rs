//! Town: selling during the rest (`plan/31` Stage 4).
//!
//! eloot's `Sell` module (1,943 lines) as the same two layers the hunt and
//! the loot have: a pure planner over the state and the profile's `[town]`
//! settings ([`plan::Seller`]), and a thin driver inside the hunt's own
//! (`hunt/drive/selling.rs`, `sell_round`). The author, 2026-09-24: *"sells
//! typically happen during the rest"*, so the round runs on arriving at the
//! resting room, before the rest commands, and ends back there; `loot sell`
//! and its kin run it by themselves (`plan/61`).
//!
//! What the game answers is read once, by the ledger's classifier
//! (`plan/34`): the driver's own fold of the stream queues each prompt's
//! `LootFact`s and hands them to the planner. [`reply`] reads the few lines
//! that are not loot facts.
//!
//! Stage 4a built the gem shop and the pawnshop; 4b the furrier, the
//! collectibles counter, the Chronomage and the bank; 4c the locksmith pool,
//! which `plan/61` step 5 completed: the worker the map names, a full pool,
//! the bank and back, and a returned box no bag would empty held while the
//! round sells, the shops following what the returns brought.
//! [`goods`] reads what the bags hold and which shop takes it, and
//! [`breakdown`] adds up what the round came to. [`route`] says where each
//! shop is: Mist Harbor's when the profile sells there, and none where the
//! Hinterwilds has none.

pub mod breakdown;
pub mod choice;
pub mod goods;
pub mod plan;
mod pool;
pub mod reply;
pub mod route;
pub mod settings;
mod step;

pub use breakdown::Breakdown;
pub use choice::Choice;
pub use goods::{Shop, is_gold_ring};
pub use plan::{Round, Seller, Step};
pub use pool::{box_in_hand, keeps_box};
pub use reply::{Reply, classify};
pub use settings::Town;
