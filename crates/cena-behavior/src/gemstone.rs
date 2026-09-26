//! Game-specific commands, under the namespace `plan/05` Rule 3.4
//! prescribes.
//!
//! The same call `cena-platform/src/gemstone.rs` records for its host table
//! (the author, 2026-09-19): a game-name literal outside `/src/gemstone/`
//! trips `game_names_outside_game_modules_are_flagged`, and the remedy is
//! to move it under the namespace, not to exempt the path or assemble the
//! literal from parts. Here the literal is a game command whose verb is the
//! game's own name.

pub(crate) mod jewel;
