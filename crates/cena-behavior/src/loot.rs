//! Loot: what a hunt does with a corpse and a floor full of treasure
//! (`plan/31`, the port of eloot's hunt-side share).
//!
//! eloot is 8,029 lines, of which a hunt calls about 1,750: search each
//! corpse, take what the floor holds that is wanted, put each thing in the
//! bag the game's own STOW LIST names, and remember which bags are full.
//! The rest is a settings window and the town (selling, hoarding, banking),
//! which `plan/31` §4 puts in the rest phase, after M6d.
//!
//! | Piece | Module | What it is |
//! |---|---|---|
//! | the profile | [`profile`] | one TOML file per character: what to take, what to leave, which fallbacks |
//! | the importer | [`import`](mod@import) | eloot's `eloot.yaml` in, the profile out, the town keys carried for later |
//! | worth | [`worth`] | is this thing on the floor worth taking: eloot's reject lists and category rules |
//! | the outcome | [`outcome`] | what the game said back to a search, a `loot` or a drag, as a closed set |
//! | the planner | [`plan`] | the next command, given the state and what was learned so far; pure |
//!
//! The driver that sends what the planner says, inside the hunt's authority
//! as a walk runs, is Stage 2 (`plan/31` §4) and is not here yet.

pub mod import;
pub mod outcome;
pub mod plan;
pub mod profile;
mod skin;
pub mod worth;

pub use import::{Import, import};
pub use outcome::{Outcome, classify};
pub use plan::{Left, Memory, Planner, Step};
pub use profile::{LootProfile, Skin, path, remember_unskinnable};
pub use worth::{Verdict, is_special, stow_slot, verdict};
