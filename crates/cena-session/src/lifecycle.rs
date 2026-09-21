//! Session lifecycle: the states a session can reach, [`Generation`], and the
//! [`GenerationCell`] that lets a handle outlive a connection.
//!
//! `plan/12` §5.1 gives the full contract:
//!
//! ```text
//! Connecting -> Authenticating -> Syncing -> Ready -> { Degraded | Reconnecting } -> Closed
//! ```
//!
//! # Six of seven are built; `Degraded` is not
//!
//! | State | Built | Why |
//! |---|---|---|
//! | `Connecting` | yes | criterion 1; a replay enters it too |
//! | `Authenticating` | yes | ditto; a replay transits it without work |
//! | `Syncing` | yes, pass-through | §5.3 gates behaviors on `Ready`, and criterion 4 needs that gate to exist |
//! | `Ready` | yes | behaviors run here |
//! | `Reconnecting` | **yes, Milestone 2** | see below |
//! | `Closed` | yes | criterion 6 |
//! | `Degraded` | **NO** | a state with an entry and no exit |
//!
//! **`Reconnecting` arrived in Milestone 2.** It was out for a *scope* reason
//! rather than a design one -- "`plan/12` §9c says so explicitly: reconnect and
//! criterion 9 moved to Milestone 2" -- and that reason expired when this
//! milestone began.
//!
//! It earns its place the same way `Syncing` does: it makes §5.1's rule
//! **"No automation runs"** enforceable by a state rather than by a convention.
//! [`State::behaviors_may_run`] is `matches!(self, Self::Ready)`, so adding the
//! variant gates automation with **no change to the gate** -- which is the
//! shape a state should have.
//!
//! **`Degraded` is still out, and for a reason that has not expired.** §5.4's
//! entry condition is a truncation-class parse error that triggers a *targeted
//! re-sync*, and there is still no re-sync (§7.1 puts the ~15-command Infomon
//! sync in the Out column). A session that entered `Degraded` could never leave
//! it, which is worse than not having the state: the variant would exist, the
//! gate would consult it, and nothing would ever set it -- a config option with
//! one value, in enum form.
//!
//! The distinction that keeps one and not the other is **transited versus
//! entered-and-never-left**. `Syncing` and `Reconnecting` are both passed
//! through; `Degraded` would be a trap.

/// Which **session** a fact belongs to.
///
/// **Added in Milestone 4 even though multi-session is Milestone 5**, on
/// exactly the argument [`Generation`] below makes for itself: *retrofitting an
/// id onto every event and snapshot after they exist is the expensive version
/// of this decision.* `Generation` was kept a milestone early for that reason
/// and the reasoning transfers whole.
///
/// The immediate need is M4's, not M5's. A frontend serves **one listener on
/// one port** (`plan/23` §D1a) — not a port per character, which for `plan/12`'s
/// 3-25 sessions would mean 25 allocations and a user who must know which port
/// is which character. One listener means every message must say which session
/// it concerns, and that is true with one session open as much as with twenty.
///
/// MEASURED before adding it: `grep -rn "SessionId|session_id"
/// crates/cena-session/src/` returned **nothing**. There was no session
/// identity at all, because `main.rs` builds exactly one and never needs to
/// name it.
///
/// Deterministic by construction, like `Generation`: seeded at 0 and counted
/// up, never from a clock or a random source, so criterion 7's replay produces
/// the same ids on every run.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct SessionId(pub u32);

impl SessionId {
    /// The first session.
    ///
    /// A single-session process uses this and nothing else, which is why M4
    /// can add the id without M5's session manager existing.
    pub const FIRST: Self = Self(0);

    /// The next session's id.
    ///
    /// **Unused until Milestone 5** — nothing allocates a second session yet —
    /// but it is the one line that makes this a counter rather than a
    /// constant, which is the same justification `Generation::next` carries.
    #[must_use]
    pub const fn next(self) -> Self {
        Self(self.0 + 1)
    }
}

impl std::fmt::Display for SessionId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// Which connection a fact belongs to.
///
/// **Kept even though reconnect is Milestone 2.** It costs one `u32`; `plan/12`
/// §4.4 makes it the discard rule for late frames ("generation mismatch --
/// discarded, belongs to a previous connection"); and criterion 7's replay has
/// to produce the same generation on every run, which it does because the
/// counter is seeded at 0 rather than from a clock. Retrofitting an id onto
/// every `CommandId`, event and snapshot after they exist is the expensive
/// version of this decision.
///
/// **Distinct from [`SessionId`], and the pair is not redundant**: a session
/// keeps its id across every reconnect while its generation advances. `(id,
/// generation)` names one connection of one character — which is what a late
/// frame must be checked against.
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

