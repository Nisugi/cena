//! A looping `look`: the behavior the session's own guarantees are tested on.
//!
//! **Test support, not shipped.** It was `cena-behavior`'s first behavior
//! (M1), there to exercise `plan/12` §7.2 criteria 4 and 5 against a live
//! game. M6 retired it from the crate (author, 2026-09-24: *"get rid of any
//! test behaviors, like look"*), but the criteria are properties of the
//! SESSION -- stop latency, interleaving, release -- and still need a
//! behavior that loops forever to be tested on. This is that behavior,
//! unchanged, so every falsification its tests record still applies.
//!
//! A module rather than a test target (`tests/<name>/mod.rs` is not one).
//! A facade (`plan/05` Rule 4.4): the behavior is in `look.rs`.

// Each test file uses its own subset; the rest is dead code to that file's
// compiler, and a warning there would fail `-D warnings`.
#![allow(dead_code, reason = "a shared test harness: each file uses a subset")]

mod look;

pub use look::*;
