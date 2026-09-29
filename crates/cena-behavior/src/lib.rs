//! cena-behavior
//!
//! Curated Rust behaviors. **No embedded scripting language** -- no Lua, no
//! Rhai, no DSL (CLAUDE.md, settled). Automation is Rust that a user
//! configures with data, not Rust that runs a user's code.
//!
//! | Module | What it is |
//! |---|---|
//! | [`mod@sync`] | learning a character at login: the commands whose answers the model reads |
//! | [`travel`] | the walk, with a [`Desk`](travel::Desk) that runs it from a typed command |
//! | [`hunt`] | M6's behavior (`plan/30`): the profile, the importer, the engine and its driver |
//! | [`group`] | the rules a group hunts by, pure (`plan/39`) |
//! | [`loot`], [`town`] | eloot: what is taken in the field, and the selling round (`plan/31`) |
//! | [`heal`] | eherbs: healing, and stocking the herb container (`plan/36`) |
//! | [`cast`], [`keep`], [`waggle`], [`spellcaster`] | the spell behaviors (`plan/37`) |
//! | [`batch`] | a command over many things, or many times (`plan/30` §7, M6e) |
//! | [`triggers`] | the one triggers file and its rules (`plan/45`) |
//!
//! Each ends with a [`BehaviorError`] when the session, not the behavior,
//! decided.
//!
//! (CORRECTED 2026-09-29: this said *"Two exist"*, sync and travel, and
//! that the hunt's engine *"follows"*. The list is the `pub mod`s below.)
//!
//! `look`, M1's looping behavior, was retired at M6 (author, 2026-09-24:
//! *"get rid of any test behaviors, like look"*). It lives on in
//! `tests/looping/`, where the session's stop and interleave guarantees are
//! still tested on it.
//!
//! # Still no `Behavior` trait
//!
//! What the behaviors share -- claim the authority, race every await against
//! the stop token, release on every exit (`plan/12` §4.2-4.3) -- is three
//! lines each, and a trait over it would still leave every signature
//! different. Nothing holds "a behavior" without knowing which one it is.
//! Hunt (`plan/30` §3) composes its policies as an enum under one holder of
//! the authority, which is `plan/12` §4.2's shape, not a trait's.
pub mod batch;
pub mod cast;
pub mod error;
pub(crate) mod gemstone;
pub mod group;
pub mod heal;
pub mod hunt;
pub mod keep;
pub mod loot;
pub mod operation;
pub mod settings;
pub mod spellcaster;
pub mod stance;
pub mod sync;
pub mod town;
pub mod travel;
pub mod triggers;
pub mod waggle;
pub mod watchdog;

pub use error::BehaviorError;
pub use sync::{MAX_AGE, SYNC_DEADLINE, commands_for, plan, sync};
pub use watchdog::{BEHAVIOR_WATCHDOG, Heartbeat, Watched, watch};
