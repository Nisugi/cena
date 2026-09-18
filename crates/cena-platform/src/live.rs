//! [`LiveSource`]: the TCP+TLS [`ByteSource`]. Criterion 1's transport.
//!
//! # BUILT, NOT RUN
//!
//! `CLAUDE.md` forbids logging into a live game service without the author
//! present, and `plan/12` §7.2 criterion 1 ("logs in against the live server")
//! is the one criterion Milestone 1 Step 2 does not exercise here. This module
//! is complete and compiles; **no test in this workspace calls
//! [`LiveSource::connect`]**, and none may. The author runs it.
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

use crate::bytes::ByteSource;
use std::io;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;

/// A live connection: plain TCP to the game, or TLS to eaccess.
///
/// Two shapes rather than two types because they differ only in whether a TLS
/// layer is wrapped around the same socket, and everything above them --
/// read, single-write, shutdown -- is identical. Two types would be a trait
/// with two implementors that never diverge, which is Rule -1's other half.
#[derive(Debug)]
pub enum LiveSource {
    /// The game stream. Plain TCP: the game host takes the key in the clear,
    /// which is what the spike does (`src/main.rs:341-358`).
    Plain(TcpStream),
    /// The eaccess handshake. TLS 1.2, static-RSA, `native-tls`.
    Tls(Box<tokio_native_tls::TlsStream<TcpStream>>),
}

impl LiveSource {
    /// Open a plain TCP connection.
    ///
    /// # Errors
    ///
    /// The underlying connect error, unchanged: a caller that cannot reach the
    /// host needs to know why, and wrapping it in a crate error type would be
    /// an abstraction with one caller.
    pub async fn connect(host: &str, port: u16) -> io::Result<Self> {
        let stream = connect_bounded(host, port).await?;
        // Nagle batches small writes, which is exactly wrong for a stream of
        // one-line commands: it would add up to 200ms to every round trip and
        // make criterion 4's PREEMPT_GRACE measurement a measurement of Nagle.
        stream.set_nodelay(true)?;
        Ok(Self::Plain(stream))
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
    /// # Errors
    ///
    /// Connect, TLS-builder and handshake errors, each mapped to
    /// [`io::Error`] so one `?` chain covers the sequence.
    pub async fn connect_tls(host: &str, port: u16) -> io::Result<Self> {
        let stream = connect_bounded(host, port).await?;
        stream.set_nodelay(true)?;
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
        }
    }

    async fn shutdown(&mut self) -> io::Result<()> {
        let result = match self {
            Self::Plain(s) => s.shutdown().await,
            Self::Tls(s) => s.shutdown().await,
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
