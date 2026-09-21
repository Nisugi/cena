//! cena-map: the map's vocabulary.
//!
//! `plan/21` §3c and §3f. [`room`] is what a mapped room carries and [`exit`]
//! is what leaving it costs and how it is done. [`map`] is the loaded whole --
//! every room, indexed by id and by uid -- [`binary`] is the one file format
//! it travels in, [`files`] is where the per-room JSON lives before that, and
//! [`locate`] answers which room the character is standing in and [`route`]
//! how to get from there to anywhere else.
//!
//! This crate is the **one definition** every part of the pipeline compiles
//! against: `cena-mapdb-convert` writes these records as JSON,
//! `cena-map-combine` packs them into the binary, and the client loads it.
//!
//! It is pure: no file I/O, no clock, no network. The upstream Ruby never
//! reaches it; a scripted edge that nothing has ported yet arrives here as
//! [`Crossing::Unported`], already reduced to the hash of its shape.

pub mod binary;
pub mod cond;
pub mod exit;
pub mod files;
pub mod locate;
pub mod map;
pub mod room;
pub mod route;
pub mod routine;
pub mod step;

pub use cond::{Cond, Walker};
pub use exit::{Cost, Crossing, Exit, ExitKind, Pass, ShapeId};
pub use locate::{By, Located, Origin, Sighting, title_from_subtitle};
pub use map::{DuplicateRoom, Map};
pub use room::{Image, Room, RoomId, Uid};
pub use route::{Routes, Target, as_converted, priced_for};
pub use routine::Routine;
pub use step::{Action, Step};
