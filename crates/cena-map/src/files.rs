//! Where the per-room JSON lives on disk.
//!
//! The converter writes this tree and the combiner reads it; defining the paths
//! once, here, is what stops the two disagreeing. Pure string building -- this
//! crate still touches no file.
//!
//! ```text
//! <dir>/index.json            every room id in this conversion
//! <dir>/rooms/036/36838.json  one room; sharded by thousand
//! ```
//!
//! `index.json`, not a directory listing, says which room files are current. A
//! room removed upstream leaves a stale file behind, and nothing downstream may
//! pick it up by walking the tree.

use crate::room::RoomId;

/// The index file's name, relative to the conversion directory.
pub const INDEX: &str = "index.json";

/// A room file's path relative to the conversion directory, with `/`
/// separators. Sharded by thousand because tens of thousands of entries in one
/// directory is unkind to every tool that lists it.
#[must_use]
pub fn room_file(id: RoomId) -> String {
    format!("rooms/{:03}/{}.json", id.0 / 1000, id.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rooms_shard_by_thousand() {
        assert_eq!(room_file(RoomId(0)), "rooms/000/0.json");
        assert_eq!(room_file(RoomId(999)), "rooms/000/999.json");
        assert_eq!(room_file(RoomId(36838)), "rooms/036/36838.json");
    }
}
