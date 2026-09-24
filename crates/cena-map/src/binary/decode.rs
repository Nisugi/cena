//! Reading a map file. The layout is documented on the parent module.
//!
//! Nothing here panics and nothing trusts the file: a truncated, corrupt or
//! hostile input is a [`LoadError`].

use super::wire::{
    EXT_DIRTO, EXT_EXIT_DIRTO, EXT_PLACEMENT, EXT_SHEET, LoadError, MAGIC, NONE, Reader, VERSION,
};
use crate::exit::{Cost, Crossing, Dirto, Exit, ExitKind, ShapeId};
use crate::map::Map;
use crate::room::{Image, Placement, Room, RoomId, Uid};

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
    // **Known extensions are read; the rest are skipped by length (rule 1).**
    // A name this build does not know is not an error -- that is the whole
    // point of the mechanism, and it is what lets a map carry layout to a
    // client built before layout existed.
    let mut map = None;
    let mut area = None;
    let mut placement = None;
    let mut by_destination: Vec<(u32, Option<Dirto>)> = Vec::new();
    let mut by_index: Option<Vec<(u32, Option<Dirto>)>> = None;
    for _ in 0..r.count(8)? {
        let name = strings.get(r)?;
        let blob = r.blob()?;
        match name {
            EXT_SHEET => {
                let mut b = Reader::new(blob);
                map = strings.optional(&mut b)?;
                area = strings.optional(&mut b)?;
            }
            EXT_PLACEMENT => {
                let mut b = Reader::new(blob);
                placement = Some(Placement {
                    anchor: Uid(b.i64()?),
                    dx: b.i32()?,
                    dy: b.i32()?,
                });
            }
            EXT_DIRTO => by_destination = read_bearings(blob, strings)?,
            EXT_EXIT_DIRTO => by_index = Some(read_bearings(blob, strings)?),
            _ => {}
        }
    }
    // **The index-keyed record wins whenever it is present**, because it is
    // the only one that can tell two parallel exits apart (`EXT_EXIT_DIRTO`).
    // The destination-keyed one is read only for a map built before the fix.
    if let Some(bearings) = by_index {
        for (index, dirto) in bearings {
            // An index past the end names no exit: skipped rather than
            // failing the load, as an unknown bearing name is.
            if let Some(exit) = usize::try_from(index).ok().and_then(|i| exits.get_mut(i)) {
                exit.dirto = dirto;
            }
        }
    } else {
        for exit in &mut exits {
            if let Some((_, dirto)) = by_destination.iter().find(|(to, _)| *to == exit.to.0) {
                exit.dirto = *dirto;
            }
        }
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
        map,
        area,
        placement,
    })
}

/// A bearings blob: a count, then a `u32` key and a bearing name ref each. The
/// key is an exit index or a destination id, by which extension it came in.
fn read_bearings(
    blob: &[u8],
    strings: &Strings<'_>,
) -> Result<Vec<(u32, Option<Dirto>)>, LoadError> {
    let mut b = Reader::new(blob);
    let mut bearings = Vec::new();
    for _ in 0..b.count(8)? {
        let key = b.u32()?;
        // An unknown bearing name reads as absent rather than failing the
        // load: rule 1 again.
        bearings.push((key, Dirto::from_name(strings.get(&mut b)?)));
    }
    Ok(bearings)
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
            Cost::HASTED => match serde_json::from_slice(blob) {
                Ok(hasted @ Cost::Hasted { .. }) => hasted,
                _ => Cost::Unknown(name.to_owned()),
            },
            Cost::LADDER => match serde_json::from_slice(blob) {
                Ok(ladder @ Cost::Ladder { .. }) => ladder,
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
        // Filled by the room's `dirto` extension, which is where a per-edge
        // bearing has to live: the exit record has no extension slot.
        dirto: None,
    })
}

#[cfg(test)]
mod tests {
    use super::super::wire::{EncodeError, Writer};
    use super::*;

    /// A one-room file whose bearings arrive under `name`, keyed by `key`.
    ///
    /// Written by hand because the encoder no longer writes the legacy
    /// extension, and a map built before the fix is exactly what this checks.
    fn file_with(name: &str, key: u32) -> Result<Vec<u8>, EncodeError> {
        let mut w = Writer::default();
        w.len(1)?; // rooms
        w.u32(7); // id
        for _ in 0..7 {
            w.len(0)?; // uid, then the six string lists
        }
        for _ in 0..3 {
            w.u32(NONE); // location, climate, terrain
        }
        w.u8(0); // flags
        w.u8(0); // no image
        w.len(2)?; // exits: a gate to room 8, then a door to room 9
        for (to, command) in [(8, "go gate"), (9, "go door")] {
            w.u32(to);
            w.string(ExitKind::Cardinal.name())?;
            let reference = w.intern(command)?;
            w.named(Crossing::COMMAND, &reference.to_le_bytes())?;
            w.u8(0); // no cost
        }
        w.len(1)?; // extensions
        w.extension(name, |w| {
            w.len(1)?;
            w.u32(key);
            w.string(Dirto::North.name())
        })?;
        w.finish()
    }

    #[test]
    fn a_map_built_before_exit_indices_still_loads_its_bearings() {
        // The legacy extension is keyed by destination: room 9 is the door.
        let map = decode(&file_with(EXT_DIRTO, 9).unwrap()).unwrap();
        let bearings: Vec<_> = map
            .room(RoomId(7))
            .unwrap()
            .exits
            .iter()
            .map(|e| e.dirto)
            .collect();
        assert_eq!(bearings, [None, Some(Dirto::North)]);
    }

    #[test]
    fn the_new_extension_is_keyed_by_position_and_ignores_a_stray_index() {
        // Index 0 is the gate -- whose destination, 8, the old key would have
        // matched against too. The index is the only key that names one edge.
        let map = decode(&file_with(EXT_EXIT_DIRTO, 0).unwrap()).unwrap();
        let bearings: Vec<_> = map
            .room(RoomId(7))
            .unwrap()
            .exits
            .iter()
            .map(|e| e.dirto)
            .collect();
        assert_eq!(bearings, [Some(Dirto::North), None]);
        // An index past the end names no exit: skipped, not a failed load.
        let map = decode(&file_with(EXT_EXIT_DIRTO, 5).unwrap()).unwrap();
        assert!(
            map.room(RoomId(7))
                .unwrap()
                .exits
                .iter()
                .all(|e| e.dirto.is_none())
        );
    }
}
