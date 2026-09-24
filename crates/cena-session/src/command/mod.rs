//! What a command is, how it travels, and what comes back.
//!
//! Two modules, split when this one hit its cap as `send_now` landed:
//!
//! * `verdict` -- the **vocabulary**. [`Origin`], [`Refusal`], [`Outcome`],
//!   [`Sent`], [`Gate`]: who asked, and what happened.
//! * `handle` -- the **transport**. [`SessionHandle`], [`Envelope`],
//!   [`Inbox`]: how a command reaches the actor.
//!
//! The seam is the one the line cap forced and it turned out to be the right
//! one: a behavior imports the vocabulary and reasons in it; only the session
//! touches the transport.

pub(crate) mod attendance;
pub mod claimant;
mod handle;
mod round_trip;
mod verdict;

pub use claimant::{Claimed, DEFAULT_SYMBOL as COMMAND_SYMBOL};
pub use handle::{Envelope, Farewell, Inbox, SessionHandle};
pub use verdict::{CommandId, Gate, Origin, Outcome, Refusal, Sent};
