//! [`GameState`]: what the session knows, built **only** from typed frames.
//!
//! `plan/12` §7.1's Out list is **per-milestone, not permanent**: inventory
//! landed in M2 (`plan/18` §2d) and stats/skills land in M3. Out today is the
//! rest of the text-scraped model.
//!
//! # Criterion 2, and what "never raw text" actually forbids
//!
//! Criterion 2 (`plan/12:537`) is "renders a room description and prompt from
//! **typed frames**, never raw text". The failure it rules out is a consumer
//! that scans display text for `"Obvious exits:"` and splits on commas. So the
//! room's exits come from [`Frame::Compass`]'s `directions`, and the room's id
//! from [`Frame::RoomId`] -- both of which the wire states outright and
//! neither of which is recoverable from prose without guessing.
//!
//! The room *description* is genuinely text: it is prose the game wrote, and
//! there is nothing else it could be. What makes it typed rather than raw is
//! that it arrives as a [`Frame::Component`] with `id = "room desc"` and a
//! parsed [`Runs`] body -- the component tells the
//! consumer what the prose *is*, and the body has already had its markup
//! resolved. Vellum stores the inner XML string verbatim here
//! (`reference/VellumFE/src/parser.rs:803-832`); `Runs` is the fix, and it is
//! already in place in `cena-protocol`.
//!
//! # Criterion 8 lives here too
//!
//! "Unknown tags survive to display (`Frame::UnknownTag`) rather than
//! panicking" (`plan/12:552`). An unknown tag is **recorded into the state**,
//! not merely not-crashed-on: [`GameState::unknown_tags`] is what a display
//! reads, so a test can assert the tag reached something a user would see
//! rather than asserting only that nothing blew up. Rule 2.2
//! (`plan/05:276-283`) requires the tag to reach the user *as text and a log*,
//! and "it did not panic" is neither.

use crate::effects::Effects;
use crate::status::StatusInfo;
use cena_protocol::Frame;
use cena_protocol::runs::Runs;
use idle::{IDLE_WARNING, IdleWarning};
use std::time::Instant;

pub mod armaments;
pub mod bounty;
pub mod character;
pub mod chunks;
pub mod claim;
mod clock;
pub mod combat;
pub mod creature;
pub mod creatures;
pub mod gameobj;
mod idle;
mod inventory;
mod nouns;
pub mod objectives;
mod reconnect;
mod room;
pub mod societies;
pub mod streams;
mod unknown;
pub mod vitals;

pub use character::{Character, Experience, Injury};
pub use inventory::{Container, Inventory};
pub use nouns::{Found, Where};
pub use objectives::Objectives;
pub use room::{PlayerStatus, Room, RoomItem};
pub use unknown::{MAX_UNKNOWN_TAGS, UnknownTag};
pub use vitals::{Vital, Vitals};

