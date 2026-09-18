//! [`GameState`]: what the session knows, built **only** from typed frames.
//!
//! `plan/12` §7.1 scopes this to "room, hands, roundtime, vitals" and puts
//! "stats, skills, PSMs, spells, inventory" in the Out column. That is what is
//! here and no more.
//!
//! # Criterion 2, and what "never raw text" actually forbids
//!
//! Criterion 2 (`plan/12:457`) is "renders a room description and prompt from
//! **typed frames**, never raw text". The failure it rules out is a consumer
//! that scans display text for `"Obvious exits:"` and splits on commas. So the
//! room's exits come from [`Frame::Compass`]'s `directions`, and the room's id
//! from [`Frame::RoomId`] -- both of which the wire states outright and
//! neither of which is recoverable from prose without guessing.
//!
//! The room *description* is genuinely text: it is prose the game wrote, and
//! there is nothing else it could be. What makes it typed rather than raw is
//! that it arrives as a [`Frame::Component`] with `id = "room desc"` and a
//! parsed [`Runs`](cena_protocol::runs::Runs) body -- the component tells the
//! consumer what the prose *is*, and the body has already had its markup
//! resolved. Vellum stores the inner XML string verbatim here
//! (`reference/VellumFE/src/parser.rs:803-832`); `Runs` is the fix, and it is
//! already in place in `cena-protocol`.
//!
//! # Criterion 8 lives here too
//!
//! "Unknown tags survive to display (`Frame::UnknownTag`) rather than
//! panicking" (`plan/12:466`). An unknown tag is **recorded into the state**,
//! not merely not-crashed-on: [`GameState::unknown_tags`] is what a display
//! reads, so a test can assert the tag reached something a user would see
//! rather than asserting only that nothing blew up. Rule 2.2
//! (`plan/05:276-283`) requires the tag to reach the user *as text and a log*,
//! and "it did not panic" is neither.

use cena_protocol::Frame;
use cena_protocol::runs::Runs;

/// A vitals gauge, as a percentage.
///
/// `BTreeMap`, not `HashMap`, and that is load-bearing for criterion 7: a
/// `HashMap`'s iteration order varies run to run, so a replay that asserted
/// over one would be non-deterministic by construction. The arch tests already
/// model this preference (`crates/cena-arch-tests/tests/architecture.rs:28`).
pub type Vitals = std::collections::BTreeMap<String, u32>;

/// The room the character is in, as the wire stated it.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Room {
    /// `<nav rm=>`. The game's own id, not a guess from the title.
    pub id: Option<String>,
    /// `<component id='room desc'>`, body parsed.
    pub description: Option<Runs>,
    /// `<compass><dir value=>`. Direction tokens, not prose.
    pub exits: Vec<String>,
}

/// What the session knows. `plan/12` §7.1's In column, exactly.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct GameState {
    /// The current room.
    pub room: Room,
    /// The last `<prompt>` text, e.g. `">"`.
    pub prompt: Option<String>,
    /// `<left>` hand contents.
    pub left_hand: Option<String>,
    /// `<right>` hand contents.
    pub right_hand: Option<String>,
    /// `<roundTime value=>`: the absolute epoch second the roundtime ends.
    ///
    /// **`Option`, not `0`.** `plan/12` §5.2: "`Unknown` is a first-class
    /// value, not a default. `world.roundtime` after reconnect is `Unknown`,
    /// never `0`." `0` would be a real epoch second and a behavior could not
    /// tell it from "never observed".
    pub roundtime_ends: Option<u32>,
    /// Gauge id to percentage, e.g. `health` -> 97.
    pub vitals: Vitals,
    /// Tags the parser did not model, in arrival order.
    ///
    /// Criterion 8: these **survive to display**. Rule 2.2 requires an
    /// unmodelled tag to reach the user, so it is kept here rather than
    /// counted and dropped.
    pub unknown_tags: Vec<UnknownTag>,
}

/// A tag `cena-protocol` has no variant for, kept for display and for the log.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct UnknownTag {
    /// The element name.
    pub name: String,
    /// The bytes the game actually sent. The raw form IS the diagnostic: a
    /// reader has to see what arrived, not a summary of it.
    pub raw: String,
}

impl GameState {
    /// Fold one frame into the state.
    ///
    /// Returns `true` if the frame closed a round-trip window, which is the
    /// [`Frame::Prompt`] terminator (`plan/12` §4.4). The caller uses that to
    /// resolve a waiter; folding and window-closing are the same walk over the
    /// frame, so splitting them would mean matching twice.
    pub fn apply(&mut self, frame: &Frame) -> bool {
        match frame {
            Frame::RoomId { id } => {
                // A new room invalidates the old description and exits: they
                // describe somewhere the character no longer is. Keeping them
                // is how a consumer renders the previous room's exits under
                // the new room's name.
                self.room = Room {
                    id: Some(id.clone()),
                    ..Room::default()
                };
            }
            Frame::Component { id, body } if id == "room desc" => {
                self.room.description = Some(body.clone());
            }
            Frame::Compass { directions } => {
                self.room.exits.clone_from(directions);
            }
            Frame::Prompt { text, .. } => {
                self.prompt = Some(text.clone());
                return true;
            }
            Frame::LeftHand { item, .. } => self.left_hand = Some(item.clone()),
            Frame::RightHand { item, .. } => self.right_hand = Some(item.clone()),
            Frame::RoundTime { value } => self.roundtime_ends = Some(*value),
            Frame::ProgressBar(bar) => {
                self.vitals.insert(bar.id.clone(), bar.percent);
            }
            Frame::UnknownTag { name, raw } => self.unknown_tags.push(UnknownTag {
                name: name.clone(),
                raw: raw.clone(),
            }),
            // Every other frame is published to observers without changing
            // state. `plan/12` §7.1 scopes GameState to room/hands/
            // roundtime/vitals; a frame this slice does not model is not
            // dropped -- the actor has already broadcast it -- it simply has
            // nowhere to land here yet.
            _ => {}
        }
        false
    }
}
