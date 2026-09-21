//! `Puzzle::FamiliarDoors`: the stone doors a familiar opens, room 6486.
//!
//! Upstream (`upstream_scripts/familiar_doors.rb`): `go stone doors`, and if
//! that lands, done. Otherwise, for a walker who knows Call Familiar (920):
//!
//! 1. A familiar already out is told to `return`, and waited on for up to
//!    fifteen seconds to show among the room's creatures; else 920 is cast.
//! 2. `tell familiar to watch`. If it was not watching already, it is told to
//!    stop at the end.
//! 3. Four laps: in (`doors,w,s,arch,ne,e,privy,hole,w` the first time,
//!    `nw,w,w,n` after), on through the four ring doors, `get rocks`, back
//!    out, `drop basket`. Each `go` the familiar cannot make sense of marks
//!    the run failed, and **the list it is in is still finished** before the
//!    laps stop (upstream's `next`, then `break if fail`). A door it cannot
//!    enter has it `get` that door's ring -- bronze and copper are crossed --
//!    and go again.
//! 4. With `mapdb_hate_familiar` set, the familiar is sent home to stay.
//! 5. `go doors`, whatever happened. A walker without 920 does only this.
//!
//! # Where this is not upstream
//!
//! - An answer to `go` is looked for in what came back, and only then waited
//!   for, upstream's fifteen seconds.
//! - The familiar is looked for every half second, not every tenth.
//! - A `return` that is not understood kills upstream's script (`nil` has no
//!   `captures`), as does a door with no ring in the table. Here neither is
//!   waited on, and the routine goes on.
//! - The wait for mana is bounded (`casting`).

use std::collections::VecDeque;

use cena_map::{Action, Step};

use super::casting::{self, MANA_WAIT_MS, MANA_WAITS};
use super::{Next, Seen, Solver};

const FAMILIAR: u16 = 920;
const IN_FIRST: &str = "doors,w,s,arch,ne,e,privy,hole,w";
const IN_AGAIN: &str = "nw,w,w,n";
const TO_ROCKS: &str = "w,w,sw,iron door,steel door,bronze door,copper door";
const TO_BASKET: &str = "copper door,bronze door,steel door,iron door,ne,e,e,s,e,e,se";
const LAPS: usize = 4;
const ANSWERS: [&str; 4] = [
    "You sense",
    "just squeezed between the stone doors",
    "Obvious exits",
    "Obvious paths",
];
const ANSWER_MS: u64 = 15_000;
const LOOK_MS: u64 = 500;
const LOOKS: u32 = 30;

/// The ring that opens each door.
fn ring_for(door: &str) -> Option<&'static str> {
    Some(match door {
        "iron" => "iron",
        "steel" => "steel",
        "bronze" => "copper",
        "copper" => "bronze",
        _ => return None,
    })
}

