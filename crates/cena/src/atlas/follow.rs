//! Where each character is on the map, for its minimap (`plan/53` §7 step
//! 4): `room_of`, the same ladder travel climbs, with memory guarded as §6
//! item 4 has it. Where it just was settles a room that reads like others,
//! but only when the move count rose by exactly one since it last looked:
//! the same count is the same room, and a count that jumped means moves this
//! never saw, so where it was means nothing.

use std::sync::{Arc, Mutex, PoisonError};

use cena_agent::scripts::local::WalkerOf;
use cena_behavior::travel::{Map, RoomId, Whence, itinerary, room_of};
use cena_session::GameState;
use cena_ui::MinimapView;

use super::{Atlas, Waiting};

/// A character's minimap source: a follower of its own over the one map
/// and atlas every character shares, routing to the room clicked as
/// travel would walk it for this character (`walker`).
pub(crate) fn minimap(map: Arc<Map>, atlas: Arc<Atlas>, walker: WalkerOf) -> cena_gui::Minimap {
    let last: Mutex<Option<(u32, RoomId)>> = Mutex::new(None);
    let routed: Mutex<Option<Routed>> = Mutex::new(None);
    Arc::new(move |state: &GameState, target: Option<u32>| {
        let mut last = last.lock().unwrap_or_else(PoisonError::into_inner);
        let room = room_of(&map, state, whence(*last, state.arrivals));
        *last = room.map(|room| (state.arrivals, room));
        match room {
            None => MinimapView::Waiting("Where you are is not known yet.".to_owned()),
            Some(room) => match atlas.scene_of(room.0) {
                Ok(scene) => MinimapView::Here {
                    scene,
                    room: room.0,
                    target,
                    route: target.map_or_else(Vec::new, |to| {
                        let mut routed = routed.lock().unwrap_or_else(PoisonError::into_inner);
                        route(&mut routed, (room, RoomId(to)), || {
                            let walker = walker("", state);
                            itinerary(&map, &walker, room, RoomId(to))
                                .map(|legs| legs.iter().map(|leg| leg.to.0).collect())
                        })
                    }),
                },
                Err(Waiting::NoArea) => {
                    MinimapView::Waiting("This place is not on the map's areas.".to_owned())
                }
                Err(Waiting::Laying) => {
                    let (done, of) = atlas.progress();
                    MinimapView::Waiting(format!("Laying out the map: {done} of {of} areas."))
                }
            },
        }
    })
}

/// The last route worked out: from where, to where, and the rooms of it.
/// The walker reads the character's travel file, so a route is worked out
/// again only when either end changes.
struct Routed {
    ends: (RoomId, RoomId),
    rooms: Vec<u32>,
}

/// The rooms from `ends.0` to `ends.1`, both included: the last route's
/// when its ends are these, else `legs`' rooms after the first, kept.
/// Empty when there is no way.
fn route(
    last: &mut Option<Routed>,
    ends: (RoomId, RoomId),
    legs: impl FnOnce() -> Option<Vec<u32>>,
) -> Vec<u32> {
    if let Some(routed) = last.as_ref().filter(|r| r.ends == ends) {
        return routed.rooms.clone();
    }
    let rooms: Vec<u32> = legs()
        .map(|after| std::iter::once(ends.0.0).chain(after).collect())
        .unwrap_or_default();
    *last = Some(Routed {
        ends,
        rooms: rooms.clone(),
    });
    rooms
}

/// What memory may say, given the room last found and the move count then.
fn whence(last: Option<(u32, RoomId)>, arrivals: u32) -> Whence {
    match last {
        Some((then, room)) if then == arrivals => Whence::Still(room),
        Some((then, room)) if then.wrapping_add(1) == arrivals => Whence::Left(room),
        _ => Whence::Nowhere,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The guard of `plan/53` §6 item 4: the same count is the same room,
    /// one more is the room just left, and any other jump is no memory.
    /// A route is worked out once for its ends, and again when either
    /// changes; no way is no rooms.
    #[test]
    fn a_route_is_worked_out_once_for_its_ends() {
        let (a, b, c) = (RoomId(1), RoomId(2), RoomId(3));
        let mut last = None;
        let asked = std::cell::Cell::new(0);
        let legs = |rooms: Option<Vec<u32>>| {
            asked.set(asked.get() + 1);
            rooms
        };
        assert_eq!(
            route(&mut last, (a, c), || legs(Some(vec![2, 3]))),
            [1, 2, 3]
        );
        assert_eq!(route(&mut last, (a, c), || legs(None)), [1, 2, 3]);
        assert_eq!(asked.get(), 1, "the same ends are not asked again");
        assert_eq!(route(&mut last, (b, c), || legs(Some(vec![3]))), [2, 3]);
        assert!(route(&mut last, (b, a), || legs(None)).is_empty());
        assert_eq!(asked.get(), 3);
    }

    #[test]
    fn memory_only_across_one_move() {
        let room = RoomId(228);
        assert_eq!(whence(None, 5), Whence::Nowhere);
        assert_eq!(whence(Some((5, room)), 5), Whence::Still(room));
        assert_eq!(whence(Some((5, room)), 6), Whence::Left(room));
        assert_eq!(whence(Some((5, room)), 7), Whence::Nowhere);
        assert_eq!(whence(Some((5, room)), 4), Whence::Nowhere);
    }
}
