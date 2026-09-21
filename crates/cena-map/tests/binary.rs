//! The map file format, held to the three rules it exists to keep
//! (`cena_map::binary`, `plan/21` §3a).
//!
//! "A newer map" is simulated by patching an encoded file's string table: a
//! wire name is replaced by another of the same length, which is exactly what
//! an older client sees when a later build has added a kind it does not know.

use cena_map::binary::{LoadError, MAGIC, VERSION, decode, encode};
use cena_map::{Cost, Crossing, Map, Room, RoomId, Uid};

const ROOMS: &str = r#"[
  {"id":7,"uid":[13100007],"title":["[Ta'Illistim, BriarStone Court]"],
   "description":["A court.","A court, at night."],"paths":["Obvious paths: north"],
   "location":"Ta'Illistim","climate":"temperate","terrain":"cobblestone",
   "tags":["town","urchin-access"],"meta":["map:multi-uid"],
   "image":{"file":"EN-ti.png","rect":[10,20,30,40]},
   "exits":[
     {"to":8,"kind":"cardinal","cmd":"north","cost":0.2},
     {"to":30714,"kind":"scripted","unported":"2bdadd5c29e6d32b","cost":{"unported":"89948cb765ebd1a0"}}
   ]},
  {"id":8,"uid":[13100008,-9054],"location_unknowable":true,"check_location":true,
   "unique_loot":["a bronze statue"],
   "exits":[{"to":7,"kind":"cardinal","cmd":"south"}]},
  {"id":30714,"title":["[Ta'Illistim - Urchin Hideout]"],
   "exits":[{"to":7,"kind":"other","cmd":"urchin guide briarstone","cost":0.1}]}
]"#;

fn map() -> Option<Map> {
    let rooms: Vec<Room> = serde_json::from_str(ROOMS).ok()?;
    Map::from_rooms(rooms).ok()
}

/// Replace one string-table entry with another of the same length.
fn rename(file: &[u8], from: &str, to: &str) -> Option<Vec<u8>> {
    if from.len() != to.len() {
        return None;
    }
    let mut needle = u32::try_from(from.len()).ok()?.to_le_bytes().to_vec();
    needle.extend_from_slice(from.as_bytes());
    let at = file
        .windows(needle.len())
        .position(|window| window == needle)?;
    let mut patched = file.to_vec();
    patched[at + 4..at + needle.len()].copy_from_slice(to.as_bytes());
    Some(patched)
}

#[test]
fn a_map_round_trips_exactly() {
    let map = map().unwrap();
    let file = encode(&map).unwrap();
    assert_eq!(&file[..8], MAGIC);
    assert_eq!(decode(&file).unwrap(), map);
    // Every field that has a quiet default survived as itself.
    let decoded = decode(&file).unwrap();
    let court = decoded.room(RoomId(7)).unwrap();
    assert_eq!(
        court.description.len(),
        2,
        "every description variant is kept"
    );
    let next = decoded.room(RoomId(8)).unwrap();
    assert!(next.location_unknowable && next.check_location);
    assert_eq!(next.uid, [Uid(13_100_008), Uid(-9054)]);
    assert_eq!(
        next.exits[0].cost, None,
        "no cost stays no cost: impassable, not defaulted"
    );
}

#[test]
fn encoding_is_deterministic() {
    let map = map().unwrap();
    assert_eq!(encode(&map).unwrap(), encode(&map).unwrap());
}

/// Strings are interned: a command used a thousand times is stored once.
#[test]
fn repeated_strings_are_stored_once() {
    let file = encode(&map().unwrap()).unwrap();
    let count = file
        .windows(b"Ta'Illistim".len())
        .filter(|w| *w == b"Ta'Illistim")
        .count();
    // Once as the location; the two titles contain it inside longer strings.
    assert_eq!(count, 3);
    let north = file.windows(5).filter(|w| *w == b"north").count();
    assert_eq!(
        north, 2,
        "`north` the command once, and once inside the paths line"
    );
}

/// RULE 1. A map built after a primitive was added meets a client built before.
#[test]
fn a_crossing_this_build_does_not_know_loads_as_impassable() {
    let file = rename(&encode(&map().unwrap()).unwrap(), "cmd", "zap").unwrap();
    let map = decode(&file).expect("an unknown crossing is not a load error");
    for room in map.rooms() {
        for exit in &room.exits {
            assert!(!matches!(exit.crossing, Crossing::Command(_)));
            assert!(!exit.is_routable());
        }
    }
    let court = map.room(RoomId(7)).unwrap();
    assert_eq!(court.exits[0].crossing, Crossing::Unknown("zap".into()));
    // The rest of the room, and the rest of the map, is untouched.
    assert_eq!(court.exits[0].cost, Some(Cost::Fixed(0.2)));
    assert_eq!(map.len(), 3);
}

