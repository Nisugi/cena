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

use crate::status::StatusInfo;
use cena_protocol::Frame;
use cena_protocol::runs::Runs;
use std::time::Instant;

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
///
/// # `PartialEq` is hand-written, and deliberately ignores one field
///
/// [`Self::game_time_received`] is a **local `Instant`**, so two replays of
/// one recording produce two different values -- microseconds apart, but
/// different. Deriving `PartialEq` made `crates/cena-session/tests/
/// replay_determinism.rs` go red on exactly that (`Instant { t: 311689.3884761s }`
/// against `311689.388152s`), which is criterion 7's ratchet doing its job:
/// `Recorder`'s own documentation says "a wall clock is the single easiest way
/// to make a replay non-deterministic", and this is that mistake in a
/// different file.
///
/// It is excluded rather than removed because it is **an observation about
/// when state arrived, not part of the state**. The server's clock
/// ([`Self::game_time`]) IS state and is compared; the local instant it
/// reached us is scaffolding for extrapolating it, and two sessions that saw
/// the same frames are in the same state regardless of when.
#[derive(Clone, Debug, Default, Eq)]
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
    /// What the game says is true of the character right now.
    ///
    /// `plan/17` §2, ported from Vellum. Read it through the typed accessors
    /// ([`StatusInfo::stunned`] and friends) or by id; an id the game has
    /// never reported reads `false`, and [`StatusInfo::is_known`] is what
    /// separates that from a reported `false`.
    pub status: StatusInfo,
    /// The server's clock, from the last `<prompt time=>`.
    ///
    /// Private, because a raw reading is a trap: it is only correct at the
    /// instant the prompt arrived. [`GameState::game_time_now`] extrapolates
    /// it and is what callers want.
    game_time: Option<u32>,
    /// When [`Self::game_time`] arrived, on the **monotonic** clock.
    ///
    /// `Instant`, not `SystemTime`: it must not move when NTP steps the wall
    /// clock or when DST changes, because the only thing it is used for is
    /// measuring an interval.
    game_time_received: Option<Instant>,
    /// Tags the parser did not model, in arrival order.
    ///
    /// Criterion 8: these **survive to display**. Rule 2.2 requires an
    /// unmodelled tag to reach the user, so it is kept here rather than
    /// counted and dropped.
    pub unknown_tags: Vec<UnknownTag>,
}

impl PartialEq for GameState {
    fn eq(&self, other: &Self) -> bool {
        // Every field EXCEPT `game_time_received`. Listed explicitly rather
        // than derived-minus-one so that adding a field is a compile error
        // here, and whoever adds it has to decide which side it belongs on.
        let Self {
            room,
            prompt,
            left_hand,
            right_hand,
            roundtime_ends,
            vitals,
            status,
            game_time,
            game_time_received: _,
            unknown_tags,
        } = self;
        room == &other.room
            && prompt == &other.prompt
            && left_hand == &other.left_hand
            && right_hand == &other.right_hand
            && roundtime_ends == &other.roundtime_ends
            && vitals == &other.vitals
            && status == &other.status
            && game_time == &other.game_time
            && unknown_tags == &other.unknown_tags
    }
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
    /// The server's clock **now**, extrapolated.
    ///
    /// The last prompt's timestamp plus how long ago it arrived on the local
    /// monotonic clock. Ported from Vellum
    /// (`core/state.rs`, `game_time_now`), and the single most important
    /// borrowed idea in this file.
    ///
    /// # Why extrapolate rather than read a clock
    ///
    /// **Timers must keep flowing between prompts.** A prompt is sent when
    /// something happens; nothing is sent when a roundtime merely ends
    /// (`plan/15` §2a.1, the author). MEASURED (§2a.4a): a session with a
    /// behavior firing once a second produced 58 prompts, and an idle one
    /// would produce none -- so anything that waits for a prompt to learn a
    /// roundtime ended waits forever in a quiet room.
    ///
    /// **Both sides stay in server time**, so clock skew cancels instead of
    /// needing correction. This is what made an elaborate skew-calibration
    /// design unnecessary: there is no comparison between two clocks anywhere
    /// in it, only an interval measured on one.
    ///
    /// Returns `None` until the first prompt arrives -- `plan/12` §5.2's
    /// `Unknown`, not a fabricated zero.
    #[must_use]
    pub fn game_time_now(&self) -> Option<u32> {
        let base = self.game_time?;
        let elapsed = self.game_time_received.map_or(0, |at| {
            u32::try_from(at.elapsed().as_secs()).unwrap_or(u32::MAX)
        });
        Some(base.saturating_add(elapsed))
    }

