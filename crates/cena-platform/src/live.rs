//! [`LiveSource`]: the TCP+TLS [`ByteSource`]. Criterion 1's transport.
//!
//! # BUILT, NOT RUN
//!
//! `CLAUDE.md` forbids logging into a live game service without the author
//! present, and `plan/12` §7.2 criterion 1 ("logs in against the live server")
//! is the one criterion Milestone 1 Step 2 does not exercise here. This module
//! is complete and compiles; **no test in this workspace calls
//! [`LiveSource::connect`] against a real host**, and none may. The author
//! runs it. One test calls it against a LOOPBACK listener in its own process
//! (`tests/keepalive.rs`), to read back the socket options it sets -- nothing
//! leaves the machine.
//!
//! # Three findings ported verbatim from the spike
//!
//! The spike (`spike/eaccess-spike/src/main.rs`) is 615 lines of blocking
//! `std::net::TcpStream` (`:23-24`) and its own header calls it throwaway. It
//! is **rewritten, not ported**, because `plan/12` §5.5 requires every await to
//! be cancellable and a blocking `read` is not: `set_read_timeout` (`:180`) is
//! a timeout, not a cancellation, and criterion 4's 250ms stop is unreachable
//! from inside one.
//!
//! But three things in it are knowledge that lives in code, which `CLAUDE.md`
//! says to port rather than rediscover:
//!
//! 1. **One write per message** (`:110-122`). Two `write_all` calls can emit
//!    two TLS records and this server does not tolerate a command split across
//!    them. Ruby's `IO#puts` is one write, so Lich never met this. Encoded in
//!    [`ByteSource::write_all`]'s contract and honoured below.
//! 2. **`native-tls`, not `rustls`** (`spike/eaccess-spike/Cargo.toml:8-11`).
//!    eaccess negotiates TLS 1.2 static-RSA key exchange (0x009c), which
//!    rustls refuses to implement. VERIFIED by packet capture 2026-09-18.
//! 3. **The game socket is not the eaccess socket.** The spike connects to
//!    eaccess over TLS for the login handshake, then opens a *plain* TCP
//!    connection to the game host and sends the key. This type is the second
//!    of those: the session's byte source is the game stream, which is not TLS.
//!    [`LiveSource::connect_tls`] exists for the eaccess half.
//!
//! The game stream has a second route since Lich PR #1664: the WebSocket shim
//! on 443 ([`LiveSource::connect_shim`], `shim.rs`), which IS TLS -- verified
//! TLS, unlike eaccess. It is the fallback when the game port is unreachable.

use crate::bytes::ByteSource;
use crate::gemstone::shim;
use std::io;
use std::time::Duration;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;

/// Idle time before the kernel sends its first keepalive probe.
///
/// Lich's value, taken verbatim (`reference/lich-5/lib/games.rb:458-463`:
/// `idle: 30, interval: 30`), with its own comment for why it is this low:
/// *"defensive against L3/L4 idle reapers"*.
pub const KEEPALIVE_IDLE: Duration = Duration::from_secs(30);

/// Time between probes once they start.
pub const KEEPALIVE_INTERVAL: Duration = Duration::from_secs(30);

/// How long sent data may sit unacknowledged before the kernel fails the
/// socket. `TCP_USER_TIMEOUT`, Linux and Android only -- see
/// `set_user_timeout` in this file for why it is only those two.
///
/// **Lich's value, which is 120s and not 10s.** The review that asked for this
/// said "~10s", reading `tcp_maxrt: 10` in `games.rb:473`. That is the
/// WINDOWS option (`TCP_MAXRT`, seconds). On Unix, Lich's
/// `SocketConfigurator.configure_unix` sets `TCP_USER_TIMEOUT` to a fixed
/// `120000` ms (`reference/lich-5/lib/common/socketconfigurator.rb:289-292`).
/// Taken verbatim, like the keepalive pair: a shipped value against these
/// servers, and 10s would drop a session over an ordinary mobile handover.
///
/// What it buys is the send-side twin of keepalive. Keepalive only probes an
/// IDLE connection; once a command has been written and not acknowledged, the
/// socket is not idle, keepalive stays silent, and Linux retransmits for
/// `tcp_retries2` = 15 rounds before the write errors -- `tcp(7)` puts the
/// default at a hypothetical 924.6 seconds, about 15 minutes. Not measured
/// here: no test in this workspace may sever a real connection.
pub const UNACKED_SEND_TIMEOUT: Duration = Duration::from_mins(2);

