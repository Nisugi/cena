//! The loaded map: every room, and the two indexes `plan/21` §3d decides on.
//!
//! One `Map` is built once and shared by every session in the process; nothing
//! here is mutable after construction.

use std::collections::HashMap;
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
        })
    }

    /// The room with this id.
    #[must_use]
    pub fn room(&self, id: RoomId) -> Option<&Room> {
        self.index_of
            .get(&id)
            .and_then(|index| self.rooms.get(*index))
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
