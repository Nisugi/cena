//! cena-session
//!
//! One character's connection, as one actor: it owns the socket, the parser,
//! the command queue and the state, and it runs as one supervised task.
//!
//! **The session owns the socket for one connection.** [`supervisor`] owns
//! what outlives one: credentials, the command receiver, the event sender, the
//! generation and the carried-over state.
//!
//! This said there was no such layer, on the grounds that `plan/12` §9c moved
//! reconnect to Milestone 2 and a manager would be a trait with one
//! implementor. Reconnect is built and live-verified, and the manager is a
//! module rather than a crate or a trait, so Rule -1 is satisfied without the
//! abstraction that clause was refusing.
//!
//! Read [`queue`] for `plan/12` §4 (command ownership, the authority token,
//! and the corrected rule that manual input **interleaves rather than
//! preempts**), [`lifecycle`] for §5 and for which two of its states are
//! deliberately not built, [`command`] for the typed [`Outcome`] and why
//! `send_and_await` is one call, `cena_model::state` for what is known and how criteria
//! 2 and 8 land in it, and [`actor`] for the select loop.
//!
//! # Choosing an observation API
//!
//! [`Session::subscribe`] and [`SupervisedSession::subscribe`] are legacy,
//! pre-run subscriptions only. Their receiver carries unnumbered [`Event`]s:
//! it cannot be fenced using [`Snapshot::cursor`]. Do not combine that receiver
//! with snapshots from another subscription. For late attachment or lag recovery,
//! obtain a [`SessionObserver`] before consuming the owner in `run`, then call
//! [`SessionObserver::subscribe`] for a fresh snapshot and its matching numbered
//! [`ObservedEvent`] stream. Observing does not confer command authority.

pub mod actor;
pub mod character_store;
pub mod combat_recorder;
pub mod command;
pub mod dirty_groups;
pub mod ledger;
pub mod lifecycle;
pub mod menu_store;
pub mod notice;
mod observation;
pub mod player_log;
pub mod queue;
pub mod settings_store;
pub mod store;
pub mod supervisor;
pub mod travel_store;

pub use actor::{EndReason, Event, SETUP_DEADLINE, Session, SessionActor, Snapshot};
// `CritTables` is this crate's own vocabulary, not a relay: `with_crit_tables`
// takes one, and the binary that calls it has no edge to `cena-model`.
pub use cena_model::crit::CritTables;
pub use cena_model::movement::{self, MoveFeedback};
// What the travel driver reads to store the hands and cast (`plan/24` 4c).
pub use cena_model::state::character::snapshot::{CharacterSnapshot, Group};
pub use cena_model::state::character::stance::Stance;
pub use cena_model::state::{claim, containers, gameobj, group, hands, stream_windows};
// The loot ledger's facts, for the town planner that reads them from its own
// fold of the stream (`plan/31` Stage 4).
pub use cena_model::{Appraiser, Buyer, LootFact};
pub use cena_model::{ChunkLine, GameState, Room, RoomItem, UnknownTag};
// The creature a hunt reads and the status words it asks about: `plan/30`
// section 1's table of what the model answers a hunter with. Beside the
// other model vocabulary re-exported for behaviors, on the same terms.
pub use cena_model::{Able, CreatureInstance, Effect, Injuries, StatusName};
// The bounty's task, for a behavior that narrows its work to it (eloot's
// `skin_bounty_only`).
pub use cena_model::{
    PsmCategory, PsmLine, PsmRanks, SkillKind, SkillLine, Society, Vital, spell_named, spells,
};
pub use cena_model::{Task, TaskKind};
// The herbs eherbs knows and where they are sold (`plan/36`), and the
// wound and scar reader the healing chooses by.
pub use cena_model::herbs;
pub use cena_model::state::character::body;
pub use cena_model::state::kit;
pub use cena_protocol::InventoryItem;
pub use cena_protocol::frame::{Amount, Link, LinkKind, ProgressBar, TextFrame};
pub use cena_protocol::runs::{Run, Runs};
pub use character_store::MAX_STALE;
pub use command::{
    CommandId, Envelope, Farewell, Gate, Origin, Outcome, PREEMPT_GRACE, Preempted, Refusal, Sent,
    SessionHandle,
};
pub use lifecycle::{Generation, GenerationCell, SessionId, State};
pub use notice::{Body, Notice, NoticeKind};
pub use observation::{ObserveError, ObservedEvent, RetryStatus, SessionObserver};
pub use player_log::writer::PlayerWriter;
pub use player_log::{LogLine, LogSink, PlayerLog};
pub use queue::{AuthorityHeld, AuthorityToken, CommandQueue};
pub use supervisor::{
    ConnectError, Connector, LONG_LIVED, MAX_UNATTENDED_LOSSES, Retryability, STABLE_CONNECTION,
    SessionCore, StoppedBecause, SupervisedEnd, SupervisedSession, backoff,
};

/// The wire vocabulary, re-exported for behaviors.
///
/// A behavior supplies a matcher (`plan/12` §4.4, "the waiter's matcher"), so
/// it must be able to name what it is matching on. This is NOT the
/// pass-through facade Rule -1 forbids: `cena-session` already owns a `Parser`
/// and matches on `Frame` throughout, so this is its own vocabulary rather
/// than a relay for a type it does not use. It also keeps `cena-behavior`'s
/// dependency row at `["cena-session"]`, which is what `plan/12` §2 says.
pub use cena_protocol::Frame;
