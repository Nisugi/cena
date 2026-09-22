//! The map file Hydra loads: every room in one versioned binary.
//!
//! `plan/21` §3a. Written by `cena-map-combine` (now in `Nisugi/hydra-mapdb`,
//! which depends on this crate as a git dependency) from the converter's
//! per-room JSON, read by the client. Both compile against this module, so
//! there is one definition of the format and neither side can drift.
//!
//! # The three rules the format exists to keep
//!
//! 1. **An older client must load a newer map.** The map updates without a
//!    client release, so a map built after a primitive was added *will* meet a
//!    client built before. A crossing and a cost are therefore stored as a
//!    **name and a length-prefixed blob**, never as an enum discriminant: a name
//!    this build does not know becomes [`Crossing::Unknown`] /
//!    [`Cost::Unknown`] -- impassable -- and its blob is skipped by length. Each
//!    room also ends in a list of named **extensions**, skipped the same way,
//!    which is how layout and floors (`plan/21` §3e) will arrive without
//!    breaking anyone.
//! 2. **A breaking change is refused, loudly.** The header carries a version; a
//!    file with another version is [`LoadError::UnsupportedVersion`], never
//!    misread.
//! 3. **One vocabulary.** The wire names are constants on the types themselves
//!    ([`Crossing::COMMAND`], [`Cost::FIXED`], [`ExitKind::name`]), used by both
//!    [`encode`] and [`decode`].
//!
//! # Layout (version 1, little-endian)
//!
//! ```text
//! magic     8 bytes  "HYDRAMAP"
//! version   u32
//! strings   u32 count, then each: u32 length, UTF-8 bytes
//! rooms     u32 count, then each room:
//!   id u32
//!   uid                u32 count, i64 each
//!   title, description, paths, unique_loot, tags, meta
//!                      each: u32 count, u32 string refs
//!   location, climate, terrain      u32 string ref, or NONE
//!   flags u8           bit 0 location_unknowable, bit 1 check_location
//!   image              u8 present; then u32 file ref, four i32
//!   exits              u32 count, then each:
//!     to u32, kind u32 (string ref to its name)
//!     crossing         u32 name ref, u32 blob length, blob
//!     cost             u8 present; then u32 name ref, u32 blob length, blob
//!   extensions         u32 count, then each: u32 name ref, u32 length, blob
//! ```
//!
//! # Extensions in use (all at version 1)
//!
//! Layout corrections arrive here rather than as fixed fields, which is what
//! rule 1 reserved the slot for -- an extension is written only when a room has
//! something to say, so a map with no corrections is byte-identical to one
//! built before they existed.
//!
//! | name | blob | carries |
//! |---|---|---|
//! | `sheet` | two string refs, either `NONE` | `Room::map` (the plate slug) and `Room::area` |
//! | `placement` | `i64` anchor uid, `i32` dx, `i32` dy | `Room::placement` |
//! | `dirto` | `u32` count, then `u32` destination id + `u32` name ref | `Exit::dirto`, keyed by destination |
//!
//! **`dirto` is a room extension although it is a per-EDGE fact.** The exit
//! record ends at `cost` and has no extension slot of its own, so a field
//! appended there would be read as a malformed exit by every client built
//! before it. The bearings travel together on the room and are matched to
//! their exits by `to`.
//!
//! **The plate registry is NOT in the file.** `Map::sheets` maps a slug to a
//! display name and area, and carrying it would need a file-level section
//! after the rooms -- which `decode` refuses as
//! [`LoadError::TrailingBytes`], so it would mean `VERSION = 2`. It does not
//! need to: everything a client needs in order to *draw* a room is on the
//! room, and the registry holds build-side display names. A slug with no
//! entry is a validation error for whoever builds the file.
//!
//! Blobs by name: `cmd` and `unported` are one string ref; `fixed` is an f64;
//! `pass` is empty; **`steps`, `routine`, `gated` and `table` are JSON**, the same text the per-room file
//! holds. JSON inside a binary is deliberate: a step or a condition added by a
//! later build fails to parse here, which makes the exit an unknown crossing
//! -- rule 1 -- with no second versioning scheme to maintain. Scripted exits
//! are under a tenth of the map, so the size is not the consideration it
//! would be for plain commands.
//!
//! Strings are interned in one table because they repeat enormously: a command
//! like `north`, a climate, a location name, a tag.
//!
//! Decoding never panics and never trusts a count: every read is bounds-checked
//! and a count larger than the bytes that remain is [`LoadError::Truncated`]
//! before anything is allocated for it.
//!
//! [`Crossing::Unknown`]: crate::Crossing::Unknown
//! [`Cost::Unknown`]: crate::Cost::Unknown
//! [`Crossing::COMMAND`]: crate::Crossing::COMMAND
//! [`Cost::FIXED`]: crate::Cost::FIXED
//! [`ExitKind::name`]: crate::ExitKind::name

mod decode;
mod encode;
mod wire;

pub use decode::decode;
pub use encode::encode;
pub use wire::{EncodeError, LoadError, MAGIC, VERSION};
