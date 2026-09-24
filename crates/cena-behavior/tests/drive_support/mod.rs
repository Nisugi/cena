//! What the driver's tests share: a real session over a scripted game, a
//! character in room 1 with a broadsword, and the map fixtures more than one
//! file walks.
//!
//! A module rather than a test target (`tests/<name>/mod.rs` is not one), so
//! `travel_drive.rs` and `travel_deeds.rs` walk the same world. Split out when
//! the review's regression tests took `travel_drive.rs` past its line cap
//! (`plan/05` Rule 4.1: move code down, do not raise the cap).

// Each test file uses its own subset of this harness; the rest is dead code
// to that file's compiler, and a warning there would fail `-D warnings`.
#![allow(dead_code, reason = "a shared test harness: each file uses a subset")]

mod harness;

pub use harness::*;
