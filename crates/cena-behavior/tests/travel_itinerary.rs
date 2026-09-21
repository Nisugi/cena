//! The route, shown and not walked: `cena_behavior::travel::itinerary`.

use std::collections::BTreeMap;

use cena_behavior::travel::{ShutWhy, destination, itinerary, table};
use cena_map::{Map, Room, RoomId, Walker};

/// `travel_trip.rs`'s map, less what this does not need:
/// ```text
///   1 --east-- 2 --east-- 3 --east-- 4      plain, 1s each
///   1 --go shortcut, bards only, 0.5s-- 4
///   2 --a routine-- 4                       named, and not yet written
///   1 --go nowhere-- 4                      no cost at all
/// ```
const ROOMS: &str = r#"[
  {"id":1,"title":["[Town, Gate]"],"location":"the town",
   "exits":[{"to":2,"kind":"cardinal","cmd":"east","cost":1},
            {"to":4,"kind":"go","cmd":"go shortcut","cost":{"when":{"profession":"Bard"},"then":0.5}},
            {"to":4,"kind":"go","cmd":"go nowhere"}]},
  {"id":2,"exits":[{"to":3,"kind":"cardinal","cmd":"east","cost":1},
                   {"to":4,"kind":"scripted","routine":{"name":"mirror"},"cost":0.1}]},
  {"id":3,"exits":[{"to":4,"kind":"cardinal","cmd":"east","cost":1}]},
  {"id":4,"title":["[Town, Square]"]}
]"#;

/// `None` is a broken fixture, which every test unwraps into a failure.
fn map_of(rooms: &str) -> Option<Map> {
    let rooms: Vec<Room> = serde_json::from_str(rooms).ok()?;
    Map::from_rooms(rooms).ok()
}

fn a(profession: &str) -> Walker {
    Walker {
        profession: Some(profession.into()),
        ..Walker::default()
    }
}

#[test]
fn the_route_is_priced_for_the_one_walking_it() {
    let map = map_of(ROOMS).unwrap();
    let bard = itinerary(&map, &a("Bard"), RoomId(1), RoomId(4)).unwrap();
    assert_eq!(bard.len(), 1);
    assert_eq!(bard[0].how, "go shortcut");
    assert!((bard[0].so_far - 0.5).abs() < f64::EPSILON);
    assert_eq!(bard[0].title.as_deref(), Some("[Town, Square]"));

    let warrior = itinerary(&map, &a("Warrior"), RoomId(1), RoomId(4)).unwrap();
    let moves: Vec<&str> = warrior.iter().map(|leg| leg.how.as_str()).collect();
    assert_eq!(moves, ["east", "east", "east"]);
    // The total is the real one, scripted costs and all.
    assert!((warrior[2].so_far - 3.0).abs() < f64::EPSILON);
}

/// The long way round says why: each of the three reasons, told apart.
#[test]
fn what_it_did_not_take_is_named_with_the_reason() {
    let map = map_of(ROOMS).unwrap();
    let why = |walker: &Walker, leg: usize, how: &str| {
        let legs = itinerary(&map, walker, RoomId(1), RoomId(4)).unwrap();
        let shut = legs[leg].shut.iter().find(|shut| shut.how.contains(how));
        shut.map(|shut| shut.why.clone())
    };
    assert_eq!(why(&a("Warrior"), 0, "shortcut"), Some(ShutWhy::SaysNo));
    // Not told the profession: the question itself, so it can be answered.
    let unknown = why(&Walker::default(), 0, "shortcut");
    assert!(
        matches!(&unknown, Some(ShutWhy::NotKnown(question)) if question.contains("Bard")),
        "{unknown:?}"
    );
    assert_eq!(why(&a("Warrior"), 0, "nowhere"), Some(ShutWhy::NoCost));
    // Payable by anyone, and a routine nobody has written.
    assert_eq!(why(&a("Warrior"), 1, "Mirror"), Some(ShutWhy::NotBuiltYet));
    // What the bard could take is not listed as shut to the bard.
    assert_eq!(why(&a("Bard"), 0, "shortcut"), None);
}

#[test]
fn no_way_is_no_route() {
    assert_eq!(
        itinerary(&map_of(ROOMS).unwrap(), &a("Bard"), RoomId(4), RoomId(1)),
        None
    );
}