    /// Whether the character is in roundtime.
    ///
    /// Compares the extrapolated server clock against
    /// [`Self::roundtime_ends`], both in server epoch seconds.
    ///
    /// # This test is EXACT, not approximate
    ///
    /// MEASURED 2026-09-18 (`plan/15` §2a.4a): across three roundtimes the
    /// first plain `>` prompt landed **precisely** on `<roundTime value=>`, so
    /// `value` is the end instant rather than "somewhere inside that second".
    ///
    /// # `None` means unknown, and unknown is not `false`
    ///
    /// Returns `None` when either side is unobserved -- no roundtime seen, or
    /// no prompt yet to calibrate against. A caller that treats `None` as "not
    /// in roundtime" is making exactly the assumption `plan/12` §5.2 forbids,
    /// and the type makes them write that decision down.
    #[must_use]
    pub fn in_roundtime(&self) -> Option<bool> {
        Some(self.game_time_now()? < self.roundtime_ends?)
    }

    /// How many seconds of roundtime remain, or `None` if unknown.
    ///
    /// Saturates at zero rather than going negative: a roundtime that has
    /// passed has no remainder.
    #[must_use]
    pub fn roundtime_remaining(&self) -> Option<u32> {
        Some(self.roundtime_ends?.saturating_sub(self.game_time_now()?))
    }

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
            // `compDef`/`component` ONLY. The room arrives in two shapes and
            // they mean different things (author, 2026-09-18, from live
            // traffic):
            //
            //   * `<compDef id='room desc'>` feeds the ROOM WINDOW and is
            //     truth for WHERE THE CHARACTER IS. It is also truth for the
            //     creatures, objects and players in the room, each in its own
            //     `compDef`, which is why the window feed stays structured.
            //   * inline text styled `<style id="roomDesc"/>` feeds the STORY
            //     WINDOW and is only WHAT THE CHARACTER SAW. It is flattened:
            //     the live capture shows `room objs` appended into the prose
            //     rather than kept separate.
            //
            // **Abilities that look into another room emit the story form
            // WITHOUT the window form.** That asymmetry is not noise -- it is
            // exactly how a client knows the character did NOT move. Folding
            // the styled text here would turn every scry into a phantom
            // relocation, and the bug would only appear for players who use
            // those abilities.
            //
            // So `look` does not update `room.description`, by design. A
            // consumer that wants the looked-at prose reads the published
            // `Frame::Text`; `GameState` tracks location, not narration.
            Frame::Component { id, body } if id == "room desc" => {
                self.room.description = Some(body.clone());
            }
            Frame::Compass { directions } => {
                self.room.exits.clone_from(directions);
            }
            Frame::Prompt { time, text } => {
                self.prompt = Some(text.clone());
                // The server's clock, and when it reached us. Together these
                // are what make `game_time_now()` keep counting between
                // prompts -- which matters because a prompt is only sent when
                // something happens, so an idle client gets none at all
                // (`plan/15` §2a.1, MEASURED §2a.4a).
                if let Ok(t) = time.parse::<u32>() {
                    self.game_time = Some(t);
                    self.game_time_received = Some(Instant::now());
                }
                return true;
            }
            Frame::StatusIndicator { id, active } => {
                self.status.set(id, *active);
            }
            Frame::LeftHand { item, .. } => self.left_hand = Some(item.clone()),
            Frame::RightHand { item, .. } => self.right_hand = Some(item.clone()),
            Frame::RoundTime { value } => self.roundtime_ends = Some(*value),
            Frame::ProgressBar(bar) => {
                // ONLY the player's own bars. `plan/12` §7.1 scopes this to
                // the character's vitals, and `<progressBar>` is also how the
                // game ships OTHER creatures' health: an appraisal opens
                // `<dialogData id="injuries-{existID}">` carrying its own
                // `health2` bar (wiki `:243`). Keying on `bar.id` alone let a
                // target's health overwrite the player's -- the model would
                // report the character at 12% because something they appraised
                // was.
                //
                // The parser already distinguishes them (`bar.dialog` carries
                // the enclosing `dialogData` id); this is the model choosing
                // to keep that. `minivitals` and `injuries` are the player;
                // anything suffixed `injuries-<id>` is a third party and is
                // published to observers without entering the character's
                // state.
                let is_own = bar
                    .dialog
                    .as_deref()
                    .is_none_or(|d| !d.starts_with("injuries-"));
                if is_own {
                    self.vitals.insert(bar.id.clone(), bar.percent);
                }
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
