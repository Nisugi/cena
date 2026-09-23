//! The game socket's options are set BY THE CODE THAT OPENS IT.
//!
//! # What this can and cannot prove
//!
//! It **cannot** prove that a half-open connection is detected -- that needs a
//! network to sever, and `live.rs` is BUILT, NOT RUN against a real host by any
//! test in this workspace (`CLAUDE.md`: do not log into a live game service).
//!
//! What it proves is the half that silently fails: that
//! [`LiveSource::connect`] -- the function production calls -- leaves the
//! options on, with the values `live.rs` declares. It connects to a LOOPBACK
//! listener in this process and reads the options back off the socket it was
//! handed. No game, no network, nothing leaves the machine.
//!
//! # This used to test its own copy (review finding 6)
//!
//! The test here built its OWN `TcpKeepalive` from the imported constants,
//! applied it to its own socket, and read that back. It proved socket2 works.
//! Deleting the `set_keepalive(&stream)` call from `LiveSource::connect` left
//! it green, which is the only regression it existed to catch. Its header also
//! said `LiveSource` "ignores the error deliberately" -- true once, and stale
//! since PL-9 made the failure a warning.
//!
//! # Which values can be read back, and where
//!
//! The on/off bit everywhere. The idle time and interval only where socket2
//! 0.6.5 has a getter, which is not Windows (`tcp_keepalive_time` is
//! `cfg(not(windows, ...))` in its `socket.rs`). The send timeout only on
//! Linux and Android, where `live.rs` sets it at all. So on the author's
//! Windows machine this checks the bit and nodelay, and CI's Linux runner
//! checks every value. An assertion under a `cfg` is still an assertion, on the
//! platform it can be made on -- and it is not written for the platform where it
//! could not, which is the failure `PL-6` was about.

use cena_platform::{KEEPALIVE_IDLE, KEEPALIVE_INTERVAL, LiveSource};
use std::time::Duration;

/// Connect through the production path to a loopback listener, and hand back
/// the plain TCP stream it made.
///
/// Returns a `Result` rather than panicking: this is not a `#[test]`, so the
/// workspace's `expect_used` ban applies to it (`clippy.toml` exempts test
/// functions only).
async fn connected() -> std::io::Result<(tokio::net::TcpStream, std::net::TcpListener)> {
    // std's listener, not tokio's: nothing here needs to accept, and the
    // kernel completes the handshake from the backlog either way. Returned so
    // it outlives the connection.
    let listener = std::net::TcpListener::bind("127.0.0.1:0")?;
    let port = listener.local_addr()?.port();
    match LiveSource::connect("127.0.0.1", port).await? {
        LiveSource::Plain(stream) => Ok((stream, listener)),
        other => Err(std::io::Error::other(format!(
            "LiveSource::connect must return the plain game stream, got {other:?}"
        ))),
    }
}

#[tokio::test]
async fn the_game_socket_leaves_connect_with_keepalive_on() {
    let (stream, _listener) = connected().await.expect("loopback connect");
    let sock = socket2::SockRef::from(&stream);

    assert!(
        sock.keepalive().expect("keepalive must be readable"),
        "LiveSource::connect returned a socket with keepalive OFF. A severed \
         connection -- airplane mode, a dropped VPN -- then never fails a \
         read, and the session sits in a quiet game forever (live.rs, \
         MEASURED by the author 2026-09-18)."
    );
    assert!(
        stream.nodelay().expect("nodelay must be readable"),
        "Nagle is on: every command waits up to 200ms for company"
    );

    #[cfg(any(target_os = "linux", target_os = "android", target_os = "macos"))]
    {
        assert_eq!(
            sock.tcp_keepalive_time().expect("idle is readable here"),
            KEEPALIVE_IDLE,
            "the idle is the OS default -- two HOURS on most systems -- \
             rather than Lich's 30s"
        );
        assert_eq!(
            sock.tcp_keepalive_interval()
                .expect("interval is readable here"),
            KEEPALIVE_INTERVAL
        );
    }
}

#[cfg(any(target_os = "linux", target_os = "android"))]
#[tokio::test]
async fn the_game_socket_leaves_connect_with_a_send_timeout() {
    // Finding 7. Keepalive is silent once a command is in flight, because the
    // connection is not idle; this is what fails THAT write, in Lich's 120s
    // rather than the kernel's ~15 minutes.
    let (stream, _listener) = connected().await.expect("loopback connect");
    let sock = socket2::SockRef::from(&stream);
    assert_eq!(
        sock.tcp_user_timeout()
            .expect("TCP_USER_TIMEOUT is readable"),
        Some(cena_platform::UNACKED_SEND_TIMEOUT)
    );
}

/// The parameters are Lich's, and drifting from them is a decision, not an
/// accident.
///
/// Asserted against the imported constants so they cannot change silently --
/// the test above proves the socket CARRIES them; this proves they are the
/// values Lich ships.
#[test]
fn the_parameters_are_the_ones_lich_uses() {
    assert_eq!(
        KEEPALIVE_IDLE,
        Duration::from_secs(30),
        "Lich's games.rb:458 idle"
    );
    assert_eq!(
        KEEPALIVE_INTERVAL,
        Duration::from_secs(30),
        "Lich's games.rb:458 interval"
    );
    assert_eq!(
        cena_platform::UNACKED_SEND_TIMEOUT,
        Duration::from_mins(2),
        "Lich's socketconfigurator.rb:289-292, TCP_USER_TIMEOUT 120000 ms"
    );
}
