//! What one member's hunt reads of its group each tick, and how a
//! character's own game state is read into what the group needs of it
//! (`plan/39` Stage 3).
//!
//! The engine stays pure (`plan/39` §5): the driver reads the board and
//! hands the engine a [`Party`], as it hands it [`Here`] for the map.
//! What needs a clock, the muster's deadlines, the driver works out and
//! hands in as answers.
//!
//! [`Here`]: crate::hunt::said::Here

use cena_session::GameState;

use super::muster::{Muster, Standing};
use super::report::{Hindrance, Leading, Report, Role};

/// The group, as one member's hunt sees it this tick.
#[derive(Clone, Debug)]
pub struct Party {
    /// This member's name.
    pub name: String,
    /// This member's part, read off the game's group ([`super::role`]).
    pub role: Role,
    /// The leader's name: this member's own when it leads.
    pub leader: String,
    /// Follow: what the leader last published; `None` while the leader has
    /// no hunt publishing (bigshot's `tail` waiting for a `head`).
    pub leading: Option<Leading>,
    /// Lead: every follower's report.
    pub followers: Vec<Report>,
    /// Lead: what [`super::muster()`] says of each follower apart from the
    /// leader, by name, with the driver's clock.
    pub musters: Vec<(String, Muster)>,
    /// Every member lost its connection at once since the last rest
    /// ([`super::all_dropped`], latched by the driver).
    pub dropped: bool,
    /// Lead: members of the game's group whose hunts have not put up a
    /// report yet, within `lost_wait`: bigshot's `head` waits for its
    /// followers to register before it hunts (`bigshot.lic:9927-9999`).
    pub awaiting: Vec<String>,
}

/// What keeps the character from acting or moving, off its own state: the
/// one that stops it most, as [`Report::hindrance`] carries it.
#[must_use]
pub fn hindrance(state: &GameState) -> Option<Hindrance> {
    let known = state.status.known();
    let on = |value: Option<bool>| value == Some(true);
    if on(known.dead()) {
        return Some(Hindrance::Dead);
    }
    // `group_member_stunned?`'s words that the model reads
    // (`bigshot.lic:6717-6730`).
    if on(known.stunned()) || on(known.webbed()) || on(known.bound()) || on(known.sleeping()) {
        return Some(Hindrance::Stuck);
    }
    if on(known.sitting()) || on(known.kneeling()) || on(known.prone()) {
        return Some(Hindrance::Down);
    }
    if state.in_roundtime() == Some(true) || state.in_casttime() == Some(true) {
        return Some(Hindrance::Roundtime);
    }
    None
}

/// What a room's player list says keeps a player, from its status words
/// (`appears dead`, `stunned`, `lying down`; `GameObj#status`, which the
/// model keeps as the game's text).
#[must_use]
pub fn hindrance_of(status: &str) -> Option<Hindrance> {
    let has = |word: &str| status.contains(word);
    if has("dead") {
        return Some(Hindrance::Dead);
    }
    if [
        "stunned", "webbed", "bound", "sleeping", "asleep", "frozen", "immobil",
    ]
    .iter()
    .any(|word| has(word))
    {
        return Some(Hindrance::Stuck);
    }
    if ["lying", "sitting", "kneeling", "prone"]
        .iter()
        .any(|word| has(word))
    {
        return Some(Hindrance::Down);
    }
    None
}

/// The leader's own room's reading of `name`, a member Hydra has lost:
/// `None` when the room does not list it (`plan/39` §8a).
#[must_use]
pub fn standing(state: &GameState, name: &str) -> Option<Standing> {
    let player = state
        .room
        .players
        .iter()
        .find(|player| player.noun == name)?;
    Some(Standing {
        grouped: in_group(state, name),
        hindrance: player
            .status
            .as_ref()
            .and_then(|status| hindrance_of(status.as_str())),
    })
}

/// Whether `name` is in this character's game group (a follower's roster
/// holds its leader).
#[must_use]
pub fn in_group(state: &GameState, name: &str) -> bool {
    state
        .group
        .members()
        .iter()
        .any(|member| member.noun == name)
}
