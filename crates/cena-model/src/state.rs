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

pub mod afflictions;
pub mod armaments;
pub mod bank;
pub mod bounty;
pub mod bounty_status;
pub mod character;
pub mod chunks;
pub mod claim;
mod clock;
pub mod combat;
pub mod containers;
pub mod cooldowns;
pub mod creature;
pub mod creature_message;
pub mod creatures;
pub mod departure;
pub mod disk;
pub mod doses;
pub mod equality;
pub mod fog;
pub mod gameobj;
pub mod group;
pub mod hands;
mod idle;
mod inventory;
pub mod inventory_snapshot;
pub mod kit;
pub mod known_spells;
pub mod ledger;
pub mod maneuvers;
pub mod menu;
pub mod message;
pub mod movement;
mod nouns;
mod numbers;
pub mod objectives;
pub mod order_menu;
pub mod overwatch;
mod reconnect;
pub mod resolve;
mod ring;
mod room;
pub mod societies;
mod spell_time;
pub mod stream_windows;
pub mod streams;
pub mod targeting;
mod unknown;
pub mod vitals;

pub use character::{Character, Experience, Injury};
pub use disk::{DISK_NOUNS, Disk};
pub use group::{Group, GroupEvent, Member};
pub use inventory::{Container, Inventory};
pub use inventory_snapshot::InventorySnapshot;
pub use menu::{LearnedCommands, MenuCommand, MenuCommands, ResolvedItem};
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
    /// How many rooms the character has arrived in this session.
    ///
    /// **Not a room id.** It exists so a consumer can tell a move between two
    /// rooms that read identically from standing still -- see
    /// `Room::arrive` for why their ids can collide, and `fog::moved_from`
    /// for the consumer. Wraps rather than saturating: what matters is
    /// whether it CHANGED, and a saturated counter would silently stop
    /// answering that after 4 billion rooms.
    pub arrivals: u32,
    /// The last `<prompt>` text, e.g. `">"`.
    pub prompt: Option<String>,
    /// `<left>` hand contents, with the `exist` id the wire sends.
    pub left_hand: hands::Hand,
    /// `<right>` hand contents, with the `exist` id the wire sends.
    pub right_hand: hands::Hand,
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
    /// The bounty task itself: what the guild asked for, and what it will do
    /// next (`bounty_status.rs`). Distinct from [`Self::objectives`], which is
    /// the dialog's row -- this is the task's own description, parsed.
    pub bounty: bounty_status::BountyStatus,
    /// Doses left in each herb measured, by item id (`doses.rs`).
    pub doses: doses::Doses,
    /// Survivalist's Kits, as `analyze` and `look in` describe them (`kit.rs`).
    pub kits: kit::Kits,
    /// The last shop `order` menu seen (`order_menu.rs`).
    pub order_menu: order_menu::OrderMenu,
    /// Maneuvers the game has said are on cooldown (`maneuvers.rs`).
    pub maneuvers: maneuvers::Maneuvers,
    /// What the game says you can attack (`targeting.rs`): the `combat`
    /// dialog's `dDBTarget` list. Evidence of hostility, and the input to
    /// "something is here that I cannot see".
    pub targeting: targeting::Targeting,
    /// `<castTime value=>`: the epoch second a cast's hard roundtime ends.
    ///
    /// **Separate from [`Self::roundtime_ends`]**, as it is in Lich
    /// (`xmlparser.rb:770` keeps `@cast_roundtime_end` beside
    /// `@roundtime_end`): a cast time and an action roundtime run at once and
    /// expire independently, so one field could not answer either. `Option`,
    /// never `0`, for the reason `roundtime_ends` gives.
    pub cast_time_ends: Option<u32>,
    /// `<spell>`: the spell prepared, verbatim, `None` from the game when
    /// nothing is (Lich's `checkprep`). `None` here is *not told*.
    pub prepared: Option<String>,
    /// Who is grouped with you, by `exist` id.
    pub group: Group,
    /// The stow and ready lists: which container holds what, and which
    /// weapon comes to hand.
    pub containers: containers::Containers,
    /// What `bank account` last reported.
    pub bank: bank::Account,
    /// Which characters a spell has locked out (PR #1597).
    pub cooldowns: cooldowns::Cooldowns,
    /// What has been said, and by whom.
    pub messages: message::Messages,
    /// Where something was last seen hiding.
    pub overwatch: overwatch::Overwatch,
    /// The spells the game lists for this character (the `Spells` stream).
    pub known_spells: known_spells::KnownSpells,
    /// Dictionary rows the server has taught us this session
    /// (`<cmdlist>`), layered over the shipped table when a menu resolves.
    pub learned_commands: LearnedCommands,
    /// The whole-inventory snapshot (`<inventoryManager>`).
    ///
    /// Distinct from [`Self::inventory`], which is the passive container
    /// model. This one is requested, complete and point-in-time; that one is
    /// streamed, partial and live. See `inventory_snapshot.rs`.
    pub inventory_snapshot: InventorySnapshot,
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
    /// Closed-window routing, per stream: [`stream_windows`].
    stream_windows: stream_windows::Windows,
    /// Routed and discarded line tallies; see [`streams::LineTally`].
    tally: streams::LineTally,
    /// The line being assembled for each stream, not yet terminated.
    ///
    /// **A frame boundary is not a line boundary.** The parser emits one run per
    /// markup boundary, so `  a` + `<a>pebbled grey leather doublet</a>` is two
    /// frames of one line -- the split that printed the author's worn inventory
    /// down the screen. `TextFrame`'s `ends_line` is what says where a line really
    /// ends, and this holds the runs until it does.
    ///
    /// Keyed by stream because two streams can be mid-line at once: a
    /// `pushStream` can interrupt an unterminated run and the enclosing stream
    /// resumes afterwards.
    pending: std::collections::BTreeMap<String, Runs>,
    /// The combat state machine (`state/combat/tracker.rs`). Private: its
    /// inputs are the chunk and the clock, both owned here.
    combat: combat::CombatTracker,
    /// Loot facts classified at each prompt, waiting for the session's
    /// ledger (`ledger/pending.rs`). Drained by [`GameState::take_loot`].
    loot: ledger::LootQueue,
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