/// A live connection: plain TCP to the game, or TLS to eaccess.
///
/// Two shapes rather than two types because they differ only in whether a TLS
/// layer is wrapped around the same socket, and everything above them --
/// read, single-write, shutdown -- is identical. Two types would be a trait
/// with two implementors that never diverge, which is Rule -1's other half.
///
/// The third shape is the same game stream reached another way, when the game
/// port cannot be: see `gemstone/shim.rs`.
#[derive(Debug)]
pub enum LiveSource {
    /// The game stream. Plain TCP: the game host takes the key in the clear,
    /// which is what the spike does (`src/main.rs:341-358`).
    Plain(TcpStream),
    /// The eaccess handshake. TLS 1.2, static-RSA, `native-tls`.
    Tls(Box<tokio_native_tls::TlsStream<TcpStream>>),
    /// The game stream through play.net's WebSocket shim on 443, over
    /// verified TLS. The fallback when [`Self::Plain`] cannot connect.
    WebSocket(Box<shim::ShimStream>),
}

/// Turn on TCP keepalive, so a half-open socket eventually fails a read.
///
/// # The failure this exists for
///
/// **A connection that is severed rather than closed never returns `Ok(0)`.**
/// MEASURED by the author, 2026-09-18: putting the machine into airplane mode
/// mid-session produced *no* disconnect and *no* reconnect -- the session sat
/// in its 500ms read deadline indefinitely, treating "nothing arrived" as a
/// quiet game, which for a text MUD it usually is.
///
/// Keepalive is what makes that case distinguishable. The kernel sends empty
/// ACK probes on an idle connection; when enough go unanswered it fails the
/// socket, `read` returns an error, and [`EndReason::ReadFailed`] flows into
/// the reconnect ladder that already exists. **No new policy, no new timer,
/// and nothing sent to the game.**
///
/// # Why not an application-level ping
///
/// Because a command is the wrong layer. It would spend one of the account's
/// type-ahead slots (MEASURED at 2), appear in the log indistinguishably from
/// a real command, and risk roundtime -- while detecting nothing a kernel
/// probe does not. Lich reaches the same conclusion: it configures keepalive
/// and sends no ping.
///
/// # Best-effort, and it SAYS SO when it fails
///
/// A failure does not stop the session -- one that works without keepalive is
/// better than no session -- but it is **reported**, because the cost of the
/// option not applying is that airplane-mode-style disappearance goes back to
/// being undetectable, and that is the one thing this function exists to
/// prevent.
///
/// This used to swallow it with `let _`, citing Lich as precedent: "exactly as
/// Lich ignores its own". **Lich does not ignore it.** It rescues and logs
/// (`reference/lich-5/lib/games.rb`):
///
/// ```text
/// log_error("Socket configuration error (continuing with defaults)", e)
/// Lich.log("WARNING: Socket running with default OS settings - may be less
///           reliable under network stress")
/// ```
///
/// The citation was doing real work -- it justified the silence -- and it was
/// backwards (review PL-9). `eprintln!` rather than the session log because
/// this runs before a sink exists, and a silent degradation is the failure
/// being guarded against.
fn set_keepalive(stream: &TcpStream) {
    let params = socket2::TcpKeepalive::new()
        .with_time(KEEPALIVE_IDLE)
        .with_interval(KEEPALIVE_INTERVAL);
    // A borrowed view of the same socket -- it does not take ownership and
    // does not close the fd when dropped.
    if let Err(e) = socket2::SockRef::from(stream).set_tcp_keepalive(&params) {
        eprintln!(
            "[socket] WARNING: TCP keepalive could not be set ({e}). The \
             session continues on OS defaults, but a connection that \
             disappears without a FIN -- airplane mode, a dropped VPN -- may \
             now hang instead of failing, which is what this setting exists \
             to prevent."
        );
    }
}

