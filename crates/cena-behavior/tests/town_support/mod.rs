//! What the selling round's tests share: a character with a gem sack and
//! a backpack on the stow list, hands, the shops' rooms, and the facts a
//! shop answers with.
//!
//! A module rather than a test target (`tests/<name>/mod.rs` is not one).
//! Split out when Stage 4 took `town_plan.rs` past its line cap (`plan/05`
//! Rule 4.1: move code down, do not raise the cap).

#![allow(dead_code, reason = "shared test helpers: each file uses a subset")]

mod helpers;

pub use helpers::*;
