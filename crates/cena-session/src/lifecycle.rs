//! Session lifecycle: the states Step 2 can reach, and [`Generation`].
//!
//! `plan/12` §5.1 gives the full contract:
//!
//! ```text
//! Connecting -> Authenticating -> Syncing -> Ready -> { Degraded | Reconnecting } -> Closed
//! ```
//!
//! # Two of those states are deliberately NOT built
//!
//! | State | Step 2 | Why |
//! |---|---|---|
//! | `Connecting` | in | criterion 1 is built-not-exercised; replay enters it too |
//! | `Authenticating` | in | ditto; replay transits it without work |
//! | `Syncing` | in, pass-through | §5.3 gates behaviors on `Ready`, and criterion 4 needs that gate to exist |
//! | `Ready` | in | behaviors run here |
//! | `Closed` | in | criterion 6 |
//! | `Degraded` | **OUT** | |
//! | `Reconnecting` | **OUT** | |
//!
//! **`Degraded` is out because it would be a state with an entry and no
//! exit.** §5.4's entry condition is a truncation-class parse error that
//! triggers a *targeted re-sync*, and there is no re-sync in Step 2 (§7.1 puts
//! the ~15-command Infomon sync in the Out column). A session that entered
//! `Degraded` could never leave it, which is worse than not having the state:
//! the variant would exist, the gate would consult it, and nothing would ever
//! set it -- a config option with one value, in enum form.
//!
//! **`Reconnecting` is out because `plan/12` §9c says so explicitly**:
//! "reconnect and criterion 9 moved to Milestone 2."
//!
//! `Syncing` was the close call, and the distinction that kept it is that
//! `Syncing` is *transited* while `Degraded` would be *entered and never
//! left*. It earns its existence by making §5.3's readiness gate nameable:
//! without it, "behaviors may not start until `Ready`" has nothing to be
//! not-`Ready` *at*, and criterion 4's stop test would run against a gate
//! whose false branch is unreachable.

/// Which connection a fact belongs to.
///
/// **Kept even though reconnect is Milestone 2.** It costs one `u32`; `plan/12`
/// §4.4 makes it the discard rule for late frames ("generation mismatch --
/// discarded, belongs to a previous connection"); and criterion 7's replay has
/// to produce the same generation on every run, which it does because the
/// counter is seeded at 0 rather than from a clock. Retrofitting an id onto
/// every `CommandId`, event and snapshot after they exist is the expensive
/// version of this decision.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Generation(pub u32);

impl Generation {
    /// The first connection. Deterministic by construction: a replay always
    /// starts here, never at a clock reading or a random seed.
    pub const FIRST: Self = Self(0);

    /// The next connection's generation.
    ///
    /// Unused in Step 2 -- nothing reconnects -- but it is the one line that
    /// makes `Generation` a counter rather than a constant, and Milestone 2's
    /// reconnect is its only caller.
    #[must_use]
    pub const fn next(self) -> Self {
        Self(self.0 + 1)
    }
}

/// Where a session is in its life. Five of `plan/12` §5.1's seven; see the
/// module docs for the two that are deliberately absent.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum State {
    /// Opening the transport.
    #[default]
    Connecting,
    /// The `EAccess` handshake. A replay transits this without work.
    Authenticating,
    /// The login-state queries. A pass-through in Step 2: §7.1 puts the
    /// ~15-command Infomon sync in M1's Out column.
    Syncing,
    /// State is trustworthy; behaviors may run.
    Ready,
    /// The transport is gone and the task has ended.
    Closed,
}

impl State {
    /// Whether a behavior may start.
    ///
    /// `plan/12` §5.3: "**Behaviors may not start until `Ready`.**" This is the
    /// gate, and it is the reason `Syncing` exists at all in Step 2 -- a gate
    /// whose false branch is unreachable is not a gate.
    #[must_use]
    pub fn behaviors_may_run(self) -> bool {
        matches!(self, Self::Ready)
    }
}
