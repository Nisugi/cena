//! The backoff ladder, and the two things that stop it.
//!
//! A supervisor that reconnects immediately and forever is worse than one that
//! does not reconnect at all: it converts one dropped packet into a login
//! storm. This module is what makes reconnect *bounded*, which is the author's
//! decision 2 ("automatic, bounded") expressed as code.
//!
//! # Ported from `VellumFE`, which learned both bounds the hard way
//!
//! `reference/VellumFE/src/frontend/headless/runtime.rs:31` -- the ladder
//! itself, `[1, 2, 5, 10, 30]` seconds, capped at the last entry, ±20% jitter.
//!
//! **The jitter is not cosmetic for a multi-session client.** Cena runs 3-25
//! characters in one process (`plan/12`). One network blip drops all of them at
//! once, and without jitter all of them re-login in lockstep -- the same
//! thundering herd, aimed at an auth server that Vellum's own comment warns
//! about locking accounts on.
//!
//! # Two stops, and they are different
//!
//! | Stop | Asks | Source |
//! |---|---|---|
//! | [`Retryability::Fatal`] | *can* this ever work? | the connector |
//! | [`MAX_UNATTENDED_LOSSES`] | *should* it keep trying? | the supervisor |
//!
//! Conflating them would be a real bug in both directions: a fatal auth error
//! retried twice is two more chances to lock an account, and an idle-kick
//! treated as fatal would stop a session that a keypress should revive.

use std::time::Duration;

/// Seconds between attempts, capped at the last entry.
///
/// Ported verbatim from `VellumFE`
/// (`reference/VellumFE/src/frontend/headless/runtime.rs:31`) rather than
/// re-derived. It is a shipped, lived-with schedule against *these* servers,
/// which is exactly the kind of knowledge `plan/13` §4a says to port rather
/// than reinvent.
const BACKOFF_SECONDS: &[u64] = &[1, 2, 5, 10, 30];

/// Consecutive losses with **no command sent in between** before the supervisor
/// stops on its own.
///
/// `VellumFE` (`runtime.rs:33-37`), whose comment is the whole justification:
///
/// > *"Guards the abandoned-phone case: the game idle-kicks after ~30 minutes,
/// > and without this cap the supervisor would re-login all night (battery +
/// > pointless auth churn)."*
///
/// **Two, not one.** One would stop the first time a session was quiet across a
/// single drop, which is an ordinary network blip on an idle character rather
/// than evidence of an abandoned client.
pub const MAX_UNATTENDED_LOSSES: u32 = 2;

/// Whether a failed connection is worth another attempt.
///
/// **The connector decides this, not the supervisor**, and that split is the
/// point. Only the connector can tell `PROBLEM: bad password` from a TCP reset,
/// and `cena-session` must not learn the login protocol to find out -- the same
/// argument [`Connector`](super::Connector)'s header already makes for why the
/// live implementor lives in the binary.
///
/// # The two failures `VellumFE` records, in the two directions
///
/// > *"The headless reconnect supervisor stops retrying when it finds this in
/// > an error chain -- hammering the auth server with a wrong password would be
/// > pointless and **could lock the account**."*
/// > (`reference/VellumFE/src/network.rs`, on `AuthFailed`)
///
/// > *"EOF: ... a transient DROP, not a credential rejection -- it must NOT
/// > surface as `AuthFailed`, or the ... supervisor treats it as 'bad
/// > credentials, stop retrying' and strands the session."*
///
/// Those are the same mistake made twice, once each way, which is why this is
/// an explicit two-variant decision a connector has to make rather than
/// something inferred from an error string here.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Retryability {
    /// Another attempt may succeed: a refused socket, a reset, a timeout, a
    /// server that dropped the connection mid-handshake.
    ///
    /// **The default**, because the failure that must never be guessed is the
    /// other one. Defaulting to `Fatal` would strand a session on any error a
    /// connector had not thought to classify; defaulting to `Transient` costs
    /// at most a bounded ladder of retries.
    #[default]
    Transient,
    /// No number of attempts will help: rejected credentials, an account
    /// problem, a protocol the client cannot speak.
    ///
    /// The supervisor stops **immediately** -- not after the ladder, not after
    /// one more try.
    Fatal,
}