impl GameState {
    /// The combat state machine, to read its facts.
    #[must_use]
    pub const fn combat(&self) -> &combat::CombatTracker {
        &self.combat
    }

    /// Every classified loot chunk since the last call, oldest first.
    pub fn take_loot(&mut self) -> Vec<ledger::LootChunk> {
        self.loot.take()
    }

    /// Loot chunks lost to the pending cap before being drained.
    #[must_use]
    pub const fn loot_dropped(&self) -> u64 {
        self.loot.dropped()
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
            Frame::StreamWindow { .. } => self.apply_stream_window(frame),
            // See `apply_room_component` for why the styled form is refused.
            Frame::Component { id, body } => self.apply_room_component(id, body),
            Frame::CreatureStatus { id, attrs } => self.apply_creature_status(id, attrs),
            Frame::Compass { directions } => {
                // `Some`, even when `directions` is empty: the server SAID so,
                // and "observed, no cardinal exits" is a different fact from
                // "not looked yet". See `Room::exits`.
                self.room.exits = Some(directions.clone());
            }
            // See `state/idle.rs` for why this one line is text-derived.
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
                    // Effects that arrived with no clock -- the login burst
                    // precedes its first prompt -- get their end time now
                    // (`effects.rs`, `Effects::pending`).
                    self.effects.anchor(t);
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
                // `GROUP_EMPTIED` (`group.rb:603-605`): the indicator going
                // dark is the game saying you are in no group. See
                // `Group::emptied`.
                if id == "IconJOINED" && !*active {
                    self.group.emptied();
                }
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
            Frame::AppInfo { .. } => self.character.identify(frame),
            Frame::LeftHand { item, link } => {
                self.left_hand = hands::Hand::read(item, link.as_ref());
            }
            Frame::RightHand { item, link } => {
                self.right_hand = hands::Hand::read(item, link.as_ref());
            }
            Frame::RoundTime { value } => self.roundtime_ends = Some(*value),
            // `plan/18` step 4, all four in `state/inventory.rs`.
            Frame::Container { .. }
            | Frame::ClearContainer { .. }
            | Frame::DeleteContainer { .. }
            | Frame::ContainerItem { .. } => self.apply_container(frame),
            // `plan/18` step 3. Both carry the enclosing dialog, which is the
            // only thing separating `yourLvl` in `expr` from a map legend, or a
            // body part from one of the 1,313 `nomap.jpg` tiles.
            Frame::Label {
                id,
                value,
                dialog: Some(dialog),
                ..
            } => self.character.apply_label(dialog, id, value),
            Frame::InjuryImage {
                id, name, dialog, ..
            } => {
                if dialog.as_deref() == Some("injuries") {
                    self.character.apply_injury_image(id, name);
                }
            }
            Frame::ProgressBar(bar) => self.apply_progress_bar(bar),
            // The room's environment. Part of the room, so it is invalidated
            // with the rest of it on a reconnect.
            // A cast's hard roundtime, which is not the action roundtime.
            Frame::CastTime { value } => self.cast_time_ends = Some(*value),
            Frame::Spell { text } => self.prepared = Some(text.trim().to_owned()),
            // The `combat` dialog's target dropdown. MEASURED the noisiest
            // widget on the wire and read by nothing until 2026-09-21; see
            // `targeting.rs` for why a display widget is a model fact.
            Frame::DialogWidgets(widgets) => self.read_widgets(widgets),
            Frame::RoomMeta(meta) => self.room.meta = Some(*meta),
            Frame::ObjectivesUpdate { action, entries } => {
                self.objectives.apply(*action, entries);
            }
            // Both inventory responses, moved down under Rule 4.1: this
            // function was at 113 of 100 lines.
            Frame::CmdListUpdate(update) => self.learned_commands.apply(&update.entries),
            Frame::CmdTimestamp { version } => self.learned_commands.set_version(version),
            Frame::InventoryManager(_) | Frame::InventoryViewItem(_) => {
                self.apply_inventory(frame);
            }
            // `<clearStream id=>`: the wire's own snapshot boundary, and the
            // ONLY thing that empties a buffer. See `state/streams.rs` for the
            // measurement that chose this over clearing on push.
            Frame::ClearStream { id } => {
                self.clear_stream(id);
                self.pending.remove(id);
                if id == known_spells::STREAM {
                    self.known_spells.begin();
                }
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

impl GameState {
    /// Targetable ids that are in no room list: **something is here that you
    /// cannot see.**
    ///
    /// Lich's `GameObj.hidden_targets` (`gameobj.rb:1171`), and the author's
    /// point about `dDBTarget`: the server decides what goes in that dropdown,
    /// so an id it offers while the room shows nothing is the game contradicting
    /// what you can see. `overwatch.rb:117-120` pushes onto the same list from
    /// the other direction.
    ///
    /// Empty is the ordinary answer. `Targeting::is_stated` is what separates
    /// "nothing hidden" from "never told".
    #[must_use]
    pub fn hidden_targets(&self) -> Vec<i64> {
        let known: Vec<i64> = self.creatures.in_room().map(|c| c.id).collect();
        self.targeting.hidden(&known).collect()
    }
}
