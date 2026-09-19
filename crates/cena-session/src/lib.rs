//! cena-session
//!
//! One character's connection, as one actor: it owns the socket, the parser,
//! the command queue and the state, and it runs as one supervised task.
//!
//! **The session owns the socket.** There is no connection-manager layer above
//! it. `plan/12` §9c moves reconnect to Milestone 2, so a manager today would
//! be a trait with one implementor -- Rule -1 (`plan/05` §-1).
//!
//! Read [`queue`] for `plan/12` §4 (command ownership, the authority token,
//! and the corrected rule that manual input **interleaves rather than
//! preempts**), [`lifecycle`] for §5 and for which two of its states are
//! deliberately not built, [`command`] for the typed [`Outcome`] and why
//! `send_and_await` is one call, [`state`] for what is known and how criteria
//! 2 and 8 land in it, and [`actor`] for the select loop.

pub mod actor;
pub mod command;
pub mod lifecycle;
pub mod queue;

pub use actor::{EndReason, Event, Session, SessionActor, Snapshot};
pub use cena_model::{GameState, Room, UnknownTag};
pub use command::{CommandId, Envelope, Gate, Origin, Outcome, Refusal, Sent, SessionHandle};
pub use lifecycle::{Generation, GenerationCell, State};
pub use queue::{AuthorityHeld, AuthorityToken, CommandQueue};

/// The wire vocabulary, re-exported for behaviors.
///
/// A behavior supplies a matcher (`plan/12` §4.4, "the waiter's matcher"), so
/// it must be able to name what it is matching on. This is NOT the
/// pass-through facade Rule -1 forbids: `cena-session` already owns a `Parser`
/// and matches on `Frame` throughout, so this is its own vocabulary rather
/// than a relay for a type it does not use. It also keeps `cena-behavior`'s
/// dependency row at `["cena-session"]`, which is what `plan/12` §2 says.
pub use cena_protocol::Frame;
