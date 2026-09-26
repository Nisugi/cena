//! Heal: eherbs' herb healing as a curated behavior (`plan/36`).
//!
//! The same two layers as the loot and the selling round: a pure planner
//! over the state and the character's heal profile ([`plan::Healer`]), and a
//! thin driver inside the hunt's own (`hunt/drive.rs`, `heal`), so it runs
//! during a rest when the character is hurt, and on its own as `;heal`.
//!
//! [`choose`] is eherbs' order of what to treat next; the herbs themselves,
//! what each treats and where it is sold, are the model's table
//! (`cena_session::herbs`), and how many doses each has left is the model's
//! dose monitor.

pub mod choose;
pub mod kit;
pub mod plan;
pub mod profile;
pub mod reply;
pub mod stock;

pub use choose::{Mode, next_kind};
pub use kit::distill_target;
pub use plan::{Healed, Healer, Step, container_named};
pub use profile::{HealProfile, path};
pub use reply::{Reply, classify};
pub use stock::{Stocked, Stocker, Want};
