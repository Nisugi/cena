//! Which room is this? Each test is one rung of `cena_map::locate`'s ladder,
//! or one way it refuses to guess.

use cena_map::{By, Located, Map, Origin, Room, RoomId, Sighting, Uid, title_from_subtitle};

/// A street, a ship's deck that changes form, a tunnel of identical rooms, and
/// two shops the map has no number for.
const ROOMS: &str = r#"[
  {"id":1,"uid":[100],"title":["[Town, Street]"],"description":["A street."],
   "paths":["Obvious paths: north, east"],
   "exits":[{"to":10,"kind":"cardinal","cmd":"north","cost":0.2},
            {"to":20,"kind":"go","cmd":"go shop","cost":0.2},
            {"to":30,"kind":"go","cmd":"go gangplank","cost":0.2}]},

  {"id":10,"title":["[A Dark Tunnel]"],"description":["Dark."],"paths":["Obvious exits: north, south"],
   "exits":[{"to":11,"kind":"cardinal","cmd":"north","cost":0.2}]},
  {"id":11,"title":["[A Dark Tunnel]"],"description":["Dark."],"paths":["Obvious exits: north, south"],
   "exits":[{"to":12,"kind":"cardinal","cmd":"north","cost":0.2},
            {"to":10,"kind":"cardinal","cmd":"south","cost":0.2}]},
  {"id":12,"title":["[A Dark Tunnel]"],"description":["Dark."],"paths":["Obvious exits: north, south"],
   "exits":[{"to":11,"kind":"cardinal","cmd":"south","cost":0.2}]},

  {"id":20,"title":["[Gert's Goods]"],"description":["Shelves.","Shelves, at night."],
   "paths":["Obvious exits: out"],"location":"Town"},
  {"id":21,"title":["[Gert's Goods]"],"description":["Shelves."],
   "paths":["Obvious exits: out"],"location":"Elsewhere"},
  {"id":22,"uid":[200],"title":["[Gert's Goods]"],"description":["Shelves."],
   "paths":["Obvious exits: out"]},

  {"id":30,"uid":[300],"title":["[Ship, Main Deck]"],"description":["A sloop's deck."]},
  {"id":31,"uid":[300],"title":["[Ship, Main Deck]"],"description":["A brigantine's deck."]},

  {"id":40,"uid":[400],"title":["[Locker]"],"description":["A locker."],"meta":["map:multi-uid"]},
  {"id":50,"title":["[Shifting Cave]"],"tags":["random-paths"],"paths":["Obvious exits: up"]},
  {"id":51,"title":["[Shifting Cave]"],"paths":["Obvious exits: down"]}
]"#;

fn map() -> Option<Map> {
    let rooms: Vec<Room> = serde_json::from_str(ROOMS).ok()?;
    Map::from_rooms(rooms).ok()
}

fn here(room: u32, by: By) -> Located {
    Located::Here {
        room: RoomId(room),
        by,
    }
}

#[test]
fn the_games_number_is_enough_and_stale_text_does_not_argue() {
    let map = map().unwrap();
    let sighting = Sighting {
        uid: Some(Uid(100)),
        title: Some("[Town, Renamed Street]"),
        description: Some("Repainted since the map was made."),
        ..Sighting::default()
    };
    assert_eq!(map.locate(&sighting, Origin::Nowhere), here(1, By::Uid));
}

/// A room that changes form keeps its number; its text says which form.
#[test]
fn a_shared_number_is_settled_by_text() {
    let map = map().unwrap();
    let mut sighting = Sighting {
        uid: Some(Uid(300)),
        title: Some("[Ship, Main Deck]"),
        description: Some("A brigantine's deck."),
        ..Sighting::default()
    };
    assert_eq!(map.locate(&sighting, Origin::Nowhere), here(31, By::Uid));

    // Text that fits neither form is ignored, not obeyed: both stay candidates,
    // and the room just left has an exit to only one of them.
    sighting.description = Some("A deck nobody has recorded.");
    assert_eq!(
        map.locate(&sighting, Origin::Nowhere),
        Located::Ambiguous(vec![RoomId(30), RoomId(31)])
    );
    assert_eq!(
        map.locate(&sighting, Origin::Left(RoomId(1))),
        here(30, By::CameFrom)
    );
}