impl Retryability {
    /// Whether the supervisor should try again.
    #[must_use]
    pub const fn may_retry(self) -> bool {
        matches!(self, Self::Transient)
    }
}

/// How long to wait before attempt `attempt`, where 0 is the first retry.
///
/// `jitter` is a fraction in `0.0..=1.0` mapped onto ±20%, so 0.5 is exactly
/// the base delay. It is a **parameter rather than a call into a random
/// source** for one reason: this workspace's tests replay, and criterion 7
/// requires a replay to produce the same result on every run. A function that
/// reached for OS randomness could not be asserted on at all, so the ladder
/// would be tested only through its effects.
///
/// `VellumFE` takes the byte from `getrandom` inside the function
/// (`runtime.rs:72-80`); Cena hoists it to the caller so the schedule is a pure
/// function and the randomness is one line in the supervisor.
#[must_use]
pub fn backoff(attempt: u32, jitter: f64) -> Duration {
    let index = (attempt as usize).min(BACKOFF_SECONDS.len() - 1);
    // Every entry is under 64, so `f64`'s 52-bit mantissa is not remotely
    // stretched. Written as a `from` conversion of a narrowed value rather than
    // an `as` cast so that a future entry too large to represent exactly would
    // have to be an explicit decision.
    #[expect(
        clippy::cast_possible_truncation,
        reason = "every BACKOFF_SECONDS entry is far below u32::MAX; asserted \
                  by the ladder tests, which read every entry back"
    )]
    let base = f64::from(BACKOFF_SECONDS[index] as u32) * 1000.0;
    // 0.8 ..= 1.2 -- Vellum's ±20%, same arithmetic.
    let scale = 0.8 + jitter.clamp(0.0, 1.0) * 0.4;
    #[expect(
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        reason = "base is at most 30_000 and scale at most 1.2, so the product \
                  is at most 36_000 -- no truncation and no negative is reachable"
    )]
    Duration::from_millis((base * scale) as u64)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The ladder climbs and then holds. Asserted at the **midpoint jitter**,
    /// where the delay is exactly the base, so this tests the schedule rather
    /// than the jitter.
    #[test]
    fn the_ladder_climbs_then_caps() {
        let seconds = |attempt| backoff(attempt, 0.5).as_secs_f64();
        assert!((seconds(0) - 1.0).abs() < f64::EPSILON);
        assert!((seconds(1) - 2.0).abs() < f64::EPSILON);
        assert!((seconds(2) - 5.0).abs() < f64::EPSILON);
        assert!((seconds(3) - 10.0).abs() < f64::EPSILON);
        assert!((seconds(4) - 30.0).abs() < f64::EPSILON);
        assert!(
            (seconds(99) - 30.0).abs() < f64::EPSILON,
            "capped at the last entry, NOT indexed out of bounds -- the cap is \
             what makes an unbounded attempt counter safe to keep incrementing"
        );
    }

    /// ±20%, and the bounds are the bounds.
    #[test]
    fn jitter_spans_exactly_twenty_percent_each_way() {
        assert_eq!(backoff(4, 0.0), Duration::from_secs(24), "-20%");
        assert_eq!(backoff(4, 1.0), Duration::from_secs(36), "+20%");
    }

    /// A jitter outside `0.0..=1.0` must not escape the band.
    ///
    /// The caller derives it from a byte, so it cannot today -- but this is the
    /// kind of input that gets a new caller later, and a delay of 30 seconds
    /// times an unclamped multiplier is a session that never comes back.
    #[test]
    fn a_jitter_out_of_range_is_clamped_not_extrapolated() {
        assert_eq!(backoff(0, -5.0), backoff(0, 0.0));
        assert_eq!(backoff(0, 5.0), backoff(0, 1.0));
    }

    /// The default is the direction that fails safe.
    #[test]
    fn unclassified_failures_are_transient() {
        assert_eq!(Retryability::default(), Retryability::Transient);
        assert!(Retryability::default().may_retry());
        assert!(!Retryability::Fatal.may_retry());
    }
}
