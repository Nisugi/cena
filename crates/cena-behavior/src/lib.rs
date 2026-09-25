//! cena-behavior
//!
//! Curated Rust behaviors. **No embedded scripting language** -- no Lua, no
//! Rhai, no DSL (CLAUDE.md, settled). Automation is Rust that a user
//! configures with data, not Rust that runs a user's code.
//!
//! Two exist: [`sync()`](sync::sync) and [`travel()`](travel::travel), the
//! latter with a [`Desk`](travel::Desk) that runs it from a typed `;go2`.
//! Both end with a [`BehaviorError`] when the session, not the behavior,
//! decided.
//!
//! Hunt, M6's behavior (`plan/30`), is being built in [`hunt`]: the profile
//! it runs on, the guard vocabulary and the bigshot importer are there; the
//! engine that runs a profile follows (`plan/30` §7, M6b).
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
pub mod heal;
pub mod hunt;
pub mod keep;
pub mod loot;
pub mod spellcaster;
pub mod stance;
pub mod sync;
pub mod town;
pub mod travel;
pub mod waggle;
pub mod watchdog;

pub use error::BehaviorError;
pub use sync::{MAX_AGE, SYNC_DEADLINE, commands_for, plan, sync};
pub use watchdog::{BEHAVIOR_WATCHDOG, Heartbeat, Watched, watch};