/// Bound how long a SENT byte may go unacknowledged, where the platform lets
/// this crate say so.
///
/// Best-effort and warned on failure, exactly as [`set_keepalive`] is and for
/// the same reason: a session without it still works, but a write into a dead
/// link goes back to hanging for the OS default, and nobody would know why.
///
/// # Only Linux and Android, and why not Windows
///
/// Lich sets the equivalent on both families: `TCP_USER_TIMEOUT` on Unix and
/// `TCP_MAXRT` on Windows (`socketconfigurator.rb:289-292`, `:421-431`).
/// socket2 0.6.5 exposes the first and **not the second** -- `grep -rn MAXRT`
/// over its source returns nothing -- and setting it by hand is a raw
/// `setsockopt`, which is `unsafe`, which the workspace denies
/// (`Cargo.toml`, `unsafe_code = "deny"`). So Windows keeps its OS default.
/// That is a decision for the author, recorded rather than taken here: it
/// needs either an `unsafe` exception or a dependency that wraps the call.
///
/// macOS has `TCP_RXT_CONNDROPTIME`, which socket2 does not expose either, and
/// Lich does not set. Nothing is claimed for it.
///
/// Lich's other half of that block -- `SO_RCVTIMEO`/`SO_SNDTIMEO` at 30s -- is
/// deliberately NOT ported. Those bound a BLOCKING call; tokio's sockets are
/// non-blocking, where the options do not apply. The read side is already
/// bounded where it can be cancelled: the session's own read deadline.
#[cfg(any(target_os = "linux", target_os = "android"))]
fn set_user_timeout(stream: &TcpStream) {
    if let Err(e) = socket2::SockRef::from(stream).set_tcp_user_timeout(Some(UNACKED_SEND_TIMEOUT))
    {
        eprintln!(
            "[socket] WARNING: TCP_USER_TIMEOUT could not be set ({e}). The \
             session continues, but a command written into a dead link may \
             now wait out the kernel's ~15 minute retransmission default \
             before the session notices."
        );
    }
}

/// No `TCP_USER_TIMEOUT` here -- see the Linux version's docs for why.
#[cfg(not(any(target_os = "linux", target_os = "android")))]
fn set_user_timeout(_stream: &TcpStream) {}

/// Every option a GAME socket carries, in one place.
///
/// Shared by [`LiveSource::connect`] and [`LiveSource::connect_shim`] so the
/// two routes to the same stream cannot drift: they did not share this, and
/// each would have needed the send timeout added by hand. The eaccess socket
/// takes only `nodelay` -- see [`LiveSource::connect_tls`] for why.
fn configure_game_socket(stream: &TcpStream) -> io::Result<()> {
    // Nagle batches small writes, which is exactly wrong for a stream of
    // one-line commands: it would add up to 200ms to every round trip and
    // make criterion 4's PREEMPT_GRACE measurement a measurement of Nagle.
    stream.set_nodelay(true)?;
    set_keepalive(stream);
    set_user_timeout(stream);
    Ok(())
}

impl LiveSource {
    /// Open a plain TCP connection.
    ///
    /// `tests/keepalive.rs` calls this against a LOOPBACK listener and reads
    /// the options back off the socket it returns -- the only way to know
    /// they are set by the code that runs, rather than by a test's own copy.
    ///
    /// # Errors
    ///
    /// The underlying connect error, unchanged: a caller that cannot reach the
    /// host needs to know why, and wrapping it in a crate error type would be
    /// an abstraction with one caller.
    pub async fn connect(host: &str, port: u16) -> io::Result<Self> {
        let stream = connect_bounded(host, port).await?;
        configure_game_socket(&stream)?;
        Ok(Self::Plain(stream))
    }

    /// Open the game stream through the WebSocket shim, for the game host and
    /// port the login named.
    ///
    /// The socket goes to `shim_host`'s remap of `gamehost` on 443 and the game port
    /// rides in the upgrade path. Nagle, keepalive and the send timeout are
    /// set on the TCP socket underneath, for the same reasons as
    /// [`Self::connect`]: they are socket options, and the WebSocket above
    /// cannot set them.
    ///
    /// # Errors
    ///
    /// The TCP connect, TLS or upgrade failure.
    pub async fn connect_shim(gamehost: &str, gameport: u16) -> io::Result<Self> {
        let host = shim::shim_host(gamehost);
        let stream = connect_bounded(host, shim::SHIM_PORT).await?;
        configure_game_socket(&stream)?;
        let shim = shim::ShimStream::open(stream, host, gameport).await?;
        Ok(Self::WebSocket(Box::new(shim)))
    }

