//! The room record.
//!
//! `plan/21` §3f. Step 2 of that plan carries only what the upstream map
//! already knows; layout (`map`, `pos`, `floor`), `color` and `aliases` arrive
//! when their source is joined in (§3e), as new optional fields.

use serde::{Deserialize, Serialize};

use crate::exit::Exit;

/// The map's own room number, assigned upstream in the order rooms were
/// mapped. **This names the graph node** (`plan/21` §3d): every exit, route and
/// crossing parameter is in these terms. It is total and unique; [`Uid`] is
/// neither.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct RoomId(pub u32);

/// The game's internal room number, as the wire reports it.
///
/// **This identifies where a character is**; it does not name a node. The
/// relation to [`RoomId`] is many-to-many: an instanced room has one id and up
/// to fifty uids, a morphing room has one uid and several ids, and a fifth of
/// the map has none (`plan/21` §3d, measured). Signed and wide, because the
/// game sends negative numbers for generated areas.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct Uid(pub i64);

/// Where a room sits on one of the pre-drawn map pictures. The fallback view
/// for rooms with no layout position (`plan/21` §3e).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Image {
    /// The picture's file name.
    pub file: String,
    /// The room's rectangle on it: left, top, right, bottom, in pixels.
    pub rect: [i32; 4],
}

/// One mapped room.
///
/// `title`, `description` and `paths` are **lists** because rooms have
/// variants -- day and night, seasonal, before and after an event -- and the
/// text matcher must accept any of them. Keeping only the first is the bug
/// Genie 5 records fixing (`plan/21` §3e).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Room {
    /// The graph node.
    pub id: RoomId,
    /// Every game room number known for this node. Empty when the room has
    /// never been seen since the game began sending them, or is virtual.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub uid: Vec<Uid>,

    /// Titles, as the game prints them, brackets included.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub title: Vec<String>,
    /// Descriptions.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub description: Vec<String>,
    /// The "Obvious exits: …" lines.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub paths: Vec<String>,

    /// What the game's `location` verb answers here.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub location: Option<String>,
    /// The `location` verb **fails** here, and that failure is itself the
    /// signal: a twin of this room is told apart by the verb failing again
    /// (`reference/lich-5/lib/common/map/map_gs.rb:135-157`). Upstream spells
    /// this `location: false`.
    #[serde(default, skip_serializing_if = "is_false")]
    pub location_unknowable: bool,
    /// An identical room exists elsewhere, so identifying this one means
    /// sending the `location` verb.
    #[serde(default, skip_serializing_if = "is_false")]
    pub check_location: bool,
    /// Objects that must all be present for the text matcher to accept this
    /// room.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub unique_loot: Vec<String>,

    /// Climate, as upstream names it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub climate: Option<String>,
    /// Terrain, as upstream names it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub terrain: Option<String>,

    /// Plain tags: travel targets (`bank`, `gemshop`, `town`) and, for now,
    /// forage names. Splitting forage out is `plan/21` §6.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tags: Vec<String>,
    /// Upstream's `meta:` tags with the prefix removed, forage sightings
    /// excluded. Kept as strings until each kind is typed by the step that
    /// first reads it.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub meta: Vec<String>,

    /// The picture fallback.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub image: Option<Image>,

    /// Exits, ordered by destination so the file is stable.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub exits: Vec<Exit>,

    /// The grid this room is drawn on, when that is not its area's own sheet:
    /// a plate, or an `<area>.interiors` shelf.
    ///
    /// A slug. What each slug *is* — its display name and the area it is a
    /// sheet of — lives in a map-level registry, which is not carried here
    /// yet: a slug with no entry is a validation error for whoever builds the
    /// file, not a load error for the client.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub map: Option<String>,

    /// The area this room belongs to.
    ///
    /// **A plate is a grid, not a place.** A room drawn on `landing.well` is
    /// still a room *of* Wehnimer's Landing, so [`Self::map`] cannot answer
    /// this and the two are separate fields rather than one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub area: Option<String>,

    /// Where a dragged room sits, relative to a room that did not move.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub placement: Option<Placement>,
}

/// A corrected position, stated as an offset from a room that did not move.
///
/// # Why an offset and not a coordinate
///
/// Room ids are assigned by the build, so a rebuild renumbers them and an
/// absolute position recorded against one build is meaningless in the next.
/// An offset from an **anchor** survives that, because the anchor is named by
/// [`Uid`] — the game's own id, which the build does not invent.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Placement {
    /// The room the offset is measured from. Never itself placed.
    pub anchor: Uid,
    /// Cells east; negative is west.
    pub dx: i32,
    /// Cells south; negative is north.
    ///
    /// **The y axis grows DOWNWARD**, which is the opposite of the intuition a
    /// compass gives. A reader that gets this backwards mirrors every
    /// correction it applies, and mirrored output looks plausible — so the
    /// direction is stated here rather than left to be inferred from a
    /// renderer.
    pub dy: i32,
}

// serde's `skip_serializing_if` passes a reference.
#[allow(clippy::trivially_copy_pass_by_ref)]
fn is_false(b: &bool) -> bool {
    !*b
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_empty_room_is_just_its_id() {
        let room: Room = serde_json::from_str(r#"{"id":7}"#).unwrap();
        assert_eq!(room.id, RoomId(7));
        assert!(room.uid.is_empty() && room.exits.is_empty());
        assert_eq!(serde_json::to_string(&room).unwrap(), r#"{"id":7}"#);
    }

    /// `plan/21` §3d: the game sends negative numbers for generated areas.
    #[test]
    fn a_uid_may_be_negative() {
        let room: Room = serde_json::from_str(r#"{"id":4136,"uid":[-9054,-9053]}"#).unwrap();
        assert_eq!(room.uid, vec![Uid(-9054), Uid(-9053)]);
    }
}
