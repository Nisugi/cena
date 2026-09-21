//! cena-map: the map's vocabulary.
//!
//! `plan/21` §3c and §3f. Two halves: [`room`] is what a mapped room carries,
//! and [`exit`] is what leaving it costs and how it is done.
//!
//! This crate is the **one definition** both sides of the pipeline compile
//! against -- `cena-mapdb-convert` writes these records offline, and the loader
//! (`plan/21` §5 step 3) will read them. It is pure: no file I/O, no clock, no
//! network. The upstream Ruby never reaches it; a scripted edge that nothing
//! has ported yet arrives here as [`Crossing::Unported`], already reduced to
//! the hash of its shape.

pub mod exit;
pub mod room;

pub use exit::{Cost, Crossing, Exit, ExitKind, ShapeId};
pub use room::{Image, Room, RoomId, Uid};
