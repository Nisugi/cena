//! Where each character is on the map, for its minimap (`plan/53` §7 step
//! 4): `room_of`, the same ladder travel climbs, with memory guarded as §6
//! item 4 has it. Where it just was settles a room that reads like others,
//! but only when the move count rose by exactly one since it last looked:
//! the same count is the same room, and a count that jumped means moves this
//! never saw, so where it was means nothing.

use std::sync::{Arc, Mutex, PoisonError};

use cena_behavior::travel::{Map, RoomId, Whence, room_of};
use cena_session::GameState;
use cena_ui::MinimapView;

use super::{Atlas, Waiting};

/// A character's minimap source: a follower of its own over the one map
/// and atlas every character shares.
pub(crate) fn minimap(map: Arc<Map>, atlas: Arc<Atlas>) -> cena_gui::Minimap {
    let last: Mutex<Option<(u32, RoomId)>> = Mutex::new(None);
    Arc::new(move |state: &GameState| {
        let mut last = last.lock().unwrap_or_else(PoisonError::into_inner);
        let room = room_of(&map, state, whence(*last, state.arrivals));
        *last = room.map(|room| (state.arrivals, room));
        match room {
            None => MinimapView::Waiting("Where you are is not known yet.".to_owned()),
            Some(room) => match atlas.scene_of(room.0) {
                Ok(scene) => MinimapView::Here {
                    scene,
                    room: room.0,
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