enum Op {
    Go(&'static str),
    Say(&'static str),
    /// `break if fail`
    Check,
}

/// Where one `tell familiar to go` stands.
enum Going {
    Sent,
    Waited,
    /// The ring was sent for.
    Ring,
    /// Gone again, whose answer upstream does not read.
    Again,
}

#[derive(Default)]
enum At {
    #[default]
    Start,
    Tried,
    Mana,
    Recalled,
    /// Looking for the familiar by this name, this many times so far.
    Coming(String, u32),
    /// `watch` is next.
    Watch,
    /// `watch` was sent.
    Watching,
    Working,
    /// What is left to ask for, the next first.
    Ending(VecDeque<Next>),
}

#[derive(Default)]
pub(super) struct FamiliarDoors {
    at: At,
    waits: u32,
    ops: VecDeque<Op>,
    going: Option<(&'static str, Going)>,
    failed: bool,
    stop_watching: bool,
}

fn laps() -> VecDeque<Op> {
    let mut ops = VecDeque::new();
    let walk = |ops: &mut VecDeque<Op>, list: &'static str| {
        ops.extend(list.split(',').map(Op::Go));
        ops.push_back(Op::Check);
    };
    for lap in 0..LAPS {
        walk(&mut ops, if lap == 0 { IN_FIRST } else { IN_AGAIN });
        walk(&mut ops, TO_ROCKS);
        ops.push_back(Op::Say("tell familiar to get rocks"));
        walk(&mut ops, TO_BASKET);
        ops.push_back(Op::Say("tell familiar to drop basket"));
    }
    ops
}

/// The name in `You sense understanding from your <name>.`
fn understood_by(seen: &Seen<'_>) -> Option<String> {
    seen.answer.iter().find_map(|line| {
        let text = line.text();
        let name = text.strip_prefix("You sense understanding from your ")?;
        Some(name[..name.find('.')?].to_owned())
    })
}

/// The door in `You sense that your … is not able to enter the <door> door`.
fn barred_door(seen: &Seen<'_>) -> Option<String> {
    seen.answer.iter().find_map(|line| {
        let text = line.text();
        if !text.starts_with("You sense that your ") {
            return None;
        }
        let door = text.split_once(" is not able to enter the ")?.1;
        Some(door[..door.find(" door")?].to_owned())
    })
}

impl FamiliarDoors {
    fn watch(&mut self) -> Next {
        self.at = At::Watching;
        Next::Put("tell familiar to watch".into())
    }

    fn ending(&mut self, seen: &Seen<'_>) -> Next {
        let mut left = VecDeque::new();
        if self.stop_watching {
            left.push_back(Next::Put("tell familiar to stop watching".into()));
        }
        let hates = seen
            .walker
            .settings
            .get("mapdb_hate_familiar")
            .is_some_and(|value| !matches!(value.trim(), "" | "false" | "nil"));
        if hates {
            left.push_back(Next::Pause(300));
            left.push_back(Next::Put("tell familiar to return".into()));
            left.push_back(Next::Put("tell familiar to stay".into()));
        }
        left.push_back(Next::Go("go doors".into()));
        self.at = At::Ending(left);
        self.next(seen)
    }

    /// The answer to a `go` is in: what it means, and what is next if
    /// anything.
    fn judge(&mut self, way: &'static str, seen: &Seen<'_>) -> Option<Next> {
        let sensed = ANSWERS.iter().any(|line| seen.answered(line));
        if !sensed || seen.answered("You sense confusion") {
            self.failed = true;
            return None;
        }
        let ring = ring_for(&barred_door(seen)?)?;
        self.going = Some((way, Going::Ring));
        Some(Next::Put(format!("tell familiar to get {ring} ring")))
    }

    fn work(&mut self, seen: &Seen<'_>) -> Next {
        if let Some((way, going)) = self.going.take() {
            let sensed = ANSWERS.iter().any(|line| seen.answered(line));
            match going {
                Going::Sent if !sensed => {
                    self.going = Some((way, Going::Waited));
                    return Next::Await(ANSWERS.map(str::to_owned).into(), ANSWER_MS);
                }
                Going::Sent | Going::Waited => {
                    if let Some(next) = self.judge(way, seen) {
                        return next;
                    }
                }
                Going::Ring => {
                    self.going = Some((way, Going::Again));
                    return Next::Put(format!("tell familiar to go {way}"));
                }
                Going::Again => {}
            }
        }
        while let Some(op) = self.ops.pop_front() {
            match op {
                Op::Go(way) => {
                    self.going = Some((way, Going::Sent));
                    return Next::Put(format!("tell familiar to go {way}"));
                }
                Op::Say(command) => return Next::Put(command.to_owned()),
                Op::Check if self.failed => self.ops.clear(),
                Op::Check => {}
            }
        }
        self.ending(seen)
    }
}

impl Solver for FamiliarDoors {
    fn next(&mut self, seen: &Seen<'_>) -> Next {
        match &mut self.at {
            At::Start => {
                self.at = At::Tried;
                Next::Put("go stone doors".into())
            }
            At::Tried => {
                if seen.here == Some(seen.goal) || seen.answered("Obvious") {
                    return Next::Done;
                }
                if !casting::knows(seen, FAMILIAR) {
                    return self.ending(seen);
                }
                if casting::active(seen, FAMILIAR) {
                    self.at = At::Recalled;
                    return Next::Put("tell familiar to return".into());
                }
                self.at = At::Mana;
                self.next(seen)
            }
            At::Mana => {
                let Some(name) = casting::name_of(FAMILIAR) else {
                    return Next::Failed;
                };
                if !casting::affords(seen, FAMILIAR) {
                    self.waits += 1;
                    if self.waits > MANA_WAITS {
                        return Next::Stop(
                            "Waited ten minutes for the mana to call a familiar.".to_owned(),
                        );
                    }
                    return Next::Pause(MANA_WAIT_MS);
                }
                self.at = At::Watch;
                Next::Steps(vec![Step {
                    action: Action::Cast(name.to_owned()),
                    when: None,
                }])
            }
            At::Recalled => match understood_by(seen) {
                Some(name) => {
                    self.at = At::Coming(name, 0);
                    self.next(seen)
                }
                None => self.watch(),
            },
            At::Coming(name, looks) => {
                let here = seen
                    .state
                    .room
                    .creatures
                    .iter()
                    .any(|creature| creature.text.contains(name.as_str()));
                if here || *looks >= LOOKS {
                    return self.watch();
                }
                *looks += 1;
                Next::Pause(LOOK_MS)
            }
            At::Watch => self.watch(),
            At::Watching => {
                self.stop_watching = seen.answered("You sense understanding");
                self.ops = laps();
                self.at = At::Working;
                self.work(seen)
            }
            At::Working => self.work(seen),
            At::Ending(left) => left.pop_front().unwrap_or(Next::Done),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::super::casting::tests::{affording, knowing};
    use super::super::testing::Scene;
    use super::*;

    const SENSED: &[&str] = &["You sense understanding from your cat."];

    fn put(command: &str) -> Next {
        Next::Put(command.to_owned())
    }

    fn go(way: &str) -> Next {
        put(&format!("tell familiar to go {way}"))
    }

    fn knows() -> Scene {
        knowing(Scene::at(1, 2), &[920])
    }

    fn sensed() -> Scene {
        knows().answered(SENSED)
    }

    /// Through the cast and `watch`, to the first `go`. `watching`: the
    /// familiar already was.
    fn at_work(watching: bool) -> FamiliarDoors {
        let mut doors = FamiliarDoors::default();
        assert_eq!(knows().ask(&mut doors), put("go stone doors"));
        let spell = casting::name_of(920).unwrap().to_owned();
        assert_eq!(
            knows().answered(&["You cannot do that."]).ask(&mut doors),
            Next::Steps(vec![Step {
                action: Action::Cast(spell),
                when: None
            }])
        );
        assert_eq!(knows().ask(&mut doors), put("tell familiar to watch"));
        let answer = if watching {
            knows().answered(&["Your cat is already watching."])
        } else {
            sensed()
        };
        assert_eq!(answer.ask(&mut doors), go("doors"));
        doors
    }

    /// Answer each `go` well: the first of `ways` is already asked for.
    fn walk(doors: &mut FamiliarDoors, ways: &str) {
        for way in ways.split(',').skip(1) {
            assert_eq!(sensed().ask(doors), go(way));
        }
    }

    #[test]
    fn doors_that_open_need_no_familiar() {
        let mut doors = FamiliarDoors::default();
        knows().ask(&mut doors);
        assert_eq!(knowing(Scene::at(2, 2), &[920]).ask(&mut doors), Next::Done);
        let mut doors = FamiliarDoors::default();
        knows().ask(&mut doors);
        assert_eq!(
            knows().answered(&["Obvious exits: west"]).ask(&mut doors),
            Next::Done
        );
    }

    #[test]
    fn a_walker_with_no_familiar_just_tries_the_doors() {
        let mut doors = FamiliarDoors::default();
        Scene::at(1, 2).ask(&mut doors);
        assert_eq!(Scene::at(1, 2).ask(&mut doors), Next::Go("go doors".into()));
        assert_eq!(Scene::at(1, 2).ask(&mut doors), Next::Done);
    }

    #[test]
    fn four_laps_then_stop_watching_and_go() {
        let mut doors = at_work(false);
        for lap in 0..LAPS {
            if lap == 0 {
                walk(&mut doors, IN_FIRST);
            } else {
                assert_eq!(knows().ask(&mut doors), go("nw"));
                walk(&mut doors, IN_AGAIN);
            }
            assert_eq!(sensed().ask(&mut doors), go("w"));
            walk(&mut doors, TO_ROCKS);
            assert_eq!(sensed().ask(&mut doors), put("tell familiar to get rocks"));
            assert_eq!(knows().ask(&mut doors), go("copper door"));
            walk(&mut doors, TO_BASKET);
            assert_eq!(
                sensed().ask(&mut doors),
                put("tell familiar to drop basket")
            );
        }
        assert_eq!(
            knows().ask(&mut doors),
            put("tell familiar to stop watching")
        );
        assert_eq!(knows().ask(&mut doors), Next::Go("go doors".into()));
        assert_eq!(knows().ask(&mut doors), Next::Done);
    }

    #[test]
    fn a_barred_door_has_the_crossed_ring_fetched_and_is_gone_at_again() {
        let mut doors = at_work(false);
        let barred =
            knows().answered(&["You sense that your cat is not able to enter the bronze door."]);
        assert_eq!(
            barred.ask(&mut doors),
            put("tell familiar to get copper ring")
        );
        assert_eq!(sensed().ask(&mut doors), go("doors"));
        // The second answer is not read: even confusion goes on.
        assert_eq!(
            knows().answered(&["You sense confusion."]).ask(&mut doors),
            go("w")
        );
        assert!(!doors.failed);
    }

    #[test]
    fn every_door_has_its_ring() {
        for (door, ring) in [
            ("iron", "iron"),
            ("steel", "steel"),
            ("bronze", "copper"),
            ("copper", "bronze"),
        ] {
            assert_eq!(ring_for(door), Some(ring));
        }
    }

    #[test]
    fn confusion_finishes_the_list_and_then_stops_the_laps() {
        let mut doors = at_work(true);
        assert_eq!(
            knows().answered(&["You sense confusion."]).ask(&mut doors),
            go("w")
        );
        walk(&mut doors, "w,s,arch,ne,e,privy,hole,w");
        // The list is done: no rocks, no watching to stop, just the doors.
        assert_eq!(sensed().ask(&mut doors), Next::Go("go doors".into()));
    }

    #[test]
    fn silence_is_waited_on_and_then_is_failure() {
        let mut doors = at_work(true);
        assert_eq!(
            knows().ask(&mut doors),
            Next::Await(ANSWERS.map(str::to_owned).into(), ANSWER_MS)
        );
        assert_eq!(knows().ask(&mut doors), go("w"));
        assert!(doors.failed);

        let mut doors = at_work(true);
        knows().ask(&mut doors);
        assert_eq!(
            knows()
                .answered(&["Your cat just squeezed between the stone doors."])
                .ask(&mut doors),
            go("w")
        );
        assert!(!doors.failed);
    }

    fn out_already() -> FamiliarDoors {
        let mut scene = knows();
        scene.walker.active_spells = scene.walker.known_spells.clone();
        let mut doors = FamiliarDoors::default();
        scene.ask(&mut doors);
        assert_eq!(scene.ask(&mut doors), put("tell familiar to return"));
        doors
    }

    #[test]
    fn a_familiar_already_out_is_called_back_and_looked_for() {
        let mut doors = out_already();
        assert_eq!(sensed().ask(&mut doors), Next::Pause(LOOK_MS));
        let mut arrived = knows();
        arrived.state.room.creatures.push(cena_session::RoomItem {
            id: "7".into(),
            noun: "cat".into(),
            text: "a grey cat".into(),
            before: None,
            after: None,
            status: None,
        });
        assert_eq!(arrived.ask(&mut doors), put("tell familiar to watch"));
    }

    #[test]
    fn a_familiar_that_never_shows_is_not_looked_for_for_ever() {
        let mut doors = out_already();
        for _ in 0..LOOKS {
            assert_eq!(sensed().ask(&mut doors), Next::Pause(LOOK_MS));
        }
        assert_eq!(sensed().ask(&mut doors), put("tell familiar to watch"));
    }

    #[test]
    fn a_return_not_understood_goes_straight_to_watch() {
        let mut doors = out_already();
        assert_eq!(knows().ask(&mut doors), put("tell familiar to watch"));
    }

    #[test]
    fn mana_is_waited_for_and_not_for_ever() {
        let mut doors = FamiliarDoors::default();
        let poor = || affording(knows(), &[]);
        poor().ask(&mut doors);
        for _ in 0..MANA_WAITS {
            assert_eq!(poor().ask(&mut doors), Next::Pause(MANA_WAIT_MS));
        }
        assert!(matches!(poor().ask(&mut doors), Next::Stop(_)));
    }

    #[test]
    fn a_player_who_hates_the_familiar_has_it_sent_home() {
        let mut doors = FamiliarDoors::default();
        let mut scene = Scene::at(1, 2);
        scene
            .walker
            .settings
            .insert("mapdb_hate_familiar".into(), "true".into());
        scene.ask(&mut doors);
        let asked: Vec<Next> = (0..5).map(|_| scene.ask(&mut doors)).collect();
        assert_eq!(
            asked,
            [
                Next::Pause(300),
                put("tell familiar to return"),
                put("tell familiar to stay"),
                Next::Go("go doors".into()),
                Next::Done,
            ]
        );
    }
}
