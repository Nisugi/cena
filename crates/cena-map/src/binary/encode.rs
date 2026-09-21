//! Writing a [`Map`] as a map file. The layout is documented on the parent
//! module; this follows it field for field.

use super::wire::{EncodeError, NONE, Writer};
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
    // Extensions: none yet. The count is written so that the first build to
    // add one does not need a new format version (rule 1).
    w.len(0)
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
