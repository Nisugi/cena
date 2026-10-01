//! Where a shop is, for this round (`plan/61` step 3): the nearest room
//! tagged for it, by what this walker would pay, with eloot's two rules about
//! places laid over that.
//!
//! - **Selling in Mist Harbor** (`sell_fwi`, `ELoot.go2`, `eloot.lic:3300-3309`):
//!   a shop the island has is the island's, wherever the round begins, and
//!   the walk there and back is travel's, by the trinket
//!   (`travel/routines/trinket.rs`). The island has no Chronomage
//!   (`go_sell`, `:7106`): gold rings are given before leaving a town, and
//!   not at all from the island. A shop the island lacks is the nearest
//!   anywhere, as eloot leaves it.
//! - **The Hinterwilds** (`shop_unavailable_in_town?`, `:3236-3253`): its
//!   town has a gem shop, a furrier and the pool, and no pawnshop,
//!   collectibles counter, consignment or Chronomage. From there the nearest
//!   of those is another town's, across the map, so the shop is skipped
//!   unless the round sells in Mist Harbor.
//!
//! When the island cannot be reached at all (no trinket named in travel's
//! settings), the round sells where it stands, the Hinterwilds' rule
//! included: [`reaches_fwi`] says so before it begins.

use cena_map::{Map, Room, RoomId, Target, Uid, Walker};

use crate::travel::Trip;

/// The Hinterwilds' town, Coldriver Village, by the game's room number
/// (`HINTERWILDS_TOWN_UID`, `eloot.lic:3243`).
pub const HINTERWILDS_TOWN: Uid = Uid(7_503_205);

/// The shops the Hinterwilds' town lacks, by tag.
const NOT_IN_HINTERWILDS: &[&str] = &[
    "pawnshop",
    "collectibles",
    "collectible",
    "consignment",
    "chronomage",
];

/// Whether `room` is on the Isle of Four Winds (`ELoot.fwi?`, `:3232`).
#[must_use]
pub fn is_fwi(room: &Room) -> bool {
    room.location.as_deref().is_some_and(|location| {
        ["Four Winds", "Mist Harbor", "Western Harbor"]
            .iter()
            .any(|name| location.contains(name))
    })
}

/// The rooms tagged `tag` that `keep` keeps.
fn tagged(map: &Map, tag: &str, keep: impl Fn(&Room) -> bool) -> Vec<RoomId> {
    map.rooms()
        .iter()
        .filter(|room| room.tags.iter().any(|has| has.eq_ignore_ascii_case(tag)) && keep(room))
        .map(|room| room.id)
        .collect()
}

/// The nearest of `rooms` to `from`, by what `walker` would pay.
fn nearest(map: &Map, walker: &Walker, from: RoomId, rooms: &[RoomId]) -> Option<RoomId> {
    map.routes(from, Target::Nearest(rooms), Trip::to(from).pricing(walker))
        .reached()
}

/// Whether the town nearest `from` is the Hinterwilds'.
fn in_hinterwilds(map: &Map, walker: &Walker, from: RoomId) -> bool {
    nearest(map, walker, from, &tagged(map, "town", |_| true))
        .and_then(|town| map.room(town))
        .is_some_and(|town| town.uid.contains(&HINTERWILDS_TOWN))
}

/// Whether a round that sells in Mist Harbor can get there from `from`: some
/// room of the island is reachable. `true` when it is already there.
#[must_use]
pub fn reaches_fwi(map: &Map, walker: &Walker, from: RoomId) -> bool {
    let island: Vec<RoomId> = map
        .rooms()
        .iter()
        .filter(|room| is_fwi(room))
        .map(|room| room.id)
        .collect();
    nearest(map, walker, from, &island).is_some()
}

