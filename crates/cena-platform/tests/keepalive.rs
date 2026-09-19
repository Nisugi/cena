//! TCP keepalive is accepted by this platform's socket API.
//!
//! # What this can and cannot prove
//!
//! It **cannot** prove that a half-open connection is detected -- that needs a
//! network to sever, and `live.rs` is BUILT, NOT RUN by any test in this
//! workspace (`CLAUDE.md`: do not log into a live game service).
//!
//! It also **cannot** verify the idle and interval: socket2 0.6.5 has a getter
//! for `SO_KEEPALIVE` and none for the timings (MEASURED 2026-09-19). What
//! guards those instead is that the constants are IMPORTED from `live.rs`
//! rather than copied, and pinned by `the_parameters_are_the_ones_lich_uses`.
//!
//! What it proves is the half that silently fails: that `set_tcp_keepalive`
//! with Lich's parameters is **accepted and leaves the option on** on the
//! platform this is built for. `LiveSource` ignores the error deliberately, so an option
//! that was silently rejected -- wrong feature flags, an unsupported field on
//! Windows -- would look exactly like one that worked, right up until an
//! airplane-mode test months later reported nothing again.
//!
//! Uses a loopback socket. No game, no network, nothing external.

/// **The values `LiveSource` actually sets**, imported rather than copied.
///
/// They used to be re-declared here as two local `Duration`s. That is review
/// finding PL-6: the test then asserted its own copy, so changing
/// `KEEPALIVE_IDLE` in `live.rs` -- or deleting the `with_time` call entirely
/// -- left it green. A test of a constant must read the constant.
use cena_platform::{KEEPALIVE_IDLE as IDLE, KEEPALIVE_INTERVAL as INTERVAL};
use std::time::Duration;

#[test]
fn keepalive_with_lichs_parameters_is_accepted_and_switched_on() {
    let listener = std::net::TcpListener::bind("127.0.0.1:0").expect("bind loopback");
    let addr = listener.local_addr().expect("local addr");
    let stream = std::net::TcpStream::connect(addr).expect("connect loopback");

    let params = socket2::TcpKeepalive::new()
        .with_time(IDLE)
        .with_interval(INTERVAL);
    let sock = socket2::SockRef::from(&stream);

    sock.set_tcp_keepalive(&params).expect(
        "set_tcp_keepalive must be accepted -- LiveSource ignores this error, so a \
                 rejection here would be invisible in production and the half-open socket \
                 case would stay undetectable",
    );

    assert!(
        sock.keepalive().expect("keepalive must be readable"),
        "and it must actually be ON afterwards, not merely accepted"
    );

    // **The timings cannot be read back, and that is a real limit.** PL-6 asked
    // for the idle and interval to be verified, not just the on/off bit --
    // rightly, since the default idle is 2 HOURS on most systems and a
    // `with_time` silently dropped from `set_keepalive` would leave the socket
    // useless for detecting a half-open game connection.
    //
    // MEASURED 2026-09-19: socket2 0.6.5 exposes `keepalive()` and no getter
    // for the time or interval on any platform (`grep 'pub fn keepalive'` over
    // `socket2-0.6.5/src/socket.rs` returns exactly one). So the read-back
    // asked for is not available, and writing one that "checks" the timings
    // would be the very thing this finding is about.
    //
    // What IS enforced instead: the constants are imported from `live.rs`
    // rather than copied, and `the_parameters_are_the_ones_lich_uses` pins
    // their values. A drift in `live.rs` fails there.
}

/// The parameters are Lich's, and drifting from them is a decision, not an
/// accident.
///
/// `reference/lich-5/lib/games.rb:458` sets `idle: 30, interval: 30`. Asserted
/// against the imported constants so the pair cannot change silently -- this is
/// the check that makes importing them worthwhile rather than decorative.
#[test]
fn the_parameters_are_the_ones_lich_uses() {
    assert_eq!(IDLE, Duration::from_secs(30), "Lich's games.rb:458 idle");
    assert_eq!(
        INTERVAL,
        Duration::from_secs(30),
        "Lich's games.rb:458 interval"
    );
}
