//! TCP keepalive is accepted by this platform's socket API.
//!
//! # What this can and cannot prove
//!
//! It **cannot** prove that a half-open connection is detected -- that needs a
//! network to sever, and `live.rs` is BUILT, NOT RUN by any test in this
//! workspace (`CLAUDE.md`: do not log into a live game service).
//!
//! What it proves is the half that silently fails: that `set_tcp_keepalive`
//! with Lich's parameters is **accepted and reads back** on the platform this
//! is built for. `LiveSource` ignores the error deliberately, so an option
//! that was silently rejected -- wrong feature flags, an unsupported field on
//! Windows -- would look exactly like one that worked, right up until an
//! airplane-mode test months later reported nothing again.
//!
//! Uses a loopback socket. No game, no network, nothing external.

use std::time::Duration;

/// The same pair `LiveSource` sets, from `reference/lich-5/lib/games.rb:458`.
const IDLE: Duration = Duration::from_secs(30);
const INTERVAL: Duration = Duration::from_secs(30);

#[test]
fn keepalive_with_lichs_parameters_is_accepted_and_reads_back() {
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
}