    /// Open a TLS connection for the eaccess handshake.
    ///
    /// # Three deliberate weakenings, and why each is required
    ///
    /// This connector is **not** `TlsConnector::new()`. That was what stood
    /// here until 2026-09-18, written from the spike's *shape* rather than
    /// from its *configuration* -- and it could not have worked. The method
    /// had **no caller**, so nothing exercised it until [`crate::eaccess`]
    /// did. Found by building the caller, not by reading the code.
    ///
    /// 1. **`use_sni(false)`** -- S1 (`plan/10` §12.1). Lich's `ClientHello`
    ///    carries no `server_name` extension (VERIFIED by packet capture:
    ///    extension type 0 absent), and eaccess.play.net sits behind an AWS
    ///    load balancer that may route on it. We match Lich rather than find
    ///    out the hard way.
    /// 2. **`danger_accept_invalid_certs(true)`** -- the server's certificate
    ///    is **self-signed with no chain of trust** (`plan/10` §2.3), so
    ///    ordinary verification cannot succeed against it. Lich does not
    ///    verify either; it pins (`eaccess.rb:90-130`).
    /// 3. **`danger_accept_invalid_hostnames(true)`** -- follows from 1 and 2.
    ///    With no SNI and no chain, there is no name to check against.
    ///
    /// # What this does NOT do, and what it costs
    ///
    /// **It does not pin, so this handshake is MITM-able, and the account
    /// password crosses it.**
    ///
    /// `plan/10` §2.3 finds Lich's own model is trust-on-first-use with
    /// *silent auto-re-pin* -- "an attacker who MITMs one connection installs
    /// a persistent pin" -- and §9.2 says Cena should compare a **SHA-256 of
    /// the DER**, not PEM text (PEM equality is line-ending sensitive,
    /// `plan/10` §12.3). None of that is built here.
    ///
    /// That is a deliberate M1 scope call rather than an oversight:
    /// `plan/12` §7.1 puts saved credentials and the login ladder Out, and a
    /// pin with nowhere to be stored is half a mechanism. It is recorded on
    /// this line, not in a backlog, because the weakening is *here* and a
    /// reader of it must see the cost. **It is the one thing in this module
    /// that should not survive to a release build.**
    ///
    /// ## That sentence is now enforced, not just written
    ///
    /// It was a note and nothing checked it (review PL-10), which is `plan/05`
    /// Rule 0's definition of a wish. Two things hold it now:
    ///
    /// * A **release build says so at runtime**, every time this runs. Not a
    ///   `compile_error!`: pinning does not exist yet, so refusing to build
    ///   would only force the guard to be deleted, and a deleted guard is
    ///   worse than a loud one. `debug_assertions` is the discriminator --
    ///   it is off in `--release` and on in the dev profile the author runs.
    /// * `cena-arch-tests` asserts the warning is still here, so removing it
    ///   fails the suite rather than quietly restoring the silence.
    ///
    /// # Errors
    ///
    /// Connect, TLS-builder and handshake errors, each mapped to
    /// [`io::Error`] so one `?` chain covers the sequence.
    pub async fn connect_tls(host: &str, port: u16) -> io::Result<Self> {
        let stream = connect_bounded(host, port).await?;
        stream.set_nodelay(true)?;
        // NO keepalive here, unlike the game socket. This connection is a
        // handshake -- a few request/response pairs that finish in seconds,
        // each already bounded by `eaccess`'s per-stage deadline. Keepalive
        // guards a connection that sits IDLE for minutes, which this one never
        // does. Stated because its absence beside `connect`'s presence would
        // otherwise read as an oversight.
        // ARCH-TEST ANCHOR: `release_builds_announce_the_unpinned_tls`.
        if !cfg!(debug_assertions) {
            eprintln!(
                "[tls] WARNING: this is a RELEASE build and the eaccess TLS \
                 handshake is UNPINNED -- certificates and hostnames are not \
                 verified, and the account password crosses this connection. \
                 plan/10 section 9.2 specifies a SHA-256-of-DER pin; it is \
                 not built. Do not ship this."
            );
        }
        let connector = native_tls::TlsConnector::builder()
            .danger_accept_invalid_certs(true)
            .danger_accept_invalid_hostnames(true)
            .use_sni(false)
            .build()
            .map_err(|e| io::Error::other(format!("tls connector: {e}")))?;
        let connector = tokio_native_tls::TlsConnector::from(connector);
        // The TLS handshake is bounded too. `plan/10` §2.1: Lich's
        // CONNECT_TIMEOUT "covers only the TCP handshake; the TLS handshake
        // and every protocol read are unbounded blocking calls."
        let tls = tokio::time::timeout(TLS_HANDSHAKE_TIMEOUT, connector.connect(host, stream))
            .await
            .map_err(|_elapsed| {
                io::Error::new(
                    io::ErrorKind::TimedOut,
                    format!("tls handshake with {host} did not complete within {TLS_HANDSHAKE_TIMEOUT:?}"),
                )
            })?
            .map_err(|e| io::Error::other(format!("tls handshake with {host}: {e}")))?;
        Ok(Self::Tls(Box::new(tls)))
    }
}

