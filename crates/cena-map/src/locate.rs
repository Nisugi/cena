//! Which room is this? (`plan/21` §5 step 4.)
//!
//! A pure question over a loaded [`Map`]: given what the game showed and where
//! the character was, name the room -- or say plainly that it cannot be named.
//!
//! # The ladder
//!
//! 1. **The game's number.** One room has it: done. The map's text is never
//!    checked against a number that names one room, because the text goes
//!    stale and the number does not.
//! 2. **Several rooms have it** (a room that changes form keeps its number):
//!    told apart by text. MEASURED, `research/mapdb-inventory/identify.py`:
//!    42 numbers are shared, and text separates all 42.
//! 3. **The map has never seen the number.** The game numbers every room; the
//!    map lacks the number for 7,876 of them. So the room is one of those, or
//!    one marked as having more numbers than are recorded -- never a room
//!    whose recorded numbers are complete. Text over that pool: 7,543 of the
//!    7,876 are unique by title, description and paths.
//! 4. **Still several:** where the character was. Standing still, it is the
//!    room it was; having moved, it is the one candidate the room just left
//!    has an exit to (91 of the remaining 333).
//! 5. Otherwise [`Located::Ambiguous`], with the candidates. The caller may
//!    know more -- a `location` reading narrows 122 of what is left.
//!
//! # Unambiguous or nothing
//!
//! Upstream's matcher takes the first room that fits. This one never guesses:
//! a wrong room sends a walk the wrong way, and "I do not know" is a state a
//! caller can act on. And a filter that matches *nothing* is ignored rather
//! than obeyed -- a description that fits no candidate means the map's text
//! is stale, not that the character is nowhere.
//!
//! Not built: asking the game (`location`, `peer`). That is an action, and
//! this crate only answers questions.

use crate::map::Map;
use crate::room::{Room, RoomId, Uid};

/// Marks a room whose recorded numbers are known to be incomplete.
const MULTI_UID: &str = "map:multi-uid";
/// Marks a room whose exits line changes between visits.
const RANDOM_PATHS: &str = "random-paths";
const FOG: &str = "obscured by a thick fog";

/// What the game showed of the room. Every part is optional because every
/// part can be missing: a disabled room window, fog, a login still arriving.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Sighting<'a> {
    /// The game's room number. `None` when it sent none, or sent zero.
    pub uid: Option<Uid>,
    /// As the map spells it: see [`title_from_subtitle`].
    pub title: Option<&'a str>,
    pub description: Option<&'a str>,
    /// The whole exits line, `Obvious paths: north, east`.
    pub paths: Option<&'a str>,
    /// What the `location` verb said, if anything has asked.
    pub location: Option<&'a str>,
}

/// Where the character was, as far as the caller knows.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Origin {
    /// Nothing is known: a login, or every earlier sighting was ambiguous.
    Nowhere,
    /// The game re-described the room the character is already in.
    Still(RoomId),
    /// The character moved, and this is the room it left.
    Left(RoomId),
}

/// What settled it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum By {
    /// The game's number names one room.
    Uid,
    /// Text narrowed the candidates to one.
    Text,
    /// Several fit, and the character had not moved.
    Stayed,
    /// Several fit, and the room just left has an exit to only one.
    CameFrom,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Located {
    Here {
        room: RoomId,
        by: By,
    },
    /// More than one room fits and nothing seen tells them apart. In id order.
    Ambiguous(Vec<RoomId>),
    /// No room fits, or too little was seen to look.
    Unknown,
}

/// The room title as the map spells it, from a `streamWindow` subtitle.
///
/// The wire says ` - Rawknuckle's, Watering Hole`, with ` - 7503251` after it
/// when the player has room numbers shown; the map says
/// `[Rawknuckle's, Watering Hole]`.
#[must_use]
pub fn title_from_subtitle(subtitle: &str) -> String {
    let name = subtitle.strip_prefix(" - ").unwrap_or(subtitle);
    let name = match name.rsplit_once(" - ") {
        Some((before, number))
            if !number.is_empty() && number.bytes().all(|b| b.is_ascii_digit()) =>
        {
            before
        }
        _ => name,
    };
    format!("[{name}]")
}

