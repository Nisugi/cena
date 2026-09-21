//! Crossings that are lists of steps: `cena_behavior::travel`, stage 3
//! (`plan/24` §3). The steps are written as the converter writes them, in
//! JSON, so these tests start from what the map will actually hold.

use cena_behavior::travel::{Deed, EXCHANGE_TIMEOUT_MS, Now, Said, Trip};
use cena_map::{Map, Room, RoomId, Walker};

/// One exit, 1 -> 2, crossed by `steps`; and a plain long way round by 3.
fn map(steps: &str) -> Option<Map> {
    let rooms = format!(
        r#"[
      {{"id":1,"exits":[{{"to":2,"kind":"scripted","steps":{steps},"cost":1}},
                        {{"to":3,"kind":"cardinal","cmd":"east","cost":50}}]}},
      {{"id":3,"exits":[{{"to":2,"kind":"cardinal","cmd":"north","cost":50}}]}},
      {{"id":2}}
    ]"#
    );
    let rooms: Vec<Room> = serde_json::from_str(&rooms).ok()?;
    Map::from_rooms(rooms).ok()
}

fn at(room: u32, ms: u64) -> Now {
    Now {
        here: Some(RoomId(room)),
        ms,
    }
}

fn send(command: &str) -> Said {
    Said::Send(command.to_owned())
}

/// `open door`, answered by a prompt; then the move.
#[test]
fn a_put_waits_for_its_answer_and_then_the_move_is_made() {
    let map = map(r#"[{"put":"open door"},{"move":"go door"}]"#).unwrap();
    let walker = Walker::default();
    let mut trip = Trip::to(RoomId(2));
    assert_eq!(trip.tick(&map, &walker, at(1, 0)), send("open door"));
    assert_eq!(trip.tick(&map, &walker, at(1, 100)), Said::Hold);
    trip.prompted();
    assert_eq!(trip.tick(&map, &walker, at(1, 200)), send("go door"));
    assert_eq!(trip.tick(&map, &walker, at(2, 300)), Said::Arrived);
}

/// A game that never prompts does not hang the walk.
#[test]
fn an_exchange_nobody_answers_is_taken_as_answered() {
    let map = map(r#"[{"put":"open door"},{"move":"go door"}]"#).unwrap();
    let walker = Walker::default();
    let mut trip = Trip::to(RoomId(2));
    trip.tick(&map, &walker, at(1, 0));
    assert_eq!(
        trip.tick(&map, &walker, at(1, EXCHANGE_TIMEOUT_MS)),
        send("go door")
    );
}

/// The icy path: cast when able, pause when the profile says to wait, move.
/// **The guard is asked when the step is reached.**
#[test]
fn a_guard_is_asked_when_its_step_is_reached() {
    let steps = r#"[
      {"cast":"Sigil of Resolve","when":{"spell_known":"Sigil of Resolve"}},
      {"pause":4200,"when":{"setting":["ice_mode","wait"]}},
      {"move":"north"}]"#;
    let map = map(steps).unwrap();

    // Knows nothing: no cast, no pause, just the move.
    let mut trip = Trip::to(RoomId(2));
    assert_eq!(trip.tick(&map, &Walker::default(), at(1, 0)), send("north"));

    let mut careful = Walker {
        known_spells: Some(["Sigil of Resolve".to_owned()].into()),
        ..Walker::default()
    };
    careful.settings.insert("ice_mode".into(), "wait".into());
    let mut trip = Trip::to(RoomId(2));
    assert_eq!(
        trip.tick(&map, &careful, at(1, 0)),
        Said::Do(Deed::Cast("Sigil of Resolve".into()))
    );
    assert_eq!(trip.tick(&map, &careful, at(1, 100)), Said::Hold);
    assert_eq!(trip.tick(&map, &careful, at(1, 4299)), Said::Hold);
    assert_eq!(trip.tick(&map, &careful, at(1, 4300)), send("north"));
}

/// `try_move`, then the rest only if it did not move the walker.
#[test]
fn still_here_is_true_until_the_crossing_moves_the_walker() {
    let steps = r#"[
      {"try_move":"go curtain"},
      {"put":"close locker","when":"still_here"},
      {"move":"go curtain","when":"still_here"}]"#;
    let map = map(steps).unwrap();
    let walker = Walker::default();

    // The curtain opens first time: nothing more is sent.
    let mut trip = Trip::to(RoomId(2));
    assert_eq!(trip.tick(&map, &walker, at(1, 0)), send("go curtain"));
    assert_eq!(trip.tick(&map, &walker, at(2, 100)), Said::Arrived);

    // It does not: shut the locker and go again.
    let mut trip = Trip::to(RoomId(2));
    trip.tick(&map, &walker, at(1, 0));
    trip.prompted();
    assert_eq!(trip.tick(&map, &walker, at(1, 100)), send("close locker"));
    trip.prompted();
    assert_eq!(trip.tick(&map, &walker, at(1, 200)), send("go curtain"));
}

