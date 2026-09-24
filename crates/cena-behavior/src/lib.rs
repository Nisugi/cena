//! cena-behavior
//!
//! Curated Rust behaviors. **No embedded scripting language** -- no Lua, no
//! Rhai, no DSL (CLAUDE.md, settled). Automation is Rust that a user
//! configures with data, not Rust that runs a user's code.
//!
//! Three exist: [`look()`](look::look), [`sync()`](sync::sync) and
//! [`travel()`](travel::travel), the last with a [`Desk`](travel::Desk) that
//! runs it from a typed `;go2`.
//!
//! # Still no `Behavior` trait, at three
//!
//! This said "two exist" and justified the missing trait by having one
//! implementor -- written before travel landed, and stale since (review,
//! 2026-09-23). Three is where Rule -1 (`plan/05` §-1) **permits** a shared
//! shape; it does not demand one, and the three do not have one:
//!
//! | | takes | ends with |
//! |---|---|---|
//! | `look` | nothing | never, until stopped: `Result<(), _>` |
//! | `sync` | a command list | how many were sent: `Result<usize, _>` |
//! | `travel` | a map, a goal, the travel file and a save callback | a [`Travelled`](travel::Travelled) report, never an `Err` |
//!
//! What they do share -- claim the authority, race every await against the
//! stop token, release on every exit (`plan/12` §4.2-4.3) -- is three lines
//! each, and a trait over it would still leave every signature different.
//! Nothing holds "a behavior" without knowing which one it is: the desk runs
//! travel, and nothing runs `sync` yet. The trait earns its place when
//! something must schedule behaviors it cannot name, which is `plan/12` §8's
//! M6 and not before.
pub mod look;
pub mod sync;
pub mod travel;

pub use look::{BehaviorError, LOOK_INTERVAL, ROUND_TRIP_DEADLINE, is_room_description, look};
pub use sync::{MAX_AGE, SYNC_DEADLINE, plan, sync};
