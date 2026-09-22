//! The loaded map: every room, and the two indexes `plan/21` §3d decides on.
//!
//! One `Map` is built once and shared by every session in the process; nothing
//! here is mutable after construction.

use std::collections::{BTreeMap, HashMap};
use std::fmt;

use crate::room::{Room, RoomId, Uid};

/// Two rooms claimed the same id. The id names the graph node, so this is not
/// a map.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DuplicateRoom(pub RoomId);

impl fmt::Display for DuplicateRoom {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "room id {} appears more than once", self.0.0)
    }
}

impl std::error::Error for DuplicateRoom {}

/// Every room, indexed.
#[derive(Debug, Clone, PartialEq)]
pub struct Map {
    rooms: Vec<Room>,
    index_of: HashMap<RoomId, usize>,
    uids: HashMap<Uid, Vec<RoomId>>,
    sheets: BTreeMap<String, Sheet>,
}

/// What a plate slug names: a grid a room can be drawn on.
///
/// `Room::map` carries only the slug, because a slug repeats across every room
/// on the plate and a name does not belong on each of them. This is the
/// registry those slugs point into.
///
/// # NOT carried in the binary, and that is a format constraint
///
/// The map file has no file-level section after its rooms, and `decode`
/// refuses trailing bytes (`LoadError::TrailingBytes`) -- so appending one
/// would make every existing client reject the whole file rather than skip
/// what it does not know. Carrying the registry would mean `VERSION = 2`.
///
/// It does not need to. **What a client needs in order to draw a room -- its
/// plate slug and its area -- is on the room**, in the `sheet` extension, and
/// that slot already existed. What is only in the registry is the plate's
/// display *name*, which is build-side information: a slug with no entry is a
/// validation error for whoever builds the file, not a load error for whoever
/// reads it.
///
/// So this survives JSON round-trips and is attached with
/// [`Map::with_sheets`]; it is deliberately absent from `encode`/`decode`.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Sheet {
    /// What to call the plate when it is shown to someone.
    pub name: String,
    /// The area this plate is a sheet of, when it is a sheet of one.
    ///
    /// **A plate is a grid, not a place** -- `Room::area` is what says where a
    /// room *is*. This says what the plate as a whole belongs to, which is not
    /// always answerable: a plate can hold rooms of several areas.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub area: Option<String>,
}

impl Map {
    /// Index a set of rooms. They are kept in id order.
    ///
    /// # Errors
    ///
    /// [`DuplicateRoom`] when two rooms share an id.
    pub fn from_rooms(mut rooms: Vec<Room>) -> Result<Map, DuplicateRoom> {
        rooms.sort_by_key(|room| room.id);
        let mut index_of = HashMap::with_capacity(rooms.len());
        let mut uids: HashMap<Uid, Vec<RoomId>> = HashMap::new();
        for (index, room) in rooms.iter().enumerate() {
            if index_of.insert(room.id, index).is_some() {
                return Err(DuplicateRoom(room.id));
            }
            for uid in &room.uid {
                uids.entry(*uid).or_default().push(room.id);
            }
        }
        Ok(Map {
            rooms,
            index_of,
            uids,
            sheets: BTreeMap::new(),
        })
    }

    /// Attach the plate registry.
    ///
    /// **A builder rather than a parameter on [`Self::from_rooms`]**, and
    /// deliberately: most maps have no plates, ten call sites construct a map
    /// without one, and a required argument that is almost always empty is a
    /// worse interface than a method that says what it adds.
    #[must_use]
    pub fn with_sheets(mut self, sheets: BTreeMap<String, Sheet>) -> Self {
        self.sheets = sheets;
        self
    }

    /// What a plate slug names, if the registry knows it.
    ///
    /// **`None` is not a load error.** A slug with no entry is a validation
    /// failure for whoever *builds* the file; a client that meets one draws
    /// the room without a plate name rather than refusing the map.
    #[must_use]
    pub fn sheet(&self, slug: &str) -> Option<&Sheet> {
        self.sheets.get(slug)
    }

    /// Every plate, by slug.
    pub fn sheets(&self) -> impl Iterator<Item = (&str, &Sheet)> {
        self.sheets
            .iter()
            .map(|(slug, sheet)| (slug.as_str(), sheet))
    }

    /// The room with this id.
    #[must_use]
    pub fn room(&self, id: RoomId) -> Option<&Room> {
        self.index_of
            .get(&id)
            .and_then(|index| self.rooms.get(*index))
    }

    /// Where a room sits in [`Self::rooms`], for searches that keep their
    /// working state in arrays beside it.
    pub(crate) fn position(&self, id: RoomId) -> Option<usize> {
        self.index_of.get(&id).copied()
    }

    /// Every room the game might mean by this number, in id order.
    ///
    /// **A slice, not an `Option`**, because the relation is many-to-many
    /// (`plan/21` §3d): none for a number the map has never seen, one in the
    /// common case, and up to six for a morphing room -- which the caller then
    /// tells apart by adjacency to where the character just was.
    #[must_use]
    pub fn ids_for_uid(&self, uid: Uid) -> &[RoomId] {
        self.uids.get(&uid).map_or(&[], Vec::as_slice)
    }

    /// Every room, in id order.
    #[must_use]
    pub fn rooms(&self) -> &[Room] {
        &self.rooms
    }

    /// How many rooms.
    #[must_use]
    pub fn len(&self) -> usize {
        self.rooms.len()
    }

    /// Whether there are no rooms.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.rooms.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn room(id: u32, uids: &[i64]) -> Room {
        let mut room: Room = serde_json::from_str(&format!(r#"{{"id":{id}}}"#)).unwrap();
        room.uid = uids.iter().copied().map(Uid).collect();
        room
    }

    #[test]
    fn rooms_are_found_by_id_and_kept_in_order() {
        let map = Map::from_rooms(vec![room(9, &[]), room(2, &[])]).unwrap();
        assert_eq!(map.len(), 2);
        assert_eq!(map.rooms()[0].id, RoomId(2));
        assert!(map.room(RoomId(9)).is_some() && map.room(RoomId(3)).is_none());
    }

    /// `plan/21` §3d, both directions of the many-to-many.
    #[test]
    fn a_uid_may_mean_several_rooms_and_a_room_may_have_several_uids() {
        let map = Map::from_rooms(vec![
            room(30778, &[7_120_001]),
            room(30266, &[7_120_001]),
            room(18011, &[13_010_120, 13_010_121]),
        ])
        .unwrap();
        assert_eq!(
            map.ids_for_uid(Uid(7_120_001)),
            [RoomId(30266), RoomId(30778)]
        );
        assert_eq!(map.ids_for_uid(Uid(13_010_121)), [RoomId(18011)]);
        assert!(map.ids_for_uid(Uid(1)).is_empty());
    }

    #[test]
    fn a_repeated_id_is_not_a_map() {
        assert_eq!(
            Map::from_rooms(vec![room(7, &[]), room(7, &[])]),
            Err(DuplicateRoom(RoomId(7)))
        );
    }
}
