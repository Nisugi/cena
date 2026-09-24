//! What the web tests share. A module, not a test target
//! (`tests/<name>/mod.rs` is not one); a facade (`plan/05` Rule 4.4) over
//! `helpers.rs`.

// Each test file uses its own subset; the rest is dead code to that file's
// compiler, and a warning there would fail `-D warnings`.
#![allow(dead_code, reason = "a shared test harness: each file uses a subset")]

mod helpers;

pub use helpers::*;
