//! [`Connector`]: how a new connection is obtained.
//!
//! # Two implementors, and that is the point
//!
//! `plan/05` §-1 forbids a trait with one implementor, and this one has two for
//! the same reason [`ByteSource`](cena_platform::ByteSource) does: the real
//! transport and a test double are genuinely different things, and **criterion
//! 9 requires the second**. It says the reconnect is "verified in the replay",
//! and a replay cannot log in — so a connector that hands back prepared sources
//! is required *by the criterion*, not invented to satisfy the rule.
//!
//! The live implementor belongs in the **binary**, not here. `authenticate`
//! needs `Credentials`, and `cena-session` must not learn a login protocol;
//! `cena-platform` is *below* `cena-session` so it cannot implement a trait
//! defined here. The binary is the one layer that can hold both ends, which is
//! verbatim the argument `crates/cena-arch-tests/tests/layering.rs` already
//! makes for its `cena → cena-platform` edge.
//!
//! # Why this is not `Box<dyn Connector>`
//!
//! [`ByteSource`](cena_platform::ByteSource) uses RPITIT (`-> impl Future`), so
//! it is **not object-safe** and `Box<dyn ByteSource>` does not exist. A
//! connector therefore cannot hand back a boxed source, and the supervisor is
//! generic over the connector rather than holding a trait object. That is a
//! constraint inherited from the transport, not a design preference.

use crate::lifecycle::Generation;
use cena_platform::ByteSource;
use std::future::Future;

/// Opens connections for a supervised session.
pub trait Connector: Send {
    /// The transport this produces. One concrete type per connector, because
    /// [`ByteSource`] is not object-safe.
    type Source: ByteSource;

    /// Open a connection for `generation`.
    ///
    /// The generation is **passed in rather than counted here**, so a connector
    /// cannot disagree with the session about which connection it is building.
    /// The supervisor owns the counter; a connector that kept its own could
    /// drift from it, and the resulting mismatch would discard live commands.
    ///
    /// # Errors
    ///
    /// Any failure to obtain a transport. The connector **reports**; the
    /// supervisor decides whether to retry, because only it knows how many
    /// attempts have been made.
    fn connect(
        &mut self,
        generation: Generation,
    ) -> impl Future<Output = Result<Self::Source, ConnectError>> + Send;
}

/// Why a connection could not be opened.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ConnectError {
    /// Which step failed, for the log. Mirrors `EaccessError::stage`.
    pub stage: &'static str,
    /// What went wrong. **Already redacted by the connector** -- a password or
    /// a session key must never reach this field, because it is logged.
    pub detail: String,
}

impl std::fmt::Display for ConnectError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "[{}] {}", self.stage, self.detail)
    }
}

impl std::error::Error for ConnectError {}
