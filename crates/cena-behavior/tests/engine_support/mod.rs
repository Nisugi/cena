//! What the hunt machine's tests share: the profile, a room with creatures
//! in it as the game states them, and the tick's arguments.
//!
//! A module rather than a test target (`tests/<name>/mod.rs` is not one).
//! Split out of `hunt_engine.rs` when its tests took it past its line cap
//! (`plan/05` Rule 4.1: move code down, do not raise the cap).

// Each test file uses its own subset; the rest is dead code to that file's
// compiler, and a warning there would fail `-D warnings`.
#![allow(dead_code, reason = "a shared test harness: each file uses a subset")]

mod world;

pub use world::*;