impl Map {
    /// Name the room a sighting describes. See the module docs for the ladder.
    #[must_use]
    pub fn locate(&self, sighting: &Sighting<'_>, origin: Origin) -> Located {
        let numbered = sighting.uid.map_or(&[][..], |uid| self.ids_for_uid(uid));
        if let [room] = numbered {
            return Located::Here {
                room: *room,
                by: By::Uid,
            };
        }

        let mut pool: Vec<&Room> = if numbered.is_empty() {
            // Without a title there is nothing to search the whole map by.
            let Some(title) = sighting.title else {
                return Located::Unknown;
            };
            let unseen_number = sighting.uid.is_some();
            self.rooms()
                .iter()
                .filter(|room| !unseen_number || may_have_unrecorded_number(room))
                .filter(|room| room.title.iter().any(|known| known == title))
                .collect()
        } else {
            let rooms = numbered.iter().filter_map(|id| self.room(*id));
            let mut pool: Vec<&Room> = rooms.collect();
            if let Some(title) = sighting.title {
                narrow(&mut pool, |room| room.title.iter().any(|t| t == title));
            }
            pool
        };

        if let Some(description) = sighting.description.map(str::trim) {
            narrow(&mut pool, |room| {
                room.description.iter().any(|d| d.trim() == description)
            });
        }
        if let Some(paths) = sighting.paths.map(str::trim)
            && !paths.ends_with(FOG)
        {
            narrow(&mut pool, |room| {
                room.tags.iter().any(|tag| tag == RANDOM_PATHS)
                    || room.paths.iter().any(|p| p.trim() == paths)
            });
        }
        if let Some(location) = sighting.location {
            narrow(&mut pool, |room| room.location.as_deref() == Some(location));
        }

        match (pool.as_slice(), origin) {
            ([], _) => Located::Unknown,
            ([room], _) => Located::Here {
                room: room.id,
                by: if numbered.is_empty() {
                    By::Text
                } else {
                    By::Uid
                },
            },
            (_, Origin::Still(was)) if pool.iter().any(|room| room.id == was) => Located::Here {
                room: was,
                by: By::Stayed,
            },
            (_, Origin::Left(was)) => match self.reached_from(was, &pool) {
                Some(room) => Located::Here {
                    room,
                    by: By::CameFrom,
                },
                None => ambiguous(&pool),
            },
            _ => ambiguous(&pool),
        }
    }

    /// The one candidate `from` has an exit to, if there is exactly one.
    ///
    /// Every exit counts, crossable or not: this asks how the map is joined,
    /// not whether a walk could be planned.
    fn reached_from(&self, from: RoomId, pool: &[&Room]) -> Option<RoomId> {
        let exits = &self.room(from)?.exits;
        let mut reached = pool
            .iter()
            .map(|room| room.id)
            .filter(|id| exits.iter().any(|exit| exit.to == *id));
        let first = reached.next()?;
        reached.next().is_none().then_some(first)
    }
}

fn may_have_unrecorded_number(room: &Room) -> bool {
    room.uid.is_empty() || room.meta.iter().any(|meta| meta == MULTI_UID)
}

/// Keep what passes -- unless nothing does, which says the map's text is
/// stale, not that every candidate is wrong.
fn narrow(pool: &mut Vec<&Room>, passes: impl Fn(&Room) -> bool) {
    if pool.iter().any(|room| passes(room)) {
        pool.retain(|room| passes(room));
    }
}

fn ambiguous(pool: &[&Room]) -> Located {
    Located::Ambiguous(pool.iter().map(|room| room.id).collect())
}