/// RULE 1, inside a crossing. The step list is JSON in the file, so a step a
/// later build added fails to parse here -- and that must cost one exit, not
/// the map.
#[test]
fn ported_steps_round_trip_and_a_step_this_build_does_not_know_is_impassable() {
    let rooms: Vec<Room> = serde_json::from_str(
        r#"[{"id":1,"exits":[{"to":1,"kind":"scripted","cost":0.2,
             "steps":[{"pause":4200,"when":{"spell_active":"Haste"}},{"move":"west"}]}]}]"#,
    )
    .unwrap();
    let map = Map::from_rooms(rooms).unwrap();
    let file = encode(&map).unwrap();
    let loaded = decode(&file).unwrap();
    assert_eq!(loaded, map);
    assert!(loaded.room(RoomId(1)).unwrap().exits[0].is_routable());

    // `pause` -> `yodel`, the same length: a step from the future.
    let at = file.windows(7).position(|w| w == b"\"pause\"").unwrap();
    let mut newer = file.clone();
    newer[at + 1..at + 6].copy_from_slice(b"yodel");
    let loaded = decode(&newer).expect("one unreadable crossing is not a load error");
    let exit = &loaded.room(RoomId(1)).unwrap().exits[0];
    assert_eq!(exit.crossing, Crossing::Unknown("steps".into()));
    assert_eq!(
        exit.cost,
        Some(Cost::Fixed(0.2)),
        "the rest of the exit loaded"
    );
    assert!(!exit.is_routable());
}

/// A routine is a name and its arguments; one this build has never heard of is
/// an exit it cannot cross, and nothing worse.
#[test]
fn a_routine_round_trips_and_an_unheard_of_one_is_impassable() {
    let rooms: Vec<Room> = serde_json::from_str(
        r#"[{"id":1,"exits":[
             {"to":1,"kind":"scripted","cost":0.2,
              "routine":{"name":"minotaur_maze","rooms":[6191,6254,6192]}},
             {"to":1,"kind":"scripted","cost":0.2,"routine":{"name":"confluence","leave":true}}]}]"#,
    )
    .unwrap();
    let map = Map::from_rooms(rooms).unwrap();
    let file = encode(&map).unwrap();
    assert_eq!(decode(&file).unwrap(), map);

    let at = file.windows(10).position(|w| w == b"confluence").unwrap();
    let mut newer = file.clone();
    newer[at..at + 10].copy_from_slice(b"wormhole__");
    let loaded = decode(&newer).unwrap();
    let exits = &loaded.room(RoomId(1)).unwrap().exits;
    assert!(exits[0].is_routable(), "the maze beside it is untouched");
    assert_eq!(exits[1].crossing, Crossing::Unknown("routine".into()));
    assert!(!exits[1].is_routable());
}

/// A gated cost and a pass-through crossing survive the file, and a condition
/// from the future costs one exit its price, not the map.
#[test]
fn gated_costs_and_pass_through_round_trip() {
    let rooms: Vec<Room> = serde_json::from_str(
        r#"[{"id":1,"exits":[{"to":1,"kind":"scripted","pass":null,
             "cost":{"when":{"profession":"Bard"},"then":0.2}}]}]"#,
    )
    .unwrap();
    let map = Map::from_rooms(rooms).unwrap();
    let file = encode(&map).unwrap();
    assert_eq!(decode(&file).unwrap(), map);

    let at = file
        .windows(12)
        .position(|w| w == b"\"profession\"")
        .unwrap();
    let mut newer = file.clone();
    newer[at + 1..at + 11].copy_from_slice(b"profezzion");
    let loaded = decode(&newer).unwrap();
    let exit = &loaded.room(RoomId(1)).unwrap().exits[0];
    assert_eq!(exit.cost, Some(Cost::Unknown("gated".into())));
    assert_eq!(exit.crossing, Crossing::PassThrough(cena_map::Pass));
}

