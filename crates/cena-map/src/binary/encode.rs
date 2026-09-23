//! Writing a [`Map`] as a map file. The layout is documented on the parent
//! module; this follows it field for field.

use super::wire::{EXT_EXIT_DIRTO, EXT_PLACEMENT, EXT_SHEET, EncodeError, NONE, Writer};
use crate::exit::{Cost, Crossing, Exit};
use crate::map::Map;
use crate::room::Room;

/// Encode a map.
///
/// # Errors
///
/// [`EncodeError`] when a list or string is too large for the format's 32-bit
/// counts, or when the map holds a [`Crossing::Unknown`] or [`Cost::Unknown`]
/// -- those exist so a *client* can load what it does not understand, and a
/// tool re-writing one would be laundering data it cannot vouch for.
pub fn encode(map: &Map) -> Result<Vec<u8>, EncodeError> {
    let mut w = Writer::default();
    w.len(map.len())?;
    for room in map.rooms() {
        write_room(&mut w, room)?;
    }
    w.finish()
}

fn write_room(w: &mut Writer, room: &Room) -> Result<(), EncodeError> {
    w.u32(room.id.0);
    w.len(room.uid.len())?;
    for uid in &room.uid {
        w.i64(uid.0);
    }
    for list in [
        &room.title,
        &room.description,
        &room.paths,
        &room.unique_loot,
        &room.tags,
        &room.meta,
    ] {
        w.len(list.len())?;
        for text in list {
            w.string(text)?;
        }
    }
    for optional in [&room.location, &room.climate, &room.terrain] {
        match optional {
            Some(text) => w.string(text)?,
            None => w.u32(NONE),
        }
    }
    w.u8(u8::from(room.location_unknowable) | u8::from(room.check_location) << 1);
    match &room.image {
        Some(image) => {
            w.u8(1);
            w.string(&image.file)?;
            for edge in image.rect {
                w.i32(edge);
            }
        }
        None => w.u8(0),
    }
    w.len(room.exits.len())?;
    for exit in &room.exits {
        write_exit(w, exit)?;
    }
    write_room_extensions(w, room)
}

/// The room's named extensions: **layout arrives here, not as fixed fields.**
///
/// This is what the count written since the format landed was for (rule 1),
/// and why `VERSION` does not move: `decode.rs` has skipped unknown extensions
/// by length since day one, so a client built before these existed loads a
/// corrected map and ignores the layout it cannot use.
///
/// Each extension is a name and a length-prefixed blob. An extension is
/// written **only when it has something to say**, so a map with no corrections
/// is byte-identical to one built before they existed.
fn write_room_extensions(w: &mut Writer, room: &Room) -> Result<(), EncodeError> {
    let placement = room.placement.is_some();
    let sheet = room.map.is_some() || room.area.is_some();
    let bearings = room.exits.iter().filter(|e| e.dirto.is_some()).count();
    w.len(usize::from(placement) + usize::from(sheet) + usize::from(bearings > 0))?;

    // **The blob is written into the main writer, not a separate one.** The
    // string table is file-level, so a nested `Writer` would intern into a
    // table nobody reads; the length is reserved and back-patched instead.
    if sheet {
        w.extension(EXT_SHEET, |w| {
            write_opt_str(w, room.map.as_deref())?;
            write_opt_str(w, room.area.as_deref())
        })?;
    }
    if let Some(placement) = room.placement {
        w.extension(EXT_PLACEMENT, |w| {
            w.i64(placement.anchor.0);
            w.i32(placement.dx);
            w.i32(placement.dy);
            Ok(())
        })?;
    }
    if bearings > 0 {
        // Keyed by the exit's index in this room's list, because the exit
        // record has no extension slot of its own and a destination does not
        // name an edge -- see `EXT_EXIT_DIRTO`. Only exits that state one are
        // written; the rest fall through to their command text.
        w.extension(EXT_EXIT_DIRTO, |w| {
            w.len(bearings)?;
            for (index, exit) in room.exits.iter().enumerate() {
                if let Some(dirto) = exit.dirto {
                    w.len(index)?;
                    w.string(dirto.name())?;
                }
            }
            Ok(())
        })?;
    }
    Ok(())
}

/// A string ref, or [`NONE`]. The blob's own convention, matching the room's.
fn write_opt_str(w: &mut Writer, text: Option<&str>) -> Result<(), EncodeError> {
    match text {
        Some(text) => w.string(text)?,
        None => w.u32(NONE),
    }
    Ok(())
}

fn write_exit(w: &mut Writer, exit: &Exit) -> Result<(), EncodeError> {
    w.u32(exit.to.0);
    w.string(exit.kind.name())?;
    match &exit.crossing {
        Crossing::Command(command) => {
            let reference = w.intern(command)?;
            w.named(Crossing::COMMAND, &reference.to_le_bytes())?;
        }
        Crossing::Unported(shape) => {
            let reference = w.intern(&shape.0)?;
            w.named(Crossing::UNPORTED, &reference.to_le_bytes())?;
        }
        Crossing::Steps(steps) => {
            let json = serde_json::to_vec(steps).map_err(|_| EncodeError)?;
            w.named(Crossing::STEPS, &json)?;
        }
        Crossing::Routine(routine) => {
            let json = serde_json::to_vec(routine).map_err(|_| EncodeError)?;
            w.named(Crossing::ROUTINE, &json)?;
        }
        Crossing::PassThrough(_) => w.named(Crossing::PASS, &[])?,
        Crossing::Unknown(_) => return Err(EncodeError),
    }
    match &exit.cost {
        None => w.u8(0),
        Some(cost) => {
            w.u8(1);
            match cost {
                Cost::Fixed(seconds) => w.named(Cost::FIXED, &seconds.to_le_bytes())?,
                Cost::Unported { unported } => {
                    let reference = w.intern(&unported.0)?;
                    w.named(Cost::UNPORTED, &reference.to_le_bytes())?;
                }
                gated @ Cost::Gated { .. } => {
                    let json = serde_json::to_vec(gated).map_err(|_| EncodeError)?;
                    w.named(Cost::GATED, &json)?;
                }
                hasted @ Cost::Hasted { .. } => {
                    let json = serde_json::to_vec(hasted).map_err(|_| EncodeError)?;
                    w.named(Cost::HASTED, &json)?;
                }
                ladder @ Cost::Ladder { .. } => {
                    let json = serde_json::to_vec(ladder).map_err(|_| EncodeError)?;
                    w.named(Cost::LADDER, &json)?;
                }
                table @ Cost::Table { .. } => {
                    let json = serde_json::to_vec(table).map_err(|_| EncodeError)?;
                    w.named(Cost::TABLE, &json)?;
                }
                Cost::Unknown(_) => return Err(EncodeError),
            }
        }
    }
    Ok(())
}
