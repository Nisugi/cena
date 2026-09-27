//! A runner's local copy of its character (`plan/46` §4.2): what its
//! scripts read without asking -- `XMLData`, `GameObj`, `Room.current` and
//! the `check*` family -- kept current from the `state` events it is sent.
//!
//! **The agent's projection is its core** (`crate::projection`, as `plan/46`
//! says), in Hydra's own names: a runner in another language wants Hydra's
//! words, not Lich's, and the Ruby bridge makes Lich's names of them. What
//! a script needs beyond an agent's read is added here: how many rooms the
//! character has arrived in (what Lich's `move` watches), the room's
//! description and exits as text, the map's room, and the target.
//!
//! **Unknown stays unknown**, as in the projection: a list the game has not
//! stated is `null`, a map room nobody could name is `null`.

use std::sync::Arc;

use cena_map::{Map, Origin, RoomId};
use cena_session::{GameState, Runs, Snapshot};
use serde::Serialize;

use crate::projection::{CharacterState, project};

/// The map, and how a room is named on it: what lets a runner answer
/// `Room.current`. The binary has both; `locate` is travel's own
/// (`cena_behavior::travel::room_of`), so a script and a walk name a room the
/// same way, and never by a guess.
#[derive(Clone)]
pub struct Atlas {
    /// The map.
    pub map: Arc<Map>,
    /// Name the room `state` shows, given where the character was.
    pub locate: Locate,
}

/// How a room is named: the map, what the game shows, where the character
/// was; `None` when it cannot be named without a guess.
pub type Locate = fn(&Map, &GameState, Origin) -> Option<RoomId>;

impl std::fmt::Debug for Atlas {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Atlas")
            .field("rooms", &self.map.len())
            .finish_non_exhaustive()
    }
}

/// One character, as a script reads it.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct Local {
    /// What an agent reads: room, hands, vitals, statuses, roundtimes,
    /// stance, encumbrance, mind, the prepared spell, injuries, effects.
    #[serde(flatten)]
    pub core: CharacterState,
    /// How many times the character has arrived in a room: it changes on
    /// every move, whether or not the rooms read alike. Lich's
    /// `XMLData.room_count`, which `move` watches.
    pub room_count: u32,
    /// The room's description, as text.
    pub room_description: Option<String>,
    /// The room's exits line as the game words it: `Obvious paths: north.`
    pub room_exits_line: Option<String>,
    /// The map's own number for the room, when Hydra has a map and it names
    /// the room without guessing; `null` otherwise.
    pub map_room: Option<u32>,
    /// The creature the character targets, by its id.
    pub target: Option<String>,
    /// The spells the character's spell list names, by number; `null` until
    /// the game has sent the list.
    pub known_spells: Option<Vec<u32>>,
}

/// Where the character is on the map, followed from one copy to the next:
/// the room it was in tells apart rooms that read alike (`cena_map`'s
/// ladder, step 4).
#[derive(Debug, Default)]
pub struct Whereabouts {
    room: Option<RoomId>,
    arrivals: Option<u32>,
}

impl Whereabouts {
    /// The map's room for `state`, and remember it; `None` without a map.
    pub fn locate(&mut self, atlas: Option<&Atlas>, state: &GameState) -> Option<u32> {
        let atlas = atlas?;
        let origin = match (self.room, self.arrivals) {
            (Some(room), Some(arrivals)) if arrivals == state.arrivals => Origin::Still(room),
            (Some(room), _) => Origin::Left(room),
            (None, _) => Origin::Nowhere,
        };
        self.room = (atlas.locate)(&atlas.map, state, origin);
        self.arrivals = Some(state.arrivals);
        self.room.map(|room| room.0)
    }
}

/// The local copy of `snapshot`, for `character`, in `map_room`.
#[must_use]
pub fn local(character: &str, snapshot: &Snapshot, map_room: Option<u32>) -> Local {
    let state = &snapshot.state;
    Local {
        core: project(character, snapshot),
        room_count: state.arrivals,
        room_description: state.room.description.as_ref().map(Runs::plain),
        room_exits_line: state.room.component("room exits").map(Runs::plain),
        map_room,
        target: state.targeting.current().map(|id| id.to_string()),
        known_spells: state.known_spells.is_stated().then(|| {
            state
                .known_spells
                .iter()
                .map(|(number, _)| number)
                .collect()
        }),
    }
}
