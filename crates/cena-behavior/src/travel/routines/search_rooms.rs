//! `Routine::SearchRooms`: a portal or a doorframe that wanders.
//!
//! Upstream (`upstream_scripts/bleak_portal.rb`, `doorframe.rb`; recogniser
//! `wandering_way`): for each room of a list, `go2` there -- unless already
//! there -- and look at `GameObj.loot`; stop at the first room that shows the
//! thing, and `move` into it. The portal's rooms are the game's numbers
//! (`Room["u#{r}"].id`), resolved against the map once, in [`resolve`]; the
//! portal ends with `$go2_restart = true`, which is `Next::Done`.
//!
//! **Deviations.** Upstream tells the portal by its whole name and the
//! doorframe by its noun; `cena_map::Routine::SearchRooms` has one `sees`, so
//! a thing matches when its name holds `sees` or its noun is `sees`. When no
//! room shows it upstream sends the move anyway, which can only fail; this
//! gives the exit up without sending it. A uid the map does not know is
//! skipped, where upstream would raise on `nil.id`. A `go2` that fails is
//! treated as upstream treats it: look where the walker stands and go on.

use cena_map::{Map, RoomId, Uid};

use super::{Next, Seen, Solver};

pub(super) struct SearchRooms {
    rooms: Vec<RoomId>,
    sees: String,
    enter: String,
    /// The room of `rooms` being searched.
    index: usize,
    /// Whether this room has been walked to, so it is only to be looked at.
    walked: bool,
    entered: bool,
}

/// The map's ids for a routine's rooms. `by_uid`: they are the game's
/// numbers, and the first room the map has by that number is meant.
pub(super) fn resolve(rooms: &[u32], by_uid: bool, map: &Map) -> Vec<RoomId> {
    rooms
        .iter()
        .filter_map(|room| {
            if by_uid {
                map.ids_for_uid(Uid(i64::from(*room))).first().copied()
            } else {
                Some(RoomId(*room))
            }
        })
        .collect()
}

impl SearchRooms {
    pub fn new(rooms: Vec<RoomId>, sees: String, enter: String) -> Self {
        SearchRooms {
            rooms,
            sees,
            enter,
            index: 0,
            walked: false,
            entered: false,
        }
    }

    fn shown(&self, seen: &Seen<'_>) -> bool {
        seen.state
            .room
            .objects
            .iter()
            .any(|thing| thing.text.contains(&self.sees) || thing.noun == self.sees)
    }
}

impl Solver for SearchRooms {
    fn next(&mut self, seen: &Seen<'_>) -> Next {
        if self.entered {
            return Next::Done;
        }
        // Bounded by the list: each turn walks to a room or passes one.
        while let Some(room) = self.rooms.get(self.index).copied() {
            if !self.walked && seen.here != Some(room) {
                self.walked = true;
                return Next::WalkTo(room);
            }
            if self.shown(seen) {
                self.entered = true;
                return Next::Go(self.enter.clone());
            }
            self.index += 1;
            self.walked = false;
        }
        Next::Failed
    }
}

#[cfg(test)]
mod tests {
    use cena_map::Room;

    use super::super::testing::Scene;
    use super::*;

    fn portal() -> SearchRooms {
        SearchRooms::new(
            vec![RoomId(1), RoomId(2), RoomId(3)],
            "rippling ethereal green portal".into(),
            "go ethereal portal".into(),
        )
    }

    /// A room showing a thing by its full name.
    fn showing(here: u32, text: &str, noun: &str) -> Scene {
        let mut scene = Scene::at(here, 9).showing(noun);
        if let Some(thing) = scene.state.room.objects.first_mut() {
            thing.text = text.into();
        }
        scene
    }

    #[test]
    fn it_walks_the_list_until_a_room_shows_it_and_goes_in() {
        let mut search = portal();
        // Already in the first room: upstream's `unless Room.current.id == r`.
        assert_eq!(Scene::at(1, 9).ask(&mut search), Next::WalkTo(RoomId(2)));
        assert_eq!(Scene::at(2, 9).ask(&mut search), Next::WalkTo(RoomId(3)));
        assert_eq!(
            showing(3, "a rippling ethereal green portal", "portal").ask(&mut search),
            Next::Go("go ethereal portal".into())
        );
        assert_eq!(Scene::at(7, 9).ask(&mut search), Next::Done);
    }

    #[test]
    fn a_portal_of_another_colour_is_not_it() {
        let mut search = portal();
        assert_eq!(
            showing(1, "a shimmering blue portal", "portal").ask(&mut search),
            Next::WalkTo(RoomId(2))
        );
    }

    #[test]
    fn a_doorframe_is_told_by_its_noun() {
        let mut search = SearchRooms::new(vec![RoomId(1)], "doorframe".into(), "go door".into());
        assert_eq!(
            showing(1, "a warped frame", "doorframe").ask(&mut search),
            Next::Go("go door".into())
        );
    }

    #[test]
    fn a_walk_that_failed_looks_where_it_stands_and_goes_on() {
        let mut search = portal();
        assert_eq!(Scene::at(5, 9).ask(&mut search), Next::WalkTo(RoomId(1)));
        let mut stuck = Scene::at(5, 9);
        stuck.failed = true;
        // Not walked to twice: the next room is tried instead.
        assert_eq!(stuck.ask(&mut search), Next::WalkTo(RoomId(2)));
    }

    #[test]
    fn a_list_with_nothing_in_it_ends_and_sends_no_move() {
        let mut search = portal();
        assert_eq!(Scene::at(1, 9).ask(&mut search), Next::WalkTo(RoomId(2)));
        assert_eq!(Scene::at(2, 9).ask(&mut search), Next::WalkTo(RoomId(3)));
        assert_eq!(Scene::at(3, 9).ask(&mut search), Next::Failed);
    }

    #[test]
    fn the_games_numbers_are_resolved_by_the_map_and_strangers_skipped() {
        let rooms: Vec<Room> =
            serde_json::from_str(r#"[{"id":10,"uid":[474204]},{"id":11,"uid":[474205]}]"#)
                .expect("rooms");
        let map = Map::from_rooms(rooms).expect("a map");
        assert_eq!(
            resolve(&[474_205, 5, 474_204], true, &map),
            vec![RoomId(11), RoomId(10)]
        );
        assert_eq!(resolve(&[474_205], false, &map), vec![RoomId(474_205)]);
    }
}
