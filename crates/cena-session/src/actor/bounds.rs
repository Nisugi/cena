//! The actor's two channel bounds: commands in, events out.
//!
//! Moved down out of `actor.rs` when publishing each finished line doubled
//! the event ring and its reasoning pushed that file past its cap
//! (`crates/cena-arch-tests/caps.baseline`): the constants and their
//! measurements are here, and `actor.rs` re-exports them.

/// Inbound command channel bound.
///
/// `plan/12` §5.5: bounded channels everywhere. 32 is a starting value, not a
/// measured one -- `plan/12` §10 defers buffer sizes to "measurement under
/// real load". What matters structurally is that it is *bounded*: a full
/// queue refuses (`Outcome::Refused(Transient)`) rather than blocking the
/// caller, which is what stops a slow session wedging a frontend.
///
/// **One definition for both entry points.** [`Session`] and the supervisor
/// each held a copy, commented "matches the actor's" -- a promise nothing
/// checked (review finding 10).
pub(crate) const COMMAND_CHANNEL_BOUND: usize = 32;

/// Event broadcast ring size.
///
/// `plan/12` §6.3: "Bounded ring buffer per subscriber
/// (`tokio::sync::broadcast` semantics). On overflow the subscriber receives
/// an explicit `Lagged { missed }`, never a silent gap." That is what
/// `broadcast` does, so §6.3 costs one constant rather than a mechanism.
///
/// # Why 2,048, and why raising it is not the whole answer
///
/// `plan/18` §6 declined to simply raise this, asking instead *"whether a
/// subscriber that needs EVERY frame should be a broadcast subscriber at all, or
/// whether the model is the only thing that must not miss frames."*
///
/// **The model already is.** `SessionActor::ingest` calls `state.apply(&frame)`
/// and *then* `events.send(...)`, on the same thread, so `GameState` cannot lag
/// however small this is. The ring is for OBSERVERS, and for an observer
/// `Lagged` is the honest answer rather than a failure.
///
/// So what remained was sizing. MEASURED, frames before the first `<prompt>` in
/// two of the author's captures: **1,151** (`GSIV-Nisugi/2025-04-18`) and **794**
/// (`GSIV-Monstr/2025-09-04`). Against 256 -- so the burst ran 3-4.5x the ring,
/// and the `!! 99 events dropped` the author saw was the tail of a much larger
/// overflow. 2,048 covered the larger burst with ~78% headroom.
///
/// **4,096 since each finished line is published too** (`Event::Line`,
/// `plan/45` §4a). A line needs a text frame to finish it, so a burst is at most
/// frames + lines = 2 x 1,151 = 2,302 events, the whole burst being text; 4,096
/// is the same ~78% headroom over that. The committed login fixtures are cut
/// too small to measure a real burst's line count, so this is sized to the
/// bound, not a measurement of lines. The cost is the slot memory of two rings
/// (the legacy stream and the fenced one) per session, doubled.
///
/// It is a size, not a promise: a slow enough subscriber still lags, and
/// `crates/cena-session/tests/event_ring.rs` asserts that it is still told.
///
/// # And a subscriber that lagged can recover
///
/// `plan/12` §6.3's recovery is to drop the lagged receiver and take a fresh
/// snapshot plus stream. `plan/19` (SE-6) recorded that as unreachable,
/// because `subscribe` lived on the owner and `run` consumed it. It is
/// reachable now: [`SessionObserver`](crate::SessionObserver) is obtained
/// before `run` and outlives it, and `subscribe` on it returns a fresh
/// fenced pair at any point in the session's life, reconnects included.
/// `tests/observation.rs`'s
/// `lag_resubscription_replaces_the_old_fence_with_fresh_authoritative_state`
/// exercises exactly that path.
pub(crate) const EVENT_CHANNEL_BOUND: usize = 4096;