#[test]
fn the_table_is_lines_a_terminal_can_print() {
    let map = map_of(ROOMS).unwrap();
    let legs = itinerary(&map, &a("Warrior"), RoomId(1), RoomId(4)).unwrap();
    let lines = table(&map, RoomId(1), &legs);
    assert!(lines[0].starts_with("STEP"));
    assert!(lines[1].contains("[Town, Gate] (the town)"), "{}", lines[1]);
    let last = lines.last().unwrap();
    assert!(
        last.contains("3.0") && last.contains("[Town, Square]"),
        "{last}"
    );
    assert!(
        lines
            .iter()
            .any(|line| line.contains("shut: go shortcut -> 4: not for this character")),
        "{lines:#?}"
    );
    // Columns line up: the names start under their heading, in every row
    // that has one. `unwrap`, so a row without the text fails and does not
    // compare one missing answer with another.
    let under = lines[0].find("NAME").unwrap();
    assert_eq!(lines[1].find("[Town, Gate]").unwrap(), under);
    assert_eq!(last.find("[Town, Square]").unwrap(), under);
}

/// Two banks: one a step away behind a bards' door, one three steps away by
/// the road. "The nearest bank" is a different room for different walkers.
const BANKS: &str = r#"[
  {"id":1,"uid":[7120],"exits":[
     {"to":2,"kind":"go","cmd":"go door","cost":{"when":{"profession":"Bard"},"then":1}},
     {"to":3,"kind":"cardinal","cmd":"east","cost":1}]},
  {"id":2,"tags":["bank"]},
  {"id":3,"exits":[{"to":4,"kind":"cardinal","cmd":"east","cost":1}]},
  {"id":4,"exits":[{"to":5,"kind":"cardinal","cmd":"east","cost":1}]},
  {"id":5,"uid":[9001,9002],"tags":["Bank","town"]}
]"#;

#[test]
fn what_the_player_typed_is_a_room() {
    let map = map_of(BANKS).unwrap();
    let none = BTreeMap::new();
    let to = |walker: &Walker, what: &str| destination(&map, walker, RoomId(1), what, &none);

    assert_eq!(to(&a("Bard"), "5"), Some(RoomId(5)));
    assert_eq!(to(&a("Bard"), "77"), None, "no such room");
    assert_eq!(
        to(&a("Bard"), "u9001"),
        Some(RoomId(5)),
        "the game's number"
    );
    assert_eq!(
        to(&a("Bard"), "u1"),
        None,
        "1 is the map's number, not the game's"
    );

    // The nearest, for whoever is asking -- and a tag is not case-sensitive.
    assert_eq!(to(&a("Bard"), "bank"), Some(RoomId(2)));
    assert_eq!(to(&a("Warrior"), "bank"), Some(RoomId(5)));
    assert_eq!(to(&a("Warrior"), "TOWN"), Some(RoomId(5)));
    assert_eq!(to(&a("Warrior"), "forge"), None, "nothing is tagged that");
}

/// go2's custom targets, in go2's order: before tags, the exact name before a
/// name it begins, neither minding case, and several rooms meaning the nearest.
#[test]
fn a_name_the_player_chose_comes_before_a_tag() {
    let map = map_of(BANKS).unwrap();
    let mut targets = BTreeMap::new();
    targets.insert("Bank".to_owned(), vec![4]);
    targets.insert("homestead".to_owned(), vec![5, 3]);
    targets.insert("home".to_owned(), vec![4]);
    targets.insert("nowhere".to_owned(), vec![999]);
    let to = |what: &str| destination(&map, &a("Warrior"), RoomId(1), what, &targets);

    // The player's `Bank` is room 4, and it wins over the tag, which is 5.
    assert_eq!(to("bank"), Some(RoomId(4)));
    // Exactly `home`, though `homestead` begins with it and sorts after.
    assert_eq!(to("HOME"), Some(RoomId(4)));
    // `homes` is nobody's name, and begins one: the nearest of its rooms.
    assert_eq!(to("homes"), Some(RoomId(3)));
    // A target whose rooms the map does not have is nowhere to go.
    assert_eq!(to("nowhere"), None);
    // A number is still a number.
    assert_eq!(to("5"), Some(RoomId(5)));
}

#[test]
fn the_guild_is_this_characters_guild() {
    let rooms = r#"[
      {"id":1,"exits":[{"to":2,"kind":"cardinal","cmd":"east","cost":1},
                       {"to":3,"kind":"cardinal","cmd":"west","cost":9}]},
      {"id":2,"tags":["warrior guild"]},
      {"id":3,"tags":["wizard guild","wizard guild shop"]}
    ]"#;
    let map = map_of(rooms).unwrap();
    let none = BTreeMap::new();
    let to = |who: &str, what: &str| destination(&map, &a(who), RoomId(1), what, &none);
    assert_eq!(to("Warrior", "guild"), Some(RoomId(2)));
    // The nearer guild is not the wizard's.
    assert_eq!(to("Wizard", "guild"), Some(RoomId(3)));
    assert_eq!(to("Wizard", "Guild Shop"), Some(RoomId(3)));
    assert_eq!(to("Warrior", "guild shop"), None, "warriors have none here");
}