/// What the session knows: `plan/12` §7.1's In column, **plus** what
/// `plan/17` and `plan/18` added -- `status`, `effects`, `character`,
/// `inventory`, `streams` and the unknown-tag log. (This said "§7.1's In
/// column, exactly", which those sections made untrue.)
///
/// # `PartialEq` is hand-written, and deliberately ignores TWO fields
///
/// `game_time_received` is a **local `Instant`**, so two replays of
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
/// (`game_time`) IS state and is compared; the local instant it
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
    /// The quest and bounty list (`<objectives>`).
    pub objectives: Objectives,
    /// What the game says is true of the character right now.
    ///
    /// `plan/17` §2, ported from Vellum. Read it through the typed accessors
    /// ([`StatusInfo::stunned`] and friends) or by id; an id the game has
    /// never reported reads `false`, and [`StatusInfo::is_known`] is what
    /// separates that from a reported `false`.
    pub status: StatusInfo,
    /// Active spells, buffs, debuffs and cooldowns.
    ///
    /// Liveness is an expiry compared against [`Self::game_time_now`], never
    /// mere presence -- see [`crate::effects`].
    pub effects: Effects,
    /// The server's clock, from the last `<prompt time=>`.
    ///
    /// Private, because a raw reading is a trap: it is only correct at the
    /// instant the prompt arrived. [`GameState::game_time_now`] extrapolates
    /// it and is what callers want.
    game_time: Option<u32>,
    /// When `game_time` arrived, on the **monotonic** clock.
    ///
    /// `Instant`, not `SystemTime`: it must not move when NTP steps the wall
    /// clock or when DST changes, because the only thing it is used for is
    /// measuring an interval.
    game_time_received: Option<Instant>,
    /// Tags the parser did not model, in arrival order, **bounded**.
    ///
    /// Criterion 8: these **survive to display**. Rule 2.2 requires an
    /// unmodelled tag to reach the user, so it is kept here rather than counted
    /// and dropped.
    ///
    /// At most [`MAX_UNKNOWN_TAGS`], keeping the FIRST occurrences.
    /// [`GameState::unknown_tag_count`] keeps counting past that, so the bound
    /// costs "every instance" and never "how often". See
    /// `tests/unknown_tags_are_bounded.rs`.
    pub unknown_tags: Vec<UnknownTag>,
    /// How many of each unknown tag name have been seen, **unbounded by the
    /// ring**: a `u64` per distinct name, and the wire has a finite vocabulary.
    unknown_tag_counts: std::collections::BTreeMap<String, u64>,
    /// Experience, injuries, stance and encumbrance. `plan/18` §2b.
    pub character: Character,
    /// Containers and their contents. `plan/18` §2d.
    pub inventory: Inventory,
    /// Whether the server has warned that this character is idle.
    ///
    /// Private, and read through [`GameState::idle_warned`] /
    /// [`GameState::idle_warned_at`]. See `crates/cena-model/tests/
    /// idle_warning.rs` for the wire evidence and why this is a supervisor
    /// signal rather than display text.
    idle_warning: IdleWarning,
    /// Buffered display lines per stream id; `""` is the main window.
    ///
    /// Private, and read through [`GameState::stream`] / [`GameState::streams`],
    /// so "a stream nobody has pushed to" answers empty rather than making every
    /// call site unwrap an `Option`.
    streams: streams::StreamBuffers,
    /// Routed and discarded line tallies; see [`streams::LineTally`].
    tally: streams::LineTally,
    /// The line being assembled for each stream, not yet terminated.
    ///
    /// **A frame boundary is not a line boundary.** The parser emits one run per
    /// markup boundary, so `  a` + `<a>pebbled grey leather doublet</a>` is two
    /// frames of one line -- the split that printed the author's worn inventory
    /// down the screen. [`TextFrame::ends_line`] is what says where a line really
    /// ends, and this holds the runs until it does.
    ///
    /// Keyed by stream because two streams can be mid-line at once: a
    /// `pushStream` can interrupt an unterminated run and the enclosing stream
    /// resumes afterwards.
    pending: std::collections::BTreeMap<String, Runs>,
    /// The combat state machine (`state/combat/tracker.rs`). Private: its
    /// inputs are the chunk and the clock, both owned here.
    combat: combat::CombatTracker,
    /// Every creature the feed has shown, with what combat did to it.
    /// `state/creatures.rs`.
    creatures: creatures::Creatures,
    /// Lines of the command output since the last prompt.
    ///
    /// **The prompt is a universal boundary, not an `info`-specific one** --
    /// Lich closes container fills, combat chunks and its parser FSM on it
    /// (`state/chunks.rs` cites all three). Owning the buffer here means the
    /// `info` reader, a future `skill` reader and a combat tracker share one
    /// accumulator instead of each growing their own.
    chunk: chunks::Chunk,
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
            objectives,
            status,
            effects,
            game_time,
            game_time_received: _,
            unknown_tags,
            unknown_tag_counts,
            idle_warning,
            streams,
            // **Excluded from equality, for a different reason than
            // `game_time_received`.** That one is unreproducible; this is a
            // tally of the process's behaviour rather than a fact about the
            // game. Two states that know the same things are equal whether or
            // not one has been running longer.
            tally: _,
            // Excluded for the tally's reason: it holds a queue a consumer
            // drains and a handle the session provides, neither a fact
            // about the game. What it knows of the game -- a held cast, an
            // open assault -- is re-derived from the same chunks.
            combat: _,
            creatures,
            pending,
            chunk,
            character,
            inventory,
        } = self;
        creatures == &other.creatures
            && inventory == &other.inventory
            && character == &other.character
            && idle_warning == &other.idle_warning
            && streams == &other.streams
            && pending == &other.pending
            && chunk == &other.chunk
            && room == &other.room
            && prompt == &other.prompt
            && left_hand == &other.left_hand
            && right_hand == &other.right_hand
            && roundtime_ends == &other.roundtime_ends
            && vitals == &other.vitals
            && objectives == &other.objectives
            && status == &other.status
            && effects == &other.effects
            && game_time == &other.game_time
            && unknown_tags == &other.unknown_tags
            && unknown_tag_counts == &other.unknown_tag_counts
    }
}