/// The game numbers every room; the map lacks some. A number the map has never
/// seen cannot belong to a room whose numbers are all recorded.
#[test]
fn an_unseen_number_searches_only_rooms_that_could_own_it() {
    let map = map().unwrap();
    let sighting = Sighting {
        uid: Some(Uid(999)),
        title: Some("[Gert's Goods]"),
        description: Some("Shelves."),
        paths: Some("Obvious exits: out"),
        ..Sighting::default()
    };
    // 22 fits the text and is excluded: it has a number, and it is not this one.
    assert_eq!(
        map.locate(&sighting, Origin::Nowhere),
        Located::Ambiguous(vec![RoomId(20), RoomId(21)])
    );
    // With no number at all, nothing can be excluded.
    let unnumbered = Sighting {
        uid: None,
        ..sighting
    };
    assert_eq!(
        map.locate(&unnumbered, Origin::Nowhere),
        Located::Ambiguous(vec![RoomId(20), RoomId(21), RoomId(22)])
    );
    // A room marked as having more numbers than are recorded stays in the pool.
    let locker = Sighting {
        uid: Some(Uid(450)),
        title: Some("[Locker]"),
        ..Sighting::default()
    };
    assert_eq!(map.locate(&locker, Origin::Nowhere), here(40, By::Text));
}

#[test]
fn any_recorded_variant_of_the_text_matches() {
    let map = map().unwrap();
    let sighting = Sighting {
        uid: Some(Uid(999)),
        title: Some("[Gert's Goods]"),
        description: Some("  Shelves, at night. "),
        ..Sighting::default()
    };
    assert_eq!(map.locate(&sighting, Origin::Nowhere), here(20, By::Text));
}

#[test]
fn a_location_reading_separates_twins() {
    let map = map().unwrap();
    let sighting = Sighting {
        uid: Some(Uid(999)),
        title: Some("[Gert's Goods]"),
        description: Some("Shelves."),
        location: Some("Elsewhere"),
        ..Sighting::default()
    };
    assert_eq!(map.locate(&sighting, Origin::Nowhere), here(21, By::Text));
}

/// Twenty rooms called `[A Dark Tunnel]`: only where the character was helps.
#[test]
fn identical_rooms_are_settled_by_where_the_character_was() {
    let map = map().unwrap();
    let tunnel = Sighting {
        title: Some("[A Dark Tunnel]"),
        description: Some("Dark."),
        paths: Some("Obvious exits: north, south"),
        ..Sighting::default()
    };
    let all = Located::Ambiguous(vec![RoomId(10), RoomId(11), RoomId(12)]);
    assert_eq!(map.locate(&tunnel, Origin::Nowhere), all);

    assert_eq!(
        map.locate(&tunnel, Origin::Left(RoomId(1))),
        here(10, By::CameFrom)
    );
    assert_eq!(
        map.locate(&tunnel, Origin::Left(RoomId(10))),
        here(11, By::CameFrom)
    );
    // The game re-describing the room is not a move.
    assert_eq!(
        map.locate(&tunnel, Origin::Still(RoomId(11))),
        here(11, By::Stayed)
    );
    // 11 leads to both 10 and 12: never a guess.
    assert_eq!(map.locate(&tunnel, Origin::Left(RoomId(11))), all);
    // Standing still somewhere that is not a candidate proves nothing.
    assert_eq!(map.locate(&tunnel, Origin::Still(RoomId(1))), all);
    // A room the map does not have cannot have been left.
    assert_eq!(map.locate(&tunnel, Origin::Left(RoomId(777))), all);
}

#[test]
fn fog_and_shifting_exits_do_not_rule_a_room_out() {
    let map = map().unwrap();
    let mut cave = Sighting {
        title: Some("[Shifting Cave]"),
        paths: Some("Obvious exits: east, west"),
        ..Sighting::default()
    };
    assert_eq!(
        map.locate(&cave, Origin::Nowhere),
        here(50, By::Text),
        "50's exits shift, 51's do not"
    );
    cave.paths = Some("Obvious exits: obscured by a thick fog");
    assert_eq!(
        map.locate(&cave, Origin::Nowhere),
        Located::Ambiguous(vec![RoomId(50), RoomId(51)])
    );
}

#[test]
fn too_little_seen_is_unknown_not_a_guess() {
    let map = map().unwrap();
    assert_eq!(
        map.locate(&Sighting::default(), Origin::Still(RoomId(1))),
        Located::Unknown
    );
    let nowhere = Sighting {
        uid: Some(Uid(999)),
        title: Some("[A Room Nobody Mapped]"),
        ..Sighting::default()
    };
    assert_eq!(map.locate(&nowhere, Origin::Nowhere), Located::Unknown);
    // A description alone cannot search the map: titles are the index.
    let untitled = Sighting {
        description: Some("A street."),
        ..Sighting::default()
    };
    assert_eq!(map.locate(&untitled, Origin::Nowhere), Located::Unknown);
}

#[test]
fn the_wire_title_becomes_the_maps_spelling() {
    assert_eq!(
        title_from_subtitle(" - Rawknuckle's, Watering Hole"),
        "[Rawknuckle's, Watering Hole]"
    );
    assert_eq!(
        title_from_subtitle(" - Duskruin Arena, Dueling Sands - 8213304"),
        "[Duskruin Arena, Dueling Sands]"
    );
    // A dash inside a name is not a room number.
    assert_eq!(
        title_from_subtitle(" - Ta'Vaalor - North Gate"),
        "[Ta'Vaalor - North Gate]"
    );
}
