//! What the settings menu's tests share: a board of what the window would
//! give the menu, and Nisugi's card and pages.
//!
//! A module rather than a test target (`tests/<name>/mod.rs` is not one), so
//! `settings_menu.rs` and `widget_pages.rs` draw the same board. Split out
//! when a bar page's image took `settings_menu.rs` past its line cap
//! (`plan/05` Rule 4.1: move code down, do not raise the cap).

// Each test file uses its own subset; the rest is dead code to that file's
// compiler, and a warning there would fail `-D warnings`.
#![allow(dead_code, reason = "a shared test board: each file uses a subset")]

mod board;

pub use board::*;
