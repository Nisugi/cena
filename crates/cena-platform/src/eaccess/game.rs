//! The game socket: the second connection, and the handshake that makes it
//! usable.
//!
//! **This is not the eaccess socket.** [`super::handshake`] finishes with a
//! host, a port and a one-shot key over TLS; this opens a *plain* TCP
//! connection to that host and hands the key over in the clear. Split from
//! `handshake.rs` under `plan/05` Rule 4.1 when that file passed the 400-line
//! cap -- the seam was already there, since nothing below speaks the
//! tab-delimited `EAccess` protocol at all.

use super::wire::{CLIENT_BANNER, EaccessError, LaunchPayload, err};
use crate::bytes::ByteSource;
use crate::gemstone::endpoint::other_spelling;
use crate::live::LiveSource;

/// Open the game socket and send the three-part handshake.
///
/// This is the second connection: **plain TCP, not TLS**, to the host and port
/// the [`LaunchPayload`] names. The eaccess socket is already closed by the
/// time this is called.
///
/// The handshake is key, then banner, then two `<c>` ready signals:
///
/// 1. `{key}\n` -- the one-shot key from `L`.
/// 2. [`CLIENT_BANNER`] -- which selects the **extended feed**. See that
///    constant: it is not cosmetic.
/// 3. Two `<c>` signals, ~300ms apart.
///
/// # The `<c>` signals, and how they were settled
///
/// `VellumFE` sends them unconditionally with no `DragonRealms` branch
/// (`reference/VellumFE/src/network.rs:689-694`: "game server expects two
/// `<c>` signals with delay"). They were once removed from the spike on
/// Saga-research reasoning -- "`GemStone` sends no `<c>`" -- which was **wrong**,
/// and the removal was never run.
///
/// **They are not taken on Vellum's word either.** Believing a working
/// implementation uncritically is the same error as believing the inference,
/// pointed the other way. An A/B run against the live server on 2026-09-18
/// settled it:
///
/// | run | result |
/// |---|---|
/// | with the signals | **2,347 bytes** -- full login burst: room, exits, inventory, `exposeContainer` |
/// | without | **163 bytes** -- stops at `<settingsInfo>` |
///
/// So they are **not required to log in, and required to be usable.** Both
/// runs "connect"; only one produces a session worth having. That is the
/// distinction `plan/10` §10.3a's design lesson names -- *"connected is not
/// working"* -- and it is why the pass criterion is game text, not a TCP
/// accept.
///
/// # The endpoint fallback
///
/// The connect is tried twice when the endpoint has a known second spelling:
/// see [`connect_with_fallback`], and [`endpoint`](crate::gemstone::endpoint) for why the
/// table is a fallback rather than the pre-emptive rewrite Lich does.
///
/// # Errors
///
/// [`EaccessError`] with stage `game_connect` or `game_handshake`.
pub async fn connect_game(payload: &LaunchPayload) -> Result<LiveSource, EaccessError> {
    let mut sock = connect_with_fallback(&payload.gamehost, payload.gameport)
        .await
        .map_err(|e| err("game_connect", e))?;

    // One write per message throughout -- `ByteSource::write_all`'s contract.
    let mut key_line = Vec::with_capacity(payload.key.len() + 1);
    key_line.extend_from_slice(payload.key.as_bytes());
    key_line.push(b'\n');
    sock.write_all(&key_line)
        .await
        .map_err(|e| err("game_handshake", e))?;

    let mut banner = Vec::with_capacity(CLIENT_BANNER.len() + 1);
    banner.extend_from_slice(CLIENT_BANNER.as_bytes());
    banner.push(b'\n');
    sock.write_all(&banner)
        .await
        .map_err(|e| err("game_handshake", e))?;

    for _ in 0..2 {
        sock.write_all(b"<c>\n")
            .await
            .map_err(|e| err("game_handshake", e))?;
        tokio::time::sleep(READY_SIGNAL_GAP).await;
    }

    Ok(sock)
}

/// The gap between the two `<c>` ready signals.
///
/// 300ms, from `VellumFE` (`network.rs:689-694`). Whether the *gap* matters, as
/// against the two signals existing at all, is **UNVERIFIED** -- the live A/B
/// varied their presence, not their spacing. It is `tokio::time::sleep`, so it
/// is virtual under `tokio::time::pause()` and costs a test nothing.
const READY_SIGNAL_GAP: std::time::Duration = std::time::Duration::from_millis(300);

/// Connect to the game, retrying once against the endpoint's other spelling.
///
/// Ported from Lich's retry arm (`lib/main/main.rb:583-598`), which catches a
/// connect failure, applies the inverse host/port mapping, and tries once more
/// before giving up.
///
/// # Why the retry is scoped to the connect and no further
///
/// A connect failure is the only failure the other spelling could fix. Once
/// bytes have been exchanged, a failure means something about *this* session --
/// a rejected key, a closed socket mid-handshake -- and retrying against a
/// different host would send the one-shot key to a second server. The key is
/// single-use (`plan/10` §4.6), so a handshake retry would burn it.
///
/// # Which error is reported when both fail
///
/// **The first one.** `L` named the first host, so its failure is the answer to
/// "why could I not reach the server the login told me to use". The fallback's
/// failure is a footnote: it is expected to fail whenever the table is simply
/// not relevant to this endpoint.
async fn connect_with_fallback(host: &str, port: u16) -> std::io::Result<LiveSource> {
    let first = match LiveSource::connect(host, port).await {
        Ok(sock) => return Ok(sock),
        Err(e) => e,
    };

    // No counterpart is the common case -- an endpoint the table does not know
    // -- and it is not an error. There is simply nothing else to try.
    let Some((alt_host, alt_port)) = other_spelling(host, port) else {
        return Err(first);
    };

    LiveSource::connect(alt_host, alt_port)
        .await
        .map_err(|_alt| first)
}
