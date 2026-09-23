//! The pathfinder: `cena_map::route`.

use cena_map::{Exit, Map, Room, RoomId, Target, as_converted};

/// ```text
///   1 --1s-- 2 --1s-- 3 --1s-- 4        the long way round: 3s
///   1 ----------10s----------- 4        the direct exit: 10s
///   1 --0.1s, scripted-------- 4        unported: not an exit yet
///   4 --no cost--------------- 5        no cost: impassable
///   6                                   an island
/// ```
const ROOMS: &str = r#"[
  {"id":1,"exits":[{"to":2,"kind":"cardinal","cmd":"east","cost":1},
                   {"to":4,"kind":"go","cmd":"go shortcut","cost":10},
                   {"to":4,"kind":"scripted","unported":"abc","cost":0.1},
                   {"to":99,"kind":"cardinal","cmd":"north","cost":1}]},
  {"id":2,"tags":["bank"],"exits":[{"to":3,"kind":"cardinal","cmd":"east","cost":1},
                   {"to":1,"kind":"cardinal","cmd":"west","cost":1}]},
  {"id":3,"exits":[{"to":4,"kind":"cardinal","cmd":"east","cost":1}]},
  {"id":4,"tags":["bank"],"exits":[{"to":5,"kind":"cardinal","cmd":"east"}]},
  {"id":5},
  {"id":6,"tags":["bank"]}
]"#;

fn map() -> Option<Map> {
    let rooms: Vec<Room> = serde_json::from_str(ROOMS).ok()?;
    Map::from_rooms(rooms).ok()
}

fn ids(rooms: &[u32]) -> Vec<RoomId> {
    rooms.iter().copied().map(RoomId).collect()
}

#[test]
fn the_cheapest_way_wins_not_the_fewest_rooms() {
    let map = map().unwrap();
    let routes = map.routes(RoomId(1), Target::Room(RoomId(4)), as_converted);
    assert_eq!(routes.reached(), Some(RoomId(4)));
    assert_eq!(routes.path_to(RoomId(4)), Some(ids(&[2, 3, 4])));
    assert_eq!(routes.seconds_to(RoomId(4)), Some(3.0));
}

#[test]
fn already_there_is_an_empty_path_not_a_failure() {
    let map = map().unwrap();
    let routes = map.routes(RoomId(1), Target::Room(RoomId(1)), as_converted);
    assert_eq!(routes.path_to(RoomId(1)), Some(vec![]));
    assert_eq!(routes.seconds_to(RoomId(1)), Some(0.0));
}

/// `plan/21` §3b: upstream has no default cost, and neither does this.
#[test]
fn an_exit_without_a_cost_and_an_unported_one_are_both_walls() {
    let map = map().unwrap();
    let routes = map.routes(RoomId(1), Target::Everything, as_converted);
    assert_eq!(routes.seconds_to(RoomId(5)), None, "4 -> 5 has no cost");
    assert_eq!(routes.path_to(RoomId(5)), None);
    assert_eq!(routes.seconds_to(RoomId(6)), None, "an island");
    assert_eq!(
        routes.seconds_to(RoomId(4)),
        Some(3.0),
        "the 0.1s scripted exit was not taken"
    );
    assert_eq!(routes.reached(), None, "nothing was being looked for");
}

#[test]
fn rooms_the_map_does_not_have_are_not_reached_and_not_a_panic() {
    let map = map().unwrap();
    let routes = map.routes(RoomId(1), Target::Room(RoomId(99)), as_converted);
    assert_eq!(routes.reached(), None);
    assert_eq!(routes.path_to(RoomId(99)), None);
    let lost = map.routes(RoomId(777), Target::Everything, as_converted);
    assert_eq!(lost.seconds_to(RoomId(777)), None);
    assert_eq!(lost.path_to(RoomId(1)), None);
}

#[test]
fn the_nearest_of_several_is_the_nearest_by_time() {
    let map = map().unwrap();
    let banks = ids(&[6, 4, 2]);
    let routes = map.routes(RoomId(1), Target::Nearest(&banks), as_converted);
    assert_eq!(routes.reached(), Some(RoomId(2)));
    assert_eq!(routes.path_to(RoomId(2)), Some(ids(&[2])));

    // Standing in one of them: it is the nearest, at no distance.
    let routes = map.routes(RoomId(4), Target::Nearest(&banks), as_converted);
    assert_eq!(routes.reached(), Some(RoomId(4)));
    assert_eq!(routes.path_to(RoomId(4)), Some(vec![]));

    // None reachable.
    let island = ids(&[6]);
    let routes = map.routes(RoomId(1), Target::Nearest(&island), as_converted);
    assert_eq!(routes.reached(), None);
}

