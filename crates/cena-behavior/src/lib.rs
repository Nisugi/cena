//! cena-behavior
//!
//! Curated Rust behaviors. **No embedded scripting language** -- no Lua, no
//! Rhai, no DSL (CLAUDE.md, settled). Automation is Rust that a user
//! configures with data, not Rust that runs a user's code.
//!
//! Two exist: [`look()`](look::look) and [`sync()`](sync::sync). `plan/12` §7.1 puts "one behavior" in
//! Milestone 1's In column and Hunt/Heal/Travel in the Out column, and Rule -1
//! (`plan/05` §-1) says the rule of three comes before any shared shape. There
//! is deliberately **no `Behavior` trait**: with one implementor it would be
//! exactly the abstraction that rule forbids.
pub mod look;
pub mod sync;

pub use look::{BehaviorError, LOOK_INTERVAL, ROUND_TRIP_DEADLINE, is_room_description, look};
pub use sync::{MAX_AGE, SYNC_DEADLINE, plan, sync};