/// The room to sell at for a shop's `tag`, from `from`; `None` when the
/// round passes the shop by. `fwi` is the profile's `sell_fwi`, already
/// found reachable.
#[must_use]
pub fn shop_room(map: &Map, walker: &Walker, from: RoomId, tag: &str, fwi: bool) -> Option<RoomId> {
    let on_island = map.room(from).is_some_and(is_fwi);
    let anywhere = || nearest(map, walker, from, &tagged(map, tag, |_| true));
    if tag.eq_ignore_ascii_case("chronomage") {
        // None on the island; from a town on the way there, the town's own.
        let passed_by = on_island || (!fwi && in_hinterwilds(map, walker, from));
        return if passed_by { None } else { anywhere() };
    }
    if fwi {
        let island = tagged(map, tag, is_fwi);
        if !island.is_empty() {
            return nearest(map, walker, from, &island);
        }
        return anywhere();
    }
    if NOT_IN_HINTERWILDS
        .iter()
        .any(|lacking| tag.eq_ignore_ascii_case(lacking))
        && in_hinterwilds(map, walker, from)
    {
        return None;
    }
    anywhere()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A town (1, with its gem shop 2 and pawnshop 3), the island a step
    /// from it (10, with its gem shop 11 and bank 12), and the Hinterwilds
    /// (20, a town with a gem shop 21), a long walk from the first town.
    const ROOMS: &str = r#"[
      {"id":1,"uid":[1001],"tags":["town"],"location":"Wehnimer's Landing","exits":[
        {"to":2,"kind":"cardinal","cmd":"east","cost":1},
        {"to":3,"kind":"cardinal","cmd":"west","cost":1},
        {"to":4,"kind":"cardinal","cmd":"up","cost":1},
        {"to":10,"kind":"cardinal","cmd":"south","cost":5},
        {"to":20,"kind":"cardinal","cmd":"north","cost":500}]},
      {"id":2,"uid":[1002],"tags":["gemshop"],"location":"Wehnimer's Landing","exits":[
        {"to":1,"kind":"cardinal","cmd":"west","cost":1}]},
      {"id":3,"uid":[1003],"tags":["pawnshop"],"location":"Wehnimer's Landing","exits":[
        {"to":1,"kind":"cardinal","cmd":"east","cost":1}]},
      {"id":4,"uid":[1004],"tags":["chronomage"],"location":"Wehnimer's Landing","exits":[
        {"to":1,"kind":"cardinal","cmd":"down","cost":1}]},
      {"id":10,"uid":[1010],"tags":["town"],"location":"the Isle of Four Winds","exits":[
        {"to":1,"kind":"cardinal","cmd":"north","cost":5},
        {"to":11,"kind":"cardinal","cmd":"east","cost":1},
        {"to":12,"kind":"cardinal","cmd":"west","cost":1}]},
      {"id":11,"uid":[1011],"tags":["gemshop"],"location":"Mist Harbor","exits":[
        {"to":10,"kind":"cardinal","cmd":"west","cost":1}]},
      {"id":12,"uid":[1012],"tags":["bank"],"location":"Mist Harbor","exits":[
        {"to":10,"kind":"cardinal","cmd":"east","cost":1}]},
      {"id":20,"uid":[7503205],"tags":["town"],"location":"the Hinterwilds","exits":[
        {"to":1,"kind":"cardinal","cmd":"south","cost":500},
        {"to":21,"kind":"cardinal","cmd":"east","cost":1}]},
      {"id":21,"uid":[1021],"tags":["gemshop"],"location":"the Hinterwilds","exits":[
        {"to":20,"kind":"cardinal","cmd":"west","cost":1}]}
    ]"#;

    fn map() -> Map {
        let rooms: Vec<Room> = serde_json::from_str(ROOMS).unwrap();
        Map::from_rooms(rooms).unwrap()
    }

    fn room(tag: &str, from: u32, fwi: bool) -> Option<u32> {
        shop_room(&map(), &Walker::default(), RoomId(from), tag, fwi).map(|room| room.0)
    }

    /// With `sell_fwi` the island's shop is the shop, from a town with its
    /// own; without it, the nearest.
    #[test]
    fn selling_in_mist_harbor_takes_the_islands_shop_over_the_towns() {
        assert_eq!(room("gemshop", 1, false), Some(2));
        assert_eq!(room("gemshop", 1, true), Some(11));
        assert_eq!(room("bank", 1, true), Some(12));
        // The island has no pawnshop: the nearest anywhere, as eloot has it.
        assert_eq!(room("pawnshop", 1, true), Some(3));
        assert!(reaches_fwi(&map(), &Walker::default(), RoomId(1)));
    }

    /// Gold rings are given before leaving a town, never from the island.
    #[test]
    fn the_chronomage_is_the_towns_and_not_the_islands() {
        assert_eq!(room("chronomage", 1, true), Some(4));
        assert_eq!(room("chronomage", 1, false), Some(4));
        assert_eq!(room("chronomage", 10, true), None);
        assert_eq!(room("chronomage", 10, false), None);
    }

    /// The author's own case: the Hinterwilds has no pawnshop, so its
    /// hunter sells on the island, or not at a pawnshop at all, and never
    /// walks to another town's.
    #[test]
    fn the_hinterwilds_lacks_four_shops_and_never_walks_to_anothers() {
        assert_eq!(room("gemshop", 20, false), Some(21), "its own gem shop");
        assert_eq!(room("pawnshop", 20, false), None);
        assert_eq!(room("collectibles", 20, false), None);
        assert_eq!(room("chronomage", 20, false), None);
        // Selling in Mist Harbor, the island's gem shop and bank; the
        // pawnshop the island lacks is the nearest anywhere.
        assert_eq!(room("gemshop", 20, true), Some(11));
        assert_eq!(room("pawnshop", 20, true), Some(3));
    }
}
