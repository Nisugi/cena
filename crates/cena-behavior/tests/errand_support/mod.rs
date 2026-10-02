//! What the errand tests share: a character set out on one of loot's
//! errands over a scripted game (`plan/61` step 1), and reading what it
//! wrote. A module rather than a test target (`tests/<name>/mod.rs` is not
//! one); split out of `loot_errand.rs` when the selling round's own errands
//! (`plan/61` step 5) got a file of their own. A test file using it declares
//! `drive_support` and `ready` beside it.

#![allow(dead_code, reason = "shared test helpers: each file uses a subset")]

mod harness;

pub use harness::*;
