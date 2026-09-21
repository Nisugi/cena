//! Reading a map file. The layout is documented on the parent module.
//!
//! Nothing here panics and nothing trusts the file: a truncated, corrupt or
//! hostile input is a [`LoadError`].

use super::wire::{LoadError, MAGIC, NONE, Reader, VERSION};
use crate::exit::{Cost, Crossing, Exit, ExitKind, ShapeId};
use crate::map::Map;
use crate::room::{Image, Room, RoomId, Uid};

/// The smallest a room can be: an id, nine empty lists or absent references,
/// a flag byte, an image byte and two empty counts.
const MIN_ROOM: usize = 4 + 4 + 6 * 4 + 3 * 4 + 1 + 1 + 4 + 4;
/// The smallest an exit can be: destination, kind, an empty crossing, no cost.
const MIN_EXIT: usize = 4 + 4 + 8 + 1;

/// Decode a map file.
///
/// # Errors
///
/// [`LoadError`] for anything that is not a well-formed map file of this
/// build's version. A crossing or cost of a kind this build does not know is
/// **not** an error; see the parent module's rule 1.
pub fn decode(bytes: &[u8]) -> Result<Map, LoadError> {
    let mut r = Reader::new(bytes);
    if r.take(MAGIC.len()).ok() != Some(MAGIC.as_slice()) {
        return Err(LoadError::BadMagic);
    }
    let found = r.u32()?;
    if found != VERSION {
        return Err(LoadError::UnsupportedVersion {
            found,
            supported: VERSION,
        });
    }

    let mut strings = Vec::new();
    for _ in 0..r.count(4)? {
        let at = r.pos();
        let text = std::str::from_utf8(r.blob()?).map_err(|_| LoadError::BadUtf8 { at })?;
        strings.push(text);
    }
    let strings = Strings(strings);

    let mut rooms = Vec::new();
    for _ in 0..r.count(MIN_ROOM)? {
        rooms.push(read_room(&mut r, &strings)?);
    }
    if !r.is_at_end() {
        return Err(LoadError::TrailingBytes { at: r.pos() });
    }
    Map::from_rooms(rooms).map_err(LoadError::Duplicate)
}

/// The string table, borrowed from the file.
struct Strings<'a>(Vec<&'a str>);

impl<'a> Strings<'a> {
    fn get(&self, r: &mut Reader<'_>) -> Result<&'a str, LoadError> {
        let at = r.pos();
        let reference = r.u32()?;
        self.resolve(reference, at)
    }

    fn optional(&self, r: &mut Reader<'_>) -> Result<Option<String>, LoadError> {
        let at = r.pos();
        match r.u32()? {
            NONE => Ok(None),
            reference => self
                .resolve(reference, at)
                .map(|text| Some(text.to_owned())),
        }
    }

    fn resolve(&self, reference: u32, at: usize) -> Result<&'a str, LoadError> {
        usize::try_from(reference)
            .ok()
            .and_then(|index| self.0.get(index).copied())
            .ok_or(LoadError::BadStringRef { reference, at })
    }

    fn list(&self, r: &mut Reader<'_>) -> Result<Vec<String>, LoadError> {
        let mut list = Vec::new();
        for _ in 0..r.count(4)? {
            list.push(self.get(r)?.to_owned());
        }
        Ok(list)
    }

    /// A string reference that is the whole of a blob.
    fn string_in(&self, blob: &[u8], at: usize) -> Result<&'a str, LoadError> {
        let bytes: [u8; 4] = blob.try_into().map_err(|_| LoadError::Truncated { at })?;
        self.resolve(u32::from_le_bytes(bytes), at)
    }
}

fn read_room(r: &mut Reader<'_>, strings: &Strings<'_>) -> Result<Room, LoadError> {
    let id = RoomId(r.u32()?);
    let mut uid = Vec::new();
    for _ in 0..r.count(8)? {
        uid.push(Uid(r.i64()?));
    }
    let title = strings.list(r)?;
    let description = strings.list(r)?;
    let paths = strings.list(r)?;
    let unique_loot = strings.list(r)?;
    let tags = strings.list(r)?;
    let meta = strings.list(r)?;
    let location = strings.optional(r)?;
    let climate = strings.optional(r)?;
    let terrain = strings.optional(r)?;
    let flags = r.u8()?;
    let image = if r.u8()? == 0 {
        None
    } else {
        let file = strings.get(r)?.to_owned();
        Some(Image {
            file,
            rect: [r.i32()?, r.i32()?, r.i32()?, r.i32()?],
        })
    };
    let mut exits = Vec::new();
    for _ in 0..r.count(MIN_EXIT)? {
        exits.push(read_exit(r, strings)?);
    }
    // Extensions this build does not know: skipped by length (rule 1).
    for _ in 0..r.count(8)? {
        r.u32()?;
        r.blob()?;
    }
    Ok(Room {
        id,
        uid,
        title,
        description,
        paths,
        location,
        location_unknowable: flags & 1 != 0,
        check_location: flags & 2 != 0,
        unique_loot,
        climate,
        terrain,
        tags,
        meta,
        image,
        exits,
    })
}

fn read_exit(r: &mut Reader<'_>, strings: &Strings<'_>) -> Result<Exit, LoadError> {
    let to = RoomId(r.u32()?);
    let kind = ExitKind::from_name(strings.get(r)?);

    let name = strings.get(r)?;
    let at = r.pos();
    let blob = r.blob()?;
    let crossing = match name {
        Crossing::COMMAND => Crossing::Command(strings.string_in(blob, at)?.to_owned()),
        Crossing::UNPORTED => Crossing::Unported(ShapeId(strings.string_in(blob, at)?.to_owned())),
        // A step list this build cannot read -- a step or a condition added
        // since -- is an unknown crossing, not a bad file (rule 1).
        Crossing::STEPS => serde_json::from_slice(blob)
            .map_or_else(|_| Crossing::Unknown(name.to_owned()), Crossing::Steps),
        Crossing::ROUTINE => serde_json::from_slice(blob)
            .map_or_else(|_| Crossing::Unknown(name.to_owned()), Crossing::Routine),
        Crossing::PASS => Crossing::PassThrough(crate::exit::Pass),
        other => Crossing::Unknown(other.to_owned()),
    };

    let cost = if r.u8()? == 0 {
        None
    } else {
        let name = strings.get(r)?;
        let at = r.pos();
        let blob = r.blob()?;
        Some(match name {
            Cost::FIXED => {
                let bytes: [u8; 8] = blob.try_into().map_err(|_| LoadError::Truncated { at })?;
                Cost::Fixed(f64::from_le_bytes(bytes))
            }
            Cost::UNPORTED => Cost::Unported {
                unported: ShapeId(strings.string_in(blob, at)?.to_owned()),
            },
            // JSON, for the reason `steps` is: a condition added later fails to
            // parse and the cost is unknown -- impassable -- not a bad file.
            Cost::GATED => match serde_json::from_slice(blob) {
                Ok(gated @ Cost::Gated { .. }) => gated,
                _ => Cost::Unknown(name.to_owned()),
            },
            Cost::TABLE => match serde_json::from_slice(blob) {
                Ok(table @ Cost::Table { .. }) => table,
                _ => Cost::Unknown(name.to_owned()),
            },
            other => Cost::Unknown(other.to_owned()),
        })
    };
    Ok(Exit {
        to,
        kind,
        crossing,
        cost,
    })
}
