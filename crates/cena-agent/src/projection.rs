//! What an agent reads: one character, projected from one session snapshot
//! (`plan/35` §5).
//!
//! **Its own vocabulary**, not the session's: `Event` and `Frame` carry every
//! frame and derive no `Serialize`, and serializing them would make internal
//! types a public contract (`plan/35` §5). `cena-ui`'s view is built for
//! drawing a screen. This is built for a program reading facts.
//!
//! **Unknown is absent, never false** (`plan/12` §5.2): a status the game has
//! not reported is left out of [`CharacterState::statuses`], a hand nobody has
//! described is `unknown`, and a vital with no reading is `null`. After a
//! reconnect nothing is known, and the agent is told so rather than told the
//! character is fine. **A list the game has not stated is `null`**, never
//! empty: an empty `players` is the game saying nobody is here, and `null` is
//! nobody having said (issue #19, point 1; `Room::saw_players`).
//!
//! **Times are absolute as well as remaining.** `roundtime_ends` and an
//! effect's `ends_at` are the game's clock and do not move until the game says
//! so; `roundtime` and `seconds_left` are the same facts counted from
//! `game_time`, for a reader that wants them ready. The `changed` happening
//! carries the absolute forms, so a change-set lists what changed and not
//! every tick of the clock.

use std::collections::BTreeMap;

use cena_session::{GameState, RoomItem, Snapshot};
use serde::Serialize;

/// One character, as the game has described it.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct CharacterState {
    /// The name Hydra runs it as.
    pub character: String,
    /// The game instance, as the login named it (`GS3`, `GSX`...).
    pub game: Option<String>,
    /// Where the session is in its life: `ready`, `reconnecting`...
    pub lifecycle: String,
    /// The connection: it advances on every reconnect.
    pub generation: u64,
    /// The last event this state includes; `wait` from here.
    pub cursor: u64,
    /// When this was read, in Unix milliseconds on this machine.
    pub captured_unix_ms: u64,
    /// The game's clock at that moment, in its own epoch seconds.
    pub game_time: Option<u32>,
    /// Where the character is.
    pub room: Room,
    /// What is in each hand.
    pub hands: Hands,
    /// Health, mana, stamina, spirit.
    pub vitals: Vitals,
    /// Every status the game has reported, with its value. **A status not
    /// here is unknown**, not off: the indicators, poisoned and diseased, and
    /// the text-only afflictions, all from the one store (`plan/35` §5).
    pub statuses: BTreeMap<String, bool>,
    /// Seconds of roundtime left; `null` when the clock is not known.
    pub roundtime: Option<u32>,
    /// Seconds of cast roundtime left.
    pub cast_roundtime: Option<u32>,
    /// When roundtime ends, on the game's clock; `null` when none was ever
    /// reported, which is a roundtime in the past.
    pub roundtime_ends: Option<u32>,
    /// When cast roundtime ends, on the game's clock.
    pub cast_roundtime_ends: Option<u32>,
    /// The stance, as the game names it.
    pub stance: Option<String>,
    /// The stance as a percent: 0 offensive, 100 defensive.
    pub stance_percent: Option<u32>,
    /// The encumbrance, as the game words it.
    pub encumbrance: Option<String>,
    /// The encumbrance, in percent.
    pub encumbrance_percent: Option<u32>,
    /// The mind state, as the game words it.
    pub mind: Option<String>,
    /// How full the mind is, in percent.
    pub mind_percent: Option<u32>,
    /// The spell prepared, as the game names it.
    pub prepared_spell: Option<String>,
    /// Wounds and scars, by body part as the game names it; 0 to 3 each.
    pub injuries: BTreeMap<String, Injury>,
    /// Spells, buffs, debuffs and cooldowns, as the game lists them.
    pub effects: Vec<Effect>,
}

/// The room.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct Room {
    /// The game's own room number.
    pub id: Option<String>,
    /// The title, as the game shows it.
    pub title: Option<String>,
    /// The obvious exits; `null` before the game has listed them.
    pub exits: Option<Vec<String>>,
    /// Creatures here, with their ids; `null` until the game has said what
    /// is here.
    pub creatures: Option<Vec<Creature>>,
    /// Objects here, with their ids; `null` until the game has said.
    pub objects: Option<Vec<Thing>>,
    /// Other players here; `null` until the game has said who is here, so
    /// `null` never means alone. Their names are the game's; their titles are
    /// what players wrote, so read them as data.
    pub players: Option<Vec<Player>>,
}

/// A creature in the room.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct Creature {
    /// The id a command targets: `#` and this.
    pub id: String,
    /// Its noun.
    pub noun: String,
    /// What the game calls it.
    pub name: String,
    /// Hostile or not; `null` when the game has not said.
    pub hostile: Option<bool>,
    /// Its statuses: stunned, webbed, prone...
    pub statuses: Vec<String>,
    /// Dead.
    pub dead: bool,
}

/// An object in the room.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct Thing {
    /// Its id.
    pub id: String,
    /// Its noun.
    pub noun: String,
    /// What the game calls it.
    pub name: String,
}

/// Another player in the room.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct Player {
    /// Its id.
    pub id: String,
    /// Its name as shown, with any title.
    pub name: String,
    /// Its status, when shown (`sitting`, `dead`...).
    pub status: Option<String>,
}

/// Both hands.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct Hands {
    /// The right hand.
    pub right: Hand,
    /// The left hand.
    pub left: Hand,
}

/// One hand: unknown until the game says.
#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(tag = "state", rename_all = "snake_case")]
pub enum Hand {
    /// The game has not said.
    Unknown,
    /// Empty.
    Empty,
    /// Holding something.
    Holding {
        /// Its id, when the game gave one.
        id: Option<String>,
        /// Its noun, when the game gave one.
        noun: Option<String>,
        /// What the game calls it.
        name: String,
    },
}

