//! cena-model: typed game state, events, and game data.
//!
//! `plan/12:77`. Two halves: [`state`] is the typed game state a session folds
//! frames into, and [`crit`] is the game-data half, ported from Lich per
//! `plan/13` §4a.
//!
//! > **`GameState` moved here from `cena-session` 2026-09-18 (author's call).**
//! > It had been built in the session, which left `cena-model` in the
//! > dependency chain `protocol -> model -> session` **carrying no traffic**:
//! > `cena-model` used nothing at all from `cena-protocol`, so the edge was
//! > declared and unused. That is why `cena-session` needing `cena-protocol`
//! > directly felt like a layer skip and was not one -- there was nothing in
//! > `cena-model` to skip.
//! >
//! > Typed game state built from frames is what `plan/12:77` says this crate
//! > is FOR, so the fix was to move the code down rather than to keep routing
//! > around a layer that did no work.

pub mod crit;
pub mod effects;
pub mod state;
pub mod status;

pub use effects::{Effect, Effects};
pub use state::{
    Character, Container, Experience, Found, GameState, Injury, Inventory, MAX_UNKNOWN_TAGS, Room,
    RoomItem, UnknownTag, Where,
};
pub use status::StatusInfo;
