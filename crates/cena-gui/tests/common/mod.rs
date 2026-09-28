//! What the hub's tests share: a board of what the window would gather, and
//! the cards, roster and streams it starts with.
//!
//! A module rather than a test target (`tests/<name>/mod.rs` is not one), so
//! `hub.rs` and `not_launched.rs` draw the same board. Split out when the
//! Not launched tab's tests would have taken `hub.rs` past its line cap
//! (`plan/05` Rule 4.1: move code down, do not raise the cap).

// Each test file uses its own subset; the rest is dead code to that file's
// compiler, and a warning there would fail `-D warnings`.
#![allow(dead_code, reason = "a shared test board: each file uses a subset")]

mod board;

pub use board::*;