#[test]
fn a_search_goes_on_until_the_game_says_the_way_is_found() {
    let steps = r#"[
      {"put_until":{"command":"search","until":["discover a path"]}},
      {"move":"go path"}]"#;
    let map = map(steps).unwrap();
    let walker = Walker::default();
    let mut trip = Trip::to(RoomId(2));
    assert_eq!(trip.tick(&map, &walker, at(1, 0)), send("search"));
    trip.heard("You don't find anything of interest.");
    trip.prompted();
    assert_eq!(trip.tick(&map, &walker, at(1, 100)), send("search"));
    // Roundtime refuses the second search: it is waited out and sent again.
    trip.heard("...wait 2 seconds.");
    trip.prompted();
    assert_eq!(trip.tick(&map, &walker, at(1, 200)), Said::Hold);
    assert_eq!(trip.tick(&map, &walker, at(1, 2000)), send("search"));
    trip.heard("You discover a path leading off through the brush!");
    trip.prompted();
    assert_eq!(trip.tick(&map, &walker, at(1, 2100)), send("go path"));
}

/// Rowing: the same command until the room changes, however long it takes --
/// but not for ever.
#[test]
fn keep_moving_stops_when_the_room_changes_and_gives_up_in_the_end() {
    let map = map(r#"[{"keep_moving":"row north"}]"#).unwrap();
    let walker = Walker::default();
    let mut trip = Trip::to(RoomId(2));
    let mut ms = 0;
    for _ in 0..5 {
        assert_eq!(trip.tick(&map, &walker, at(1, ms)), send("row north"));
        trip.prompted();
        ms += 100;
    }
    assert_eq!(trip.tick(&map, &walker, at(2, ms)), Said::Arrived);

    // A boat that never gets there: the exit is given up, and the long way
    // round is taken.
    let mut trip = Trip::to(RoomId(2));
    let mut sent = 0;
    let gave_up = loop {
        match trip.tick(&map, &walker, at(1, ms)) {
            Said::Send(command) if command == "row north" => sent += 1,
            other => break other,
        }
        trip.prompted();
        ms += 100;
        assert!(sent <= 1000, "never gave up");
    };
    assert_eq!(gave_up, send("east"));
}

/// A maze is walked at random, and the same seed walks it the same way.
#[test]
fn a_seeded_trip_takes_the_same_turns_every_time() {
    let steps = r#"[{"move_any_while":[["northwest","southwest","northeast"],
                     {"exits_are":["ne","se","sw","nw"]}]}]"#;
    let map = map(steps).unwrap();
    let lost = Walker {
        exits: Some(["ne", "se", "sw", "nw"].map(str::to_owned).to_vec()),
        ..Walker::default()
    };
    let turns = |seed: u64| {
        let mut trip = Trip::seeded(RoomId(2), seed);
        let mut taken = Vec::new();
        for ms in 0..12 {
            if let Said::Send(command) = trip.tick(&map, &lost, at(1, ms * 100)) {
                taken.push(command);
            }
            trip.prompted();
        }
        taken
    };
    assert_eq!(turns(7), turns(7));
    assert_eq!(turns(7).len(), 12);
    assert_ne!(turns(7), turns(8), "another seed, another walk");
}

/// Hands emptied by a step are owed back -- when the steps give them back,
/// and when they never get the chance.
#[test]
fn what_a_crossing_changed_is_put_back_before_the_trip_is_over() {
    let steps = r#"[{"empty_hands":null},{"move":"climb rope"}]"#;
    let map = map(steps).unwrap();
    let walker = Walker::default();
    let mut trip = Trip::to(RoomId(2));
    assert_eq!(
        trip.tick(&map, &walker, at(1, 0)),
        Said::Do(Deed::EmptyHands)
    );
    assert_eq!(trip.tick(&map, &walker, at(1, 100)), send("climb rope"));
    // Arrived -- but the steps never filled the hands. The trip does, first.
    assert_eq!(
        trip.tick(&map, &walker, at(2, 200)),
        Said::Do(Deed::FillHands)
    );
    assert_eq!(trip.tick(&map, &walker, at(2, 300)), Said::Arrived);

    // A user's stop, part-way: the driver asks what is owed.
    let mut trip = Trip::to(RoomId(2));
    trip.tick(&map, &walker, at(1, 0));
    assert_eq!(trip.owed(), [Deed::FillHands]);
    assert_eq!(trip.owed(), [], "and it is owed once");
}

