//! The command authority (`plan/12` §4.2): who may run a sequence, held by
//! the **session**, not by one connection.
//!
//! # SE-4: it outlives a reconnect
//!
//! It lived in each connection's `CommandQueue`, so a reconnect built a new
//! queue with nobody holding it: a behavior that waited out an outage came
//! back to find its token refused `Permanent` -- "will never succeed" -- on a
//! session that was alive (`plan/19`, "Command authority is rebuilt per
//! generation"). The author chose that the authority belong to the session
//! (`plan/30` §6 Q3, option (c)): a network blip should not end a two-hour
//! hunt, and no other claimant should win a race it did not know it was in.
//!
//! So it is one cell, shared by every connection's queue, the supervisor
//! (which answers claims and releases between connections) and every handle
//! (which can read it and take it back).
//!
//! # Preemption (`plan/12` §4.3)
//!
//! A stop signals the holder's cancel token and waits up to
//! [`PREEMPT_GRACE`] for it to release. If it has not, the authority is
//! **revoked** anyway: the holder's queued commands are refused, and its
//! next ones too, so a behavior that ignores its stop cannot keep the
//! character.

use std::sync::{Arc, Mutex, PoisonError};
use std::time::Duration;

use tokio_util::sync::CancellationToken;

use super::handle::SessionHandle;
use crate::queue::{AuthorityHeld, AuthorityToken};

/// How long a stopped holder has to let go before its authority is taken
/// (`plan/12` §4.3: "Waits up to `PREEMPT_GRACE` (default 250ms)").
pub const PREEMPT_GRACE: Duration = Duration::from_millis(250);

/// How often a preemption looks to see whether the holder has let go.
const PREEMPT_POLL: Duration = Duration::from_millis(10);

/// The one authority cell of a session.
///
/// A lock, never held across an await: every use is a read or a swap.
#[derive(Clone, Debug, Default)]
pub(crate) struct Authority(Arc<Mutex<Option<AuthorityToken>>>);

impl Authority {
    fn with<T>(&self, f: impl FnOnce(&mut Option<AuthorityToken>) -> T) -> T {
        f(&mut self.0.lock().unwrap_or_else(PoisonError::into_inner))
    }

    /// Grant it, or say who has it.
    pub(crate) fn claim(&self, token: AuthorityToken) -> Result<(), AuthorityHeld> {
        self.with(|held| {
            if let Some(holder) = *held {
                return Err(AuthorityHeld(holder));
            }
            *held = Some(token);
            Ok(())
        })
    }

    /// Give it back. Ignored unless `token` holds it.
    pub(crate) fn release(&self, token: AuthorityToken) {
        self.with(|held| {
            if *held == Some(token) {
                *held = None;
            }
        });
    }

    /// Who holds it.
    pub(crate) fn holder(&self) -> Option<AuthorityToken> {
        self.with(|held| *held)
    }

    /// Take it from `token`, whether or not it lets go. `false` if `token`
    /// no longer held it.
    fn revoke(&self, token: AuthorityToken) -> bool {
        self.with(|held| {
            let had = *held == Some(token);
            if had {
                *held = None;
            }
            had
        })
    }
}

/// How a [`SessionHandle::preempt`] ended.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Preempted {
    /// Nobody held the authority; the stop was only signalled.
    Nothing,
    /// The holder let go within [`PREEMPT_GRACE`].
    Yielded(AuthorityToken),
    /// The holder did not let go, and its authority was taken (§4.3:
    /// "revokes the authority anyway and marks the holder `Aborted`").
    Revoked(AuthorityToken),
}

impl SessionHandle {
    /// Who holds the command authority, if anyone.
    #[must_use]
    pub fn holder(&self) -> Option<AuthorityToken> {
        self.authority.holder()
    }

    /// Stop the authority's holder: cancel `stop`, which is the holder's own
    /// cancel token, and take the authority if it has not let go within
    /// [`PREEMPT_GRACE`] (`plan/12` §4.3). Its queued commands are then
    /// refused rather than sent.
    ///
    /// Only an explicit stop preempts; manual input never does (§4.1).
    pub async fn preempt(&self, stop: &CancellationToken) -> Preempted {
        stop.cancel();
        let Some(holder) = self.authority.holder() else {
            return Preempted::Nothing;
        };
        let deadline = tokio::time::Instant::now() + PREEMPT_GRACE;
        while tokio::time::Instant::now() < deadline {
            if self.authority.holder() != Some(holder) {
                return Preempted::Yielded(holder);
            }
            tokio::time::sleep(PREEMPT_POLL).await;
        }
        if self.authority.revoke(holder) {
            Preempted::Revoked(holder)
        } else {
            Preempted::Yielded(holder)
        }
    }
}
