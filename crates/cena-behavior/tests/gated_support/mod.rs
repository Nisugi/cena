//! What the hunt's verb tests share: a creature targeted in room 10, the
//! bars and effects a gate reads, and a hunt on a one-routine profile.
//!
//! A module rather than a test target, so `hunt_gated.rs` and
//! `hunt_errands.rs` stand on the same room. Split out when the ported
//! handlers took `hunt_gated.rs` past its line cap (`plan/05` Rule 4.1).

// Each test file uses its own subset of this harness; the rest is dead code
// to that file's compiler, and a warning there would fail `-D warnings`.
#![allow(dead_code, reason = "a shared test harness: each file uses a subset")]

mod room;

pub use room::*;