/// A [`Generation`] every holder sees change at once.
///
/// # Why a handle cannot just hold a `Generation`
///
/// [`SessionHandle`](crate::SessionHandle) stamps its generation into every
/// command it sends, and the actor discards anything from a prior one
/// (`plan/12` §4.4). While nothing reconnected, a copied value was fine.
///
/// **It stops being fine the moment a second connection exists.** A handle
/// cloned in generation 0 -- by a frontend, or by a behavior -- would keep
/// stamping `0` forever, and after one reconnect every command it sent would be
/// answered [`Outcome::Interrupted`](crate::Outcome::Interrupted) for the rest
/// of the session. The session would reconnect successfully and then be deaf to
/// every caller that existed before the drop.
///
/// So a handle **reads** the generation instead of carrying a copy of it.
///
/// # The subtlety that keeps §4.4's discard rule intact
///
/// Making the handle durable must not make the discard rule toothless, and it
/// does not, because **the stamp happens at send time**:
///
/// * a handle held *across* a reconnect reads the new generation on its next
///   send, so it keeps working;
/// * a command already **in flight** when the connection died was stamped
///   before the bump, still carries the old value, and is still discarded.
///
/// Those are exactly the two properties wanted, and they come from *when* the
/// read happens rather than from any extra bookkeeping. The rejected
/// alternative -- dropping generation from the handle and letting the actor
/// stamp on admit -- would destroy the rule: a command queued before the
/// disconnect would go out on the new connection as though it were fresh.
///
/// # Why `AtomicU32` and not `watch::Sender<Generation>`
///
/// Nothing needs to be *woken* by a change. The actor reads its own generation
/// once when it is built, and a handle reads the cell at stamp time; a `watch`
/// would add a channel whose only subscriber polls it (Rule -1).
#[derive(Clone, Debug)]
pub struct GenerationCell(std::sync::Arc<std::sync::atomic::AtomicU32>);

impl GenerationCell {
    /// A cell at [`Generation::FIRST`].
    #[must_use]
    pub fn first() -> Self {
        Self(std::sync::Arc::new(std::sync::atomic::AtomicU32::new(
            Generation::FIRST.0,
        )))
    }

    /// The generation **now**.
    #[must_use]
    pub fn get(&self) -> Generation {
        Generation(self.0.load(std::sync::atomic::Ordering::Relaxed))
    }

    /// Advance to the next connection, returning it.
    ///
    /// Called by a supervisor **between** one actor ending and the next
    /// beginning -- never while an actor is running, which is why `Relaxed` is
    /// sufficient: there is no concurrent reader to order against. The actor
    /// that ended has already returned, and the one that will read the new
    /// value has not been built.
    ///
    /// Deliberately **not** `#[must_use]`: the point of the call is the side
    /// effect -- every handle now reads the new value -- and a supervisor that
    /// only wants to bump the counter should not have to bind or discard a
    /// return. It returns the new generation for the caller that does want it,
    /// which is a convenience rather than the result.
    #[allow(
        clippy::must_use_candidate,
        reason = "the side effect is the point; the return is a convenience"
    )]
    pub fn advance(&self) -> Generation {
        let next = self.get().next();
        self.0.store(next.0, std::sync::atomic::Ordering::Relaxed);
        next
    }
}

impl Default for GenerationCell {
    fn default() -> Self {
        Self::first()
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
    /// The transport is gone and another is being opened.
    ///
    /// `plan/12` §5.1: *"no transport. All in-flight commands fail immediately
    /// with `Disconnected`. **No automation runs.**"*
    ///
    /// Both halves are enforced without new code:
    ///
    /// * **No automation** -- [`Self::behaviors_may_run`] is
    ///   `matches!(self, Self::Ready)`, so a behavior's command is refused here
    ///   by the gate that already existed.
    /// * **`Disconnected`** -- the actor answers its waiters on the way out
    ///   (`SessionActor::shutdown`), so by the time a session is in this state
    ///   nobody is still waiting on the old connection.
    ///
    /// A session in this state has **no actor**: the previous one returned its
    /// [`SessionEnd`](crate::actor::SessionEnd) and the next has not been
    /// built. That is why the supervisor publishes the transition rather than
    /// the actor doing it -- there is no actor to.
    Reconnecting,
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