/// The point of taking the pricing as a value: two characters, one map, two
/// answers, and nothing shared between the searches.
#[test]
fn the_same_map_routes_differently_for_different_walkers() {
    let map = map().unwrap();
    let has_a_pass = |from: &Room, exit: &Exit| {
        if from.id == RoomId(1) && exit.to == RoomId(4) {
            Some(0.5)
        } else {
            as_converted(from, exit)
        }
    };
    let banned = |from: &Room, exit: &Exit| {
        as_converted(from, exit).filter(|_| (from.id, exit.to) != (RoomId(2), RoomId(3)))
    };
    let with_pass = map.routes(RoomId(1), Target::Room(RoomId(4)), has_a_pass);
    let after_a_failure = map.routes(RoomId(1), Target::Room(RoomId(4)), banned);
    let plain = map.routes(RoomId(1), Target::Room(RoomId(4)), as_converted);

    assert_eq!(with_pass.path_to(RoomId(4)), Some(ids(&[4])));
    assert_eq!(with_pass.seconds_to(RoomId(4)), Some(0.5));
    assert_eq!(after_a_failure.path_to(RoomId(4)), Some(ids(&[4])));
    assert_eq!(after_a_failure.seconds_to(RoomId(4)), Some(10.0));
    assert_eq!(plain.path_to(RoomId(4)), Some(ids(&[2, 3, 4])));
}

#[test]
fn a_price_that_would_corrupt_the_search_is_a_wall() {
    let map = map().unwrap();
    for bad in [-1.0, f64::NAN, f64::INFINITY] {
        let routes = map.routes(RoomId(1), Target::Everything, |_, _| Some(bad));
        assert_eq!(routes.seconds_to(RoomId(2)), None, "{bad}");
    }
    let free = map.routes(RoomId(1), Target::Everything, |_, _| Some(0.0));
    assert_eq!(
        free.seconds_to(RoomId(5)),
        Some(0.0),
        "zero is a fair price"
    );
}

/// ```text
///   1 --1s-- 2 (the target)
///   1 --5s-- 3          discovered at 5s before the search stops...
///   2 --1s-- 3          ...but 2s is the real answer, never looked at
/// ```
#[test]
fn a_room_the_search_stopped_short_of_is_not_answered_for() {
    // **A tentative distance is not a distance.** Stopping at room 2 leaves
    // room 3 discovered at 5s, one step past the frontier; the shortest way
    // is 2s, through the target. Answering 5s -- and the direct path -- would
    // be a longer route presented as the shortest.
    let rooms: Vec<Room> = serde_json::from_str(
        r#"[
      {"id":1,"exits":[{"to":2,"kind":"cardinal","cmd":"east","cost":1},
                       {"to":3,"kind":"cardinal","cmd":"north","cost":5}]},
      {"id":2,"exits":[{"to":3,"kind":"cardinal","cmd":"north","cost":1}]},
      {"id":3}
    ]"#,
    )
    .unwrap();
    let map = Map::from_rooms(rooms).unwrap();
    let stopped = map.routes(RoomId(1), Target::Room(RoomId(2)), as_converted);
    assert_eq!(
        stopped.seconds_to(RoomId(2)),
        Some(1.0),
        "the target is settled"
    );
    assert_eq!(stopped.seconds_to(RoomId(3)), None, "not known, not 5s");
    assert_eq!(stopped.path_to(RoomId(3)), None);
    // The same question with nowhere to stop gets the real answer.
    let whole = map.routes(RoomId(1), Target::Everything, as_converted);
    assert_eq!(whole.seconds_to(RoomId(3)), Some(2.0));
    assert_eq!(whole.path_to(RoomId(3)), Some(ids(&[2, 3])));
}

#[test]
fn the_route_names_which_of_two_parallel_exits_it_took() {
    // A gate and a wall into the same room, at different prices. `path_to`
    // says only "room 2"; `exits_to` says the wall -- index 1 -- so a caller
    // does not re-price both to rediscover the search's choice.
    let rooms: Vec<Room> = serde_json::from_str(
        r#"[
      {"id":1,"exits":[{"to":2,"kind":"go","cmd":"go gate","cost":5},
                       {"to":2,"kind":"climb","cmd":"climb wall","cost":1}]},
      {"id":2,"exits":[{"to":1,"kind":"go","cmd":"go gate","cost":5}]}
    ]"#,
    )
    .unwrap();
    let map = Map::from_rooms(rooms).unwrap();
    let routes = map.routes(RoomId(1), Target::Room(RoomId(2)), as_converted);
    assert_eq!(routes.path_to(RoomId(2)), Some(ids(&[2])));
    assert_eq!(
        routes.exits_to(RoomId(2)),
        Some(vec![(RoomId(1), 1, RoomId(2))])
    );
    assert_eq!(routes.exits_to(RoomId(1)), Some(vec![]), "already there");
}