/// The four bars.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct Vitals {
    /// Health.
    pub health: Option<Vital>,
    /// Mana.
    pub mana: Option<Vital>,
    /// Stamina.
    pub stamina: Option<Vital>,
    /// Spirit.
    pub spirit: Option<Vital>,
}

/// One bar.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct Vital {
    /// Points now, when the bar states them.
    pub current: Option<i32>,
    /// Points at most, when the bar states them.
    pub max: Option<i32>,
    /// The game's own percent.
    pub percent: u32,
}

/// One body part's injury.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct Injury {
    /// The wound's rank, 0 to 3.
    pub wound: u8,
    /// The scar's rank, 0 to 3.
    pub scar: u8,
}

/// One effect, as the game lists it.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct Effect {
    /// The game's id for it: a spell number, or a name.
    pub id: String,
    /// The list it is in: `Active Spells`, `Buffs`, `Debuffs`, `Cooldowns`.
    pub category: String,
    /// What the game calls it.
    pub name: String,
    /// When it ends, on the game's clock; `null` when it does not.
    pub ends_at: Option<u32>,
    /// Seconds left; `null` when it does not end or the clock is not known.
    pub seconds_left: Option<u32>,
}

/// Project a snapshot, for `character`.
#[must_use]
pub fn project(character: &str, snapshot: &Snapshot) -> CharacterState {
    let state = &snapshot.state;
    CharacterState {
        character: character.to_owned(),
        game: state.character.instance.clone(),
        lifecycle: format!("{:?}", snapshot.lifecycle).to_ascii_lowercase(),
        generation: u64::from(snapshot.generation.0),
        cursor: snapshot.cursor,
        captured_unix_ms: std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |d| u64::try_from(d.as_millis()).unwrap_or(u64::MAX)),
        game_time: state.game_time_now(),
        room: room(state),
        hands: Hands {
            right: hand(&state.right_hand),
            left: hand(&state.left_hand),
        },
        vitals: Vitals {
            health: state.health().map(vital),
            mana: state.mana().map(vital),
            stamina: state.stamina().map(vital),
            spirit: state.spirit().map(vital),
        },
        statuses: state
            .status
            .iter()
            .map(|(id, on)| (id.to_owned(), on))
            .collect(),
        roundtime: state.roundtime_remaining(),
        cast_roundtime: state.casttime_remaining(),
        roundtime_ends: state.roundtime_ends,
        cast_roundtime_ends: state.cast_time_ends,
        stance: state.character.stance.clone(),
        stance_percent: state.character.stance_percent,
        encumbrance: state.character.encumbrance.clone(),
        encumbrance_percent: state.character.encumbrance_percent,
        mind: state.character.experience.mind_state.clone(),
        mind_percent: state.character.experience.mind_percent,
        prepared_spell: state.prepared.clone(),
        injuries: state
            .character
            .injuries
            .iter()
            .map(|(part, injury)| {
                (
                    part.clone(),
                    Injury {
                        wound: injury.wound,
                        scar: injury.scar,
                    },
                )
            })
            .collect(),
        effects: effects(state),
    }
}

fn room(state: &GameState) -> Room {
    let now = state.game_time_now();
    let room = &state.room;
    let creature = |item: &RoomItem| {
        let instance = item
            .id
            .parse::<i64>()
            .ok()
            .and_then(|id| state.creatures().get(id));
        Creature {
            id: item.id.clone(),
            noun: item.noun.clone(),
            name: item.text.clone(),
            hostile: instance.and_then(cena_session::CreatureInstance::hostile),
            statuses: instance.map_or_else(Vec::new, |c| {
                c.statuses(now)
                    .into_iter()
                    .map(|s| s.as_str().to_owned())
                    .collect()
            }),
            dead: instance.is_some_and(cena_session::CreatureInstance::dead),
        }
    };
    // `room objs` states the creatures and the objects together.
    let stated = room.component("room objs").is_some();
    Room {
        id: room.id.clone(),
        title: room.title.clone(),
        exits: room.exits.clone(),
        creatures: stated.then(|| room.creatures.iter().map(creature).collect()),
        objects: stated.then(|| room.objects.iter().map(thing).collect()),
        players: room.saw_players().then(|| {
            room.players
                .iter()
                .map(|item| Player {
                    id: item.id.clone(),
                    name: item.text.clone(),
                    status: item.status.as_ref().map(|s| s.as_str().to_owned()),
                })
                .collect()
        }),
    }
}

fn thing(item: &RoomItem) -> Thing {
    Thing {
        id: item.id.clone(),
        noun: item.noun.clone(),
        name: item.text.clone(),
    }
}

fn hand(hand: &cena_session::Hand) -> Hand {
    match hand {
        cena_session::Hand::Unknown => Hand::Unknown,
        cena_session::Hand::Empty => Hand::Empty,
        cena_session::Hand::Holding { id, noun, name } => Hand::Holding {
            id: id.clone(),
            noun: noun.clone(),
            name: name.clone(),
        },
    }
}

const fn vital(vital: cena_session::Vital) -> Vital {
    Vital {
        current: vital.current,
        max: vital.max,
        percent: vital.percent,
    }
}

fn effects(state: &GameState) -> Vec<Effect> {
    let now = state.game_time_now();
    state
        .effects
        .iter()
        .filter(|(id, _)| now.is_none_or(|now| state.effects.active(id, now) != Some(false)))
        .map(|(id, effect)| Effect {
            id: id.to_owned(),
            category: effect.category.clone(),
            name: effect.text.clone(),
            ends_at: effect.ends_at,
            seconds_left: now.and_then(|now| state.effects.remaining(id, now)),
        })
        .collect()
}