/// RULE 1, for costs.
#[test]
fn a_cost_this_build_does_not_know_loads_as_impassable() {
    let file = rename(&encode(&map().unwrap()).unwrap(), "fixed", "tidal").unwrap();
    let map = decode(&file).unwrap();
    let exit = &map.room(RoomId(7)).unwrap().exits[0];
    assert_eq!(exit.cost, Some(Cost::Unknown("tidal".into())));
    assert_eq!(
        exit.crossing,
        Crossing::Command("north".into()),
        "the command still loaded"
    );
    assert!(!exit.is_routable());
}

/// RULE 1, for rooms: layout and floors will arrive as extensions, and a
/// client that predates them must skip them by length.
#[test]
fn a_room_extension_this_build_does_not_know_is_skipped() {
    let one: Vec<Room> = serde_json::from_str(r#"[{"id":1,"title":["[A Room]"]}]"#).unwrap();
    let map = Map::from_rooms(one).unwrap();
    let mut file = encode(&map).unwrap();
    // The file ends with this room's extension count, which is zero.
    let end = file.len();
    assert_eq!(&file[end - 4..], &[0, 0, 0, 0]);
    file[end - 4] = 1;
    file.extend_from_slice(&0u32.to_le_bytes()); // name: string 0, whatever it is
    file.extend_from_slice(&3u32.to_le_bytes()); // three bytes this build cannot read
    file.extend_from_slice(&[0xde, 0xad, 0xbe]);
    assert_eq!(decode(&file).unwrap(), map);
}

/// An unknown *kind* only changes how an exit is drawn.
#[test]
fn an_exit_kind_this_build_does_not_know_is_drawn_plainly() {
    let file = rename(&encode(&map().unwrap()).unwrap(), "cardinal", "teleport").unwrap();
    let map = decode(&file).unwrap();
    let exit = &map.room(RoomId(7)).unwrap().exits[0];
    assert_eq!(exit.kind, cena_map::ExitKind::Other);
    assert!(exit.is_routable(), "kind never affects routing");
}

/// RULE 2.
#[test]
fn another_version_is_refused_not_misread() {
    let mut file = encode(&map().unwrap()).unwrap();
    file[8..12].copy_from_slice(&(VERSION + 1).to_le_bytes());
    assert_eq!(
        decode(&file),
        Err(LoadError::UnsupportedVersion {
            found: VERSION + 1,
            supported: VERSION
        })
    );
    assert_eq!(
        decode(b"not a map file, not even close"),
        Err(LoadError::BadMagic)
    );
    assert_eq!(decode(b""), Err(LoadError::BadMagic));
}

/// A tool must not re-write what it could not read.
#[test]
fn an_unknown_crossing_is_never_written_back() {
    let file = rename(&encode(&map().unwrap()).unwrap(), "cmd", "zap").unwrap();
    assert!(encode(&decode(&file).unwrap()).is_err());
}

/// The loader never panics and never trusts a count.
#[test]
fn every_truncation_is_an_error_and_none_is_a_panic() {
    let file = encode(&map().unwrap()).unwrap();
    for len in 0..file.len() {
        assert!(
            decode(&file[..len]).is_err(),
            "a {len}-byte prefix must not load"
        );
    }
    let mut trailing = file.clone();
    trailing.push(0);
    assert!(matches!(
        decode(&trailing),
        Err(LoadError::TrailingBytes { .. })
    ));
}

#[test]
fn a_count_larger_than_the_file_allocates_nothing() {
    let mut file = MAGIC.to_vec();
    file.extend_from_slice(&VERSION.to_le_bytes());
    file.extend_from_slice(&u32::MAX.to_le_bytes()); // four billion strings, in no bytes
    assert!(matches!(decode(&file), Err(LoadError::Truncated { .. })));
}

#[test]
fn a_string_reference_outside_the_table_is_an_error() {
    let one: Vec<Room> = serde_json::from_str(r#"[{"id":1,"title":["[A Room]"]}]"#).unwrap();
    let mut file = encode(&Map::from_rooms(one).unwrap()).unwrap();
    // The title list is: count 1, then a reference. Point it past the table.
    let needle = [1u32.to_le_bytes(), 0u32.to_le_bytes()].concat();
    let at = file
        .windows(8)
        .rposition(|window| window == needle)
        .unwrap();
    file[at + 4..at + 8].copy_from_slice(&999u32.to_le_bytes());
    assert!(matches!(
        decode(&file),
        Err(LoadError::BadStringRef { reference: 999, .. })
    ));
}