impl GameState {
    /// The combat state machine, to read its facts.
    #[must_use]
    pub const fn combat(&self) -> &combat::CombatTracker {
        &self.combat
    }

    /// The combat state machine, to hand it crit tables or drain its facts.
    pub const fn combat_mut(&mut self) -> &mut combat::CombatTracker {
        &mut self.combat
    }

    /// The creature registry.
    #[must_use]
    pub const fn creatures(&self) -> &creatures::Creatures {
        &self.creatures
    }

    /// Fold one frame into the state.
    ///
    /// Returns `true` if the frame closed a round-trip window, which is the
    /// [`Frame::Prompt`] terminator (`plan/12` §4.4). The caller uses that to
    /// resolve a waiter; folding and window-closing are the same walk over the
    /// frame, so splitting them would mean matching twice.
    pub fn apply(&mut self, frame: &Frame) -> bool {
        match frame {
            Frame::RoomId { id } => self.arrive(id.as_deref()),
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
            // **Every room component, each replacing only its own entry.**
            // `plan/18` §2c and the author's requirement: the room is several
            // independent feeds, so a `room players` update must not disturb the
            // objects. See `state/room.rs` for the measurement.
            Frame::Component { id, body } => self.apply_room_component(id, body),
            Frame::CreatureStatus { id, attrs } => self.apply_creature_status(id, attrs),
            Frame::Compass { directions } => {
                // `Some`, even when `directions` is empty: the server SAID so,
                // and "observed, no cardinal exits" is a different fact from
                // "not looked yet". See `Room::exits`.
                self.room.exits = Some(directions.clone());
            }
            // The server's idle warning. Text-derived, and the ONE line that is,
            // for the reasons in `tests/idle_warning.rs`: it is an exact string
            // with nothing to extract, and the supervisor cannot otherwise tell
            // an idle kick from a network drop.
            //
            // `trim() ==`, not `contains`: a player can say anything, and a
            // `contains` would let one make the supervisor stop reconnecting by
            // typing it. The bells the wire wraps it in are already gone --
            // `text::strip_control_chars` runs in the parser.
            Frame::Text(text) => {
                if text.content.trim() == IDLE_WARNING {
                    // The clock as it stands, which may be `None` during a login
                    // burst. Stamped rather than derived later because
                    // `game_time_now` extrapolates and this is a point in time.
                    self.idle_warning = match self.game_time {
                        Some(at) => IdleWarning::At(at),
                        None => IdleWarning::Unstamped,
                    };
                }
                // **And it is still routed.** Observing a line must not consume
                // it (Rule 2.2): the player has to see the warning. This used to
                // be a guarded arm that swallowed the frame, which was invisible
                // while nothing else read text and would have silently dropped
                // the one line from the buffer the moment routing arrived.
                self.route_text(text);
            }
            // NOTHING INBOUND CLEARS IT, and that is a measurement rather than
            // an omission. The obvious guess -- "the next prompt means the
            // player answered" -- is false: MEASURED on `GSIV-Dicate`
            // (2024-10-12), the session idled on for minutes after the warning
            // with prompts arriving every few seconds, driven by `dialogData
            // id='Buffs'` refreshes and by a bystander emoting. A prompt is sent
            // when ANYTHING happens, not when the player acts.
            //
            // Only an OUTBOUND command answers an idle warning, and this model
            // is inbound-only by design. So the actor clears it on write --
            // `cena-session/src/actor/io.rs`, in `write_bounded`, which is the
            // one chokepoint all three send paths pass through.
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
                // **The chunk closes here**, and this is the only place it
                // does. See `state/chunks.rs`: Lich closes container fills,
                // combat chunks and its own parser FSM on the prompt, for the
                // same reason -- a command's output has no terminator of its
                // own.
                self.close_chunk();
                return true;
            }
            Frame::StatusIndicator { id, active } => {
                self.status.set(id, *active);
            }
            Frame::ClearDialogData { id } => {
                // MEASURED: `<dialogData id='Buffs' clear='t'></dialogData>`
                // arrives EMPTY, immediately followed by the populated
                // element. Clear then refill, per category -- a `Buffs` clear
                // says nothing about `Cooldowns`.
                if crate::effects::is_effect_dialog(id) {
                    self.effects.clear_category(id);
                }
            }
            Frame::LeftHand { item, .. } => self.left_hand = Some(item.clone()),
            Frame::RightHand { item, .. } => self.right_hand = Some(item.clone()),
            Frame::RoundTime { value } => self.roundtime_ends = Some(*value),
            // `plan/18` step 4. `<container>` declares, `<clearContainer>`
            // empties, `<inv>` adds one line; see `state/inventory.rs`.
            Frame::Container { id, title, target } => {
                self.inventory
                    .declare(id, title.as_deref(), target.as_deref());
            }
            Frame::ClearContainer { id } => self.inventory.clear(id),
            Frame::DeleteContainer { id } => self.inventory.delete(id),
            Frame::ContainerItem {
                container_id,
                content,
            } => self.inventory.add_line(container_id, content),
            // `plan/18` step 3. Both carry the enclosing dialog, which is the
            // only thing separating `yourLvl` in `expr` from a map legend, or a
            // body part from one of the 1,313 `nomap.jpg` tiles.
            Frame::Label {
                id,
                value,
                dialog: Some(dialog),
            } => self.character.apply_label(dialog, id, value),
            Frame::InjuryImage { id, name, dialog } => {
                if dialog.as_deref() == Some("injuries") {
                    self.character.apply_injury_image(id, name);
                }
            }
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
                // EFFECTS FIRST. The same `<progressBar>` shape carries
                // vitals, stance and effects; the enclosing dialog id is the
                // only thing that tells them apart (MEASURED 2026-09-18: one
                // burst carried `minivitals`, `combat`, `stance`, `Buffs`,
                // `Cooldowns` and `Active Spells` bars, all as progressBars).
                if let Some(dialog) = bar.dialog.as_deref()
                    && crate::effects::is_effect_dialog(dialog)
                {
                    // `time_remaining_secs` is a DURATION -- the wire sends
                    // `time='00:01:59'`. Adding it to the server clock once,
                    // here, is what makes it comparable later; the duration
                    // itself goes stale immediately because the game only
                    // re-sends an effect when it changes.
                    //
                    // **`game_time`, not `game_time_now()`.**
                    //
                    // `game_time_now()` extrapolates: it adds
                    // `game_time_received.elapsed()`, a reading of the LOCAL
                    // monotonic clock. Baking that into `ends_at` put a local
                    // measurement inside a value `PartialEq` compares -- and
                    // `game_time_received` is destructured to `_` in that impl
                    // specifically to keep local readings out of it. The
                    // exclusion was correct and was being routed around
                    // through this field (review MO-1).
                    //
                    // The cost was replay equality. Live, a refill arriving 90
                    // seconds after its prompt gave `base + 90 + secs`;
                    // replayed from the same recording, the frames arrive
                    // back-to-back and give `base + secs`. Same bytes,
                    // unequal state -- against criterion 7, which is what M2's
                    // golden corpus rests on.
                    //
                    // Using the raw server clock needs no local reading at
                    // all: the server sent the time and the duration, and
                    // their sum is what the server said. The extrapolation
                    // was never adding information, only the delay between
                    // two frames the game sent together.
                    //
                    // `game_time_now()` remains right for READING the clock --
                    // `in_roundtime` needs to know what time it is now. It is
                    // wrong for STAMPING a fact the server already dated.
                    let ends_at = bar
                        .time_remaining_secs
                        .and_then(|secs| Some(self.game_time?.saturating_add(secs)));
                    self.effects.insert(
                        bar.id.clone(),
                        crate::effects::Effect {
                            category: dialog.to_owned(),
                            text: bar.text.clone(),
                            ends_at,
                            percent: bar.percent,
                        },
                    );
                    return false;
                }
                // Step 3's dialogs, before vitals and for the same reason
                // effects come before both: one `<progressBar>` shape carries
                // gauges, stance, encumbrance and advancement, and the enclosing
                // dialog is the only thing that tells them apart.
                if let Some(dialog) = bar.dialog.as_deref()
                    && self
                        .character
                        .apply_bar(dialog, &bar.id, &bar.text, bar.percent)
                {
                    return false;
                }
                self.record_vital(bar);
            }
            // `<clearStream id=>`: the wire's own snapshot boundary, and the
            // ONLY thing that empties a buffer. See `state/streams.rs` for the
            // measurement that chose this over clearing on push.
            // The room's environment. Part of the room, so it is invalidated
            // with the rest of it on a reconnect.
            Frame::RoomMeta(meta) => self.room.meta = Some(*meta),
            Frame::ObjectivesUpdate { action, entries } => {
                self.objectives.apply(*action, entries);
            }
            Frame::ClearStream { id } => {
                self.clear_stream(id);
                self.pending.remove(id);
            }
            Frame::UnknownTag { name, raw } => self.record_unknown_tag(name, raw),
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