/// "Your hands are full" is a remedy now: empty them, move, give them back.
#[test]
fn full_hands_are_emptied_for_the_move_and_given_back_after() {
    let map = map(r#"[{"move":"climb rope"},{"put":"look"}]"#).unwrap();
    let walker = Walker::default();
    let mut trip = Trip::to(RoomId(2));
    trip.tick(&map, &walker, at(1, 0));
    trip.heard("You'll need empty hands to climb that.");
    assert_eq!(
        trip.tick(&map, &walker, at(1, 100)),
        Said::Do(Deed::EmptyHands)
    );
    assert_eq!(trip.tick(&map, &walker, at(1, 200)), send("climb rope"));
    assert_eq!(
        trip.tick(&map, &walker, at(2, 300)),
        Said::Do(Deed::FillHands)
    );
}

/// Too injured to climb: Lich casts Sigil of Resolve, if the walker has it.
#[test]
fn resolve_is_cast_for_a_walker_too_injured_to_climb() {
    let map = map(r#"[{"move":"climb rope"}]"#).unwrap();
    let line = "You are too injured to be doing any climbing!";

    let voln = Walker {
        known_spells: Some(["Sigil of Resolve".to_owned()].into()),
        ..Walker::default()
    };
    let mut trip = Trip::to(RoomId(2));
    trip.tick(&map, &voln, at(1, 0));
    trip.heard(line);
    assert_eq!(
        trip.tick(&map, &voln, at(1, 100)),
        Said::Do(Deed::Cast("Sigil of Resolve".into()))
    );
    assert_eq!(trip.tick(&map, &voln, at(1, 200)), send("climb rope"));

    // Without it the rope is left for this trip, and the long way taken.
    let mut trip = Trip::to(RoomId(2));
    trip.tick(&map, &Walker::default(), at(1, 0));
    trip.heard(line);
    assert_eq!(
        trip.tick(&map, &Walker::default(), at(1, 100)),
        send("east")
    );
}

/// A key's sack is the owner's to name, and an exit that needs it is not
/// offered to a walker whose profile does not.
#[test]
fn a_setting_is_filled_into_the_command() {
    let steps = r#"[{"put":"get my key from my {setting:key_sack}"},{"move":"go door"}]"#;
    let map = map(steps).unwrap();
    let mut owner = Walker::default();
    owner.settings.insert("key_sack".into(), "cloak".into());
    let mut trip = Trip::to(RoomId(2));
    assert_eq!(
        trip.tick(&map, &owner, at(1, 0)),
        send("get my key from my cloak")
    );
}

/// Steps this build cannot run yet are gone round, not failed at.
#[test]
fn a_crossing_with_a_step_not_yet_written_is_gone_round() {
    let map = map(r#"[{"speak":"wizard"},{"move":"say ::door wizard"}]"#).unwrap();
    let mut trip = Trip::to(RoomId(2));
    assert_eq!(trip.tick(&map, &Walker::default(), at(1, 0)), send("east"));
}

/// Upstream's `$go2_restart`: the crossing may land somewhere else.
#[test]
fn replan_plans_again_unless_the_walker_is_where_it_meant_to_be() {
    let map = map(r#"[{"move":"jump"},{"replan":null}]"#).unwrap();
    let walker = Walker::default();
    let mut trip = Trip::to(RoomId(2));
    trip.tick(&map, &walker, at(1, 0));
    // Landed in 3, not 2: planned again from there.
    assert_eq!(trip.tick(&map, &walker, at(3, 100)), send("north"));
    assert_eq!(trip.replans(), 1);
}

/// Upstream climbs a ledge with empty hands and only fills them at the top of
/// the *next* climb. What one crossing owes, the next inherits -- and the
/// trip does not give the hands back in between.
#[test]
fn what_one_crossing_owes_the_next_one_can_pay() {
    let rooms = r#"[
      {"id":1,"exits":[{"to":2,"kind":"scripted","cost":1,
                        "steps":[{"empty_hands":null},{"move":"climb ledge"}]}]},
      {"id":2,"exits":[{"to":3,"kind":"scripted","cost":1,
                        "steps":[{"move":"climb footholds"},{"fill_hands":null}]}]},
      {"id":3}
    ]"#;
    let rooms: Vec<Room> = serde_json::from_str(rooms).unwrap();
    let map = Map::from_rooms(rooms).unwrap();
    let walker = Walker::default();
    let mut trip = Trip::to(RoomId(3));
    assert_eq!(
        trip.tick(&map, &walker, at(1, 0)),
        Said::Do(Deed::EmptyHands)
    );
    assert_eq!(trip.tick(&map, &walker, at(1, 100)), send("climb ledge"));
    assert_eq!(
        trip.tick(&map, &walker, at(2, 200)),
        send("climb footholds")
    );
    // At the goal, and the crossing still has a step: it runs before the
    // trip says it has arrived, and the hands are given back exactly once.
    assert_eq!(
        trip.tick(&map, &walker, at(3, 300)),
        Said::Do(Deed::FillHands)
    );
    assert_eq!(trip.tick(&map, &walker, at(3, 400)), Said::Arrived);
    assert_eq!(trip.owed(), []);
}

/// The door at the journey's end is still closed and locked behind the
/// walker: a crossing is finished before the trip says it has arrived.
#[test]
fn the_last_crossing_is_finished_before_arriving() {
    let steps = r#"[{"put":"open door"},{"move":"go door"},{"put":"close door"}]"#;
    let map = map(steps).unwrap();
    let walker = Walker::default();
    let mut trip = Trip::to(RoomId(2));
    trip.tick(&map, &walker, at(1, 0));
    trip.prompted();
    assert_eq!(trip.tick(&map, &walker, at(1, 100)), send("go door"));
    assert_eq!(trip.tick(&map, &walker, at(2, 200)), send("close door"));
    trip.prompted();
    assert_eq!(trip.tick(&map, &walker, at(2, 300)), Said::Arrived);
}