/// Bound on the TCP handshake.
///
/// 5 seconds, matching Lich's `CONNECT_TIMEOUT` (`eaccess.rb:36`, recorded at
/// `plan/10` §2.1). Its comment there encodes a production failure worth
/// keeping: a **silently-dropped SYN** -- firewalled or blocked, with no RST
/// -- otherwise hangs on the OS connect timeout, "commonly ~75s on Linux."
/// `plan/10:1382` logs exactly that against this host on 2026-09-08.
const CONNECT_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(5);

/// Bound on the TLS handshake, which `CONNECT_TIMEOUT` does not cover.
const TLS_HANDSHAKE_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(10);

/// `TcpStream::connect`, bounded by [`CONNECT_TIMEOUT`].
///
/// Shared by both constructors: an unreachable game host hangs exactly as an
/// unreachable login host does, and there is no reason for one to be bounded
/// and the other not.
async fn connect_bounded(host: &str, port: u16) -> io::Result<TcpStream> {
    tokio::time::timeout(CONNECT_TIMEOUT, TcpStream::connect((host, port)))
        .await
        .map_err(|_elapsed| {
            io::Error::new(
                io::ErrorKind::TimedOut,
                format!(
                    "no TCP connection to {host}:{port} within {CONNECT_TIMEOUT:?}. A \
                     dropped SYN with no RST otherwise hangs on the OS timeout \
                     (~75s on Linux, ~21s on Windows) -- plan/10 §2.1."
                ),
            )
        })?
}

impl ByteSource for LiveSource {
    async fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        match self {
            Self::Plain(s) => s.read(buf).await,
            Self::Tls(s) => s.read(buf).await,
            Self::WebSocket(s) => s.read(buf).await,
        }
    }

    async fn write_all(&mut self, message: &[u8]) -> io::Result<()> {
        // ONE write of the finished message, then flush. See finding 1 in this
        // module's docs: splitting this into a write of the body and a write
        // of the newline emits two TLS records and the server drops the
        // command.
        match self {
            Self::Plain(s) => {
                s.write_all(message).await?;
                s.flush().await
            }
            Self::Tls(s) => {
                s.write_all(message).await?;
                s.flush().await
            }
            // One message is one WebSocket frame, which is one TLS write.
            Self::WebSocket(s) => s.write_all(message).await,
        }
    }

    async fn shutdown(&mut self) -> io::Result<()> {
        let result = match self {
            Self::Plain(s) => s.shutdown().await,
            Self::Tls(s) => s.shutdown().await,
            Self::WebSocket(s) => s.shutdown().await,
        };
        // Idempotence (the trait's contract, for criterion 6): a second
        // shutdown on a closed socket returns NotConnected on every platform
        // this targets, and that is success, not failure. The session shuts
        // down on three paths -- cancel, end of stream, read error -- and they
        // are not mutually exclusive.
        match result {
            Err(e) if e.kind() == io::ErrorKind::NotConnected => Ok(()),
            other => other,
        }
    }
}
