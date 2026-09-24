//! The game stream over play.net's WebSocket shim: `wss://<host>/shim/<port>`.
//!
//! # What this is for
//!
//! A network that allows only 80 and 443 -- a school, a hotel, some carriers
//! and VPN egress rules -- cannot reach the game port at all. play.net's own
//! browser client gets through such a network with a WebSocket-to-TCP shim on
//! 443, and once the WebSocket is open **the bytes are the same stream**: the
//! same key, the same banner, the same newline-delimited XML. Nothing above
//! the transport changes.
//!
//! Ported from Lich PR #1664 (`lib/common/game_transport.rb`,
//! `lib/common/websocket/stream.rb`), the follow-up to #1570's web login that
//! this crate already has. That PR records its evidence in
//! `docs/websocket-shim-probe-findings.md`: the browser client's connection
//! code (`SimuSocket.tryWebSocket` in play.net's `all_web_fe_min.js`), and full
//! live logins against production `GemStone` IV and `DragonRealms`. **Cena has
//! not run this live.** The author runs it (`CLAUDE.md`, Credentials).
//!
//! # The shim is not dialled at GAMEHOST
//!
//! The browser client remaps the host it was given to one of two fixed names,
//! by substring, `GemStone` first ([`shim_host`]). Both are `*.play.net` names
//! on the shared edge certificate, so **ordinary, unweakened TLS verification
//! passes** -- unlike the eaccess socket (`live.rs`), which needs three
//! weakenings. This is the first connection in this crate that verifies its
//! peer.
//!
//! # Framing
//!
//! Each [`ByteSource::write_all`](crate::bytes::ByteSource::write_all) is one
//! text message, which is what Lich's `Stream#puts` sends and what the browser
//! sends per command. Inbound text and binary messages are both game bytes and
//! are handed up unchanged: a message boundary is a read boundary, no more,
//! and the parser already reassembles lines across reads.

use std::io;
use std::time::Duration;

use futures_util::{SinkExt, StreamExt};
use tokio::io::{AsyncRead, AsyncWrite};
use tokio::net::TcpStream;
use tokio_tungstenite::WebSocketStream;
use tokio_tungstenite::tungstenite::client::IntoClientRequest;
use tokio_tungstenite::tungstenite::handshake::client::Request;
use tokio_tungstenite::tungstenite::http::HeaderValue;
use tokio_tungstenite::tungstenite::{Bytes, Error as WsError, Message};

/// The shim's port. Lich's `DEFAULT_SHIM_PORT`.
pub const SHIM_PORT: u16 = 443;

/// `Sec-WebSocket-Protocol`, as the browser client requests it.
const SUBPROTOCOL: &str = "websocket_shim-protocol";

/// Lich's `DEFAULT_USER_AGENT`, verbatim. Its handshake notes play.net's WAF
/// has previously refused requests that did not look like the browser client
/// they share this endpoint with.
const USER_AGENT: &str = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 \
                          (KHTML, like Gecko) Chrome/124.0.0.0 Safari/537.36";

/// Bound on the TLS handshake plus the HTTP upgrade. The TCP connect before it
/// has its own bound (`live.rs`'s `CONNECT_TIMEOUT`); this matches the eaccess
/// socket's TLS bound, for the same reason -- neither is covered by the other.
const HANDSHAKE_TIMEOUT: Duration = Duration::from_secs(10);

/// Bound on the closing handshake.
///
/// The plain game socket's `shutdown` cannot hang, and the session relies on
/// that: `cena-session`'s shutdown awaits it with no deadline of its own. A
/// WebSocket close is a round trip over TLS, and a peer that never answers must
/// not turn an orderly stop into a stuck one, so the bound lives here, in the
/// one source that needs it.
const CLOSE_TIMEOUT: Duration = Duration::from_secs(2);

/// The host the shim is dialled at, for the game host the login named.
///
/// The browser client's own rule (`all_web_fe_min.js`, quoted in Lich's
/// findings doc), checked in its order, first match wins:
///
/// | GAMEHOST contains | dial |
/// |---|---|
/// | `gs` or `chimera` | `chimera.play.net` |
/// | `dr` or `hydra` | `hydra.play.net` |
/// | neither | GAMEHOST unchanged |
///
/// The last row is dead in practice -- every known instance matches one of
/// the first two -- and is kept because it is what the source does. The match
/// is case-insensitive, as Lich's `/gs|chimera/i` is.
#[must_use]
pub fn shim_host(gamehost: &str) -> &str {
    let lower = gamehost.to_ascii_lowercase();
    if lower.contains("gs") || lower.contains("chimera") {
        "chimera.play.net"
    } else if lower.contains("dr") || lower.contains("hydra") {
        "hydra.play.net"
    } else {
        gamehost
    }
}

/// The upgrade request: `wss://<host>/shim/<gameport>`, with the headers the
/// browser client sends.
///
/// The game port rides in the PATH, not the connection: the socket goes to
/// 443 and the shim forwards to the port named here.
///
/// # Errors
///
/// A host that does not form a valid URI.
pub fn shim_request(host: &str, gameport: u16) -> io::Result<Request> {
    let mut request = format!("wss://{host}/shim/{gameport}")
        .into_client_request()
        .map_err(io::Error::other)?;
    let origin = HeaderValue::from_str(&format!("https://{host}")).map_err(io::Error::other)?;
    let headers = request.headers_mut();
    headers.insert(
        "Sec-WebSocket-Protocol",
        HeaderValue::from_static(SUBPROTOCOL),
    );
    headers.insert("Origin", origin);
    headers.insert("User-Agent", HeaderValue::from_static(USER_AGENT));
    Ok(request)
}

/// The TLS stream the shim runs over in production.
type TlsTcp = tokio_native_tls::TlsStream<TcpStream>;

/// A WebSocket carrying the game stream.
///
/// Generic over the transport only so the framing can be tested over an
/// in-memory pipe; production is always [`TlsTcp`].
#[derive(Debug)]
pub struct ShimStream<S = TlsTcp> {
    ws: WebSocketStream<S>,
    /// The rest of a message longer than the caller's buffer.
    unread: Bytes,
}

impl ShimStream {
    /// TLS to `host` over an already-connected `tcp`, then the upgrade.
    ///
    /// **Verification is ON**, with SNI: `TlsConnector::new()`, none of the
    /// eaccess socket's weakenings. See the module docs for why it passes.
    ///
    /// # Errors
    ///
    /// TLS or upgrade failure, or [`HANDSHAKE_TIMEOUT`] passing.
    pub async fn open(tcp: TcpStream, host: &str, gameport: u16) -> io::Result<Self> {
        let request = shim_request(host, gameport)?;
        let connector = native_tls::TlsConnector::new().map_err(io::Error::other)?;
        let connector = tokio_native_tls::TlsConnector::from(connector);
        let handshake = async {
            let tls = connector
                .connect(host, tcp)
                .await
                .map_err(|e| io::Error::other(format!("tls handshake with {host}: {e}")))?;
            Self::upgrade(request, tls).await
        };
        tokio::time::timeout(HANDSHAKE_TIMEOUT, handshake)
            .await
            .map_err(|_elapsed| {
                io::Error::new(
                    io::ErrorKind::TimedOut,
                    format!("websocket shim at {host} did not open within {HANDSHAKE_TIMEOUT:?}"),
                )
            })?
    }
}

impl<S: AsyncRead + AsyncWrite + Unpin> ShimStream<S> {
    /// The HTTP upgrade over an established stream.
    ///
    /// tungstenite checks the `101`, `Sec-WebSocket-Accept`, and that the
    /// server chose the subprotocol requested -- the three checks Lich's
    /// `Handshake` makes by hand.
    ///
    /// # Errors
    ///
    /// Any upgrade failure, as an [`io::Error`].
    pub async fn upgrade(request: Request, stream: S) -> io::Result<Self> {
        let (ws, _response) = tokio_tungstenite::client_async(request, stream)
            .await
            .map_err(|e| io::Error::other(format!("websocket upgrade: {e}")))?;
        Ok(Self {
            ws,
            unread: Bytes::new(),
        })
    }

    /// Read game bytes. `Ok(0)` is the end of the stream, including a close.
    ///
    /// Cancel-safe, which the session's read deadline requires: the only
    /// await is the stream's `next`, which loses nothing when dropped, and
    /// the copy out of `unread` happens after it returns.
    ///
    /// # Errors
    ///
    /// A transport or protocol error.
    pub async fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        loop {
            if !self.unread.is_empty() {
                let n = buf.len().min(self.unread.len());
                buf[..n].copy_from_slice(&self.unread.split_to(n));
                return Ok(n);
            }
            let Some(message) = self.ws.next().await else {
                return Ok(0);
            };
            match message.map_err(ws_to_io) {
                // An empty message leaves `unread` empty and the loop reads
                // on: returning `Ok(0)` for it would claim the stream ended.
                Ok(message @ (Message::Text(_) | Message::Binary(_))) => {
                    self.unread = message.into_data();
                }
                Ok(Message::Close(_)) => return Ok(0),
                // tungstenite answers pings itself; neither carries game bytes.
                Ok(Message::Ping(_) | Message::Pong(_) | Message::Frame(_)) => {}
                Err(e) if e.kind() == io::ErrorKind::NotConnected => return Ok(0),
                Err(e) => return Err(e),
            }
        }
    }

    /// Send one whole message as one text frame.
    ///
    /// # Errors
    ///
    /// A message that is not UTF-8 (a text frame must be), or a send failure.
    pub async fn write_all(&mut self, message: &[u8]) -> io::Result<()> {
        let text = std::str::from_utf8(message)
            .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;
        self.ws.send(Message::text(text)).await.map_err(ws_to_io)
    }

    /// Close the WebSocket, bounded by [`CLOSE_TIMEOUT`]. Idempotent.
    ///
    /// # Errors
    ///
    /// A transport error other than the connection already being closed.
    pub async fn shutdown(&mut self) -> io::Result<()> {
        match tokio::time::timeout(CLOSE_TIMEOUT, self.ws.close(None)).await {
            // A peer that never answers the close: the stream is dropped with
            // the source, which is the end either way.
            Err(_elapsed) => Ok(()),
            Ok(result) => match result.map_err(ws_to_io) {
                Err(e) if e.kind() == io::ErrorKind::NotConnected => Ok(()),
                other => other,
            },
        }
    }
}

/// A WebSocket error as the [`io::Error`] every `ByteSource` speaks.
///
/// A connection that is already closed maps to `NotConnected`, the kind the
/// plain socket's idempotent `shutdown` already treats as success.
fn ws_to_io(error: WsError) -> io::Error {
    match error {
        WsError::Io(e) => e,
        // `SendAfterClosing` is a close sent after the peer's: the handshake
        // is already over, which is closed by another name (found by
        // `a_close_is_the_end_of_the_stream_and_shutdown_is_idempotent`).
        WsError::ConnectionClosed
        | WsError::AlreadyClosed
        | WsError::Protocol(
            tokio_tungstenite::tungstenite::error::ProtocolError::SendAfterClosing,
        ) => io::Error::new(io::ErrorKind::NotConnected, "websocket closed"),
        other => io::Error::other(other),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::io::DuplexStream;

    #[test]
    fn the_host_is_remapped_as_the_browser_client_does() {
        // The rows Lich's findings doc records, including both families and
        // the two spellings each GAMEHOST has been seen under.
        for (gamehost, dialled) in [
            ("storm.gs4.game.play.net", "chimera.play.net"),
            ("chimera.simutronics.com", "chimera.play.net"),
            ("gs4.simutronics.net", "chimera.play.net"),
            ("dr.simutronics.net", "hydra.play.net"),
            ("storm.dr.game.play.net", "hydra.play.net"),
            ("hydra.simutronics.com", "hydra.play.net"),
            ("STORM.GS4.GAME.PLAY.NET", "chimera.play.net"),
            ("example.test", "example.test"),
        ] {
            assert_eq!(shim_host(gamehost), dialled, "{gamehost}");
        }
    }

    #[test]
    fn gemstone_is_checked_before_dragonrealms() {
        // A host matching both patterns goes to chimera: the source's order.
        assert_eq!(shim_host("gs-and-dr.example"), "chimera.play.net");
    }

    #[test]
    fn the_request_names_the_game_port_in_the_path_and_carries_the_headers() {
        let request = shim_request("chimera.play.net", 10024).expect("valid host");
        assert_eq!(
            request.uri().to_string(),
            "wss://chimera.play.net/shim/10024"
        );
        let header = |name: &str| {
            request
                .headers()
                .get(name)
                .and_then(|v| v.to_str().ok())
                .map(str::to_owned)
        };
        assert_eq!(
            header("Sec-WebSocket-Protocol").as_deref(),
            Some(SUBPROTOCOL)
        );
        assert_eq!(
            header("Origin").as_deref(),
            Some("https://chimera.play.net")
        );
        assert_eq!(header("User-Agent").as_deref(), Some(USER_AGENT));
    }

    /// A client over one end of an in-memory pipe, and the raw server end.
    #[allow(clippy::result_large_err)] // the callback's `Err` type is tungstenite's, not ours
    async fn pair() -> (ShimStream<DuplexStream>, WebSocketStream<DuplexStream>) {
        let (client, server) = tokio::io::duplex(64 * 1024);
        let request = shim_request("chimera.play.net", 10024).expect("valid host");
        let accept = tokio_tungstenite::accept_hdr_async(
            server,
            |_req: &tokio_tungstenite::tungstenite::handshake::server::Request,
             mut response: tokio_tungstenite::tungstenite::handshake::server::Response| {
                // The shim echoes the subprotocol; without it the client
                // refuses the upgrade, which is the check doing its job.
                response.headers_mut().insert(
                    "Sec-WebSocket-Protocol",
                    HeaderValue::from_static(SUBPROTOCOL),
                );
                Ok(response)
            },
        );
        let (client, server) = tokio::join!(ShimStream::upgrade(request, client), accept);
        (
            client.expect("client upgrade"),
            server.expect("server accept"),
        )
    }

    async fn read_all(client: &mut ShimStream<DuplexStream>, buf_len: usize) -> Vec<u8> {
        let mut out = Vec::new();
        let mut buf = vec![0; buf_len];
        loop {
            let n = client.read(&mut buf).await.expect("read");
            if n == 0 {
                return out;
            }
            out.extend_from_slice(&buf[..n]);
        }
    }

    #[tokio::test]
    async fn messages_become_one_byte_stream_whatever_their_boundaries() {
        let (mut client, mut server) = pair().await;
        // A line split across a text and a binary message, a ping between,
        // then the close.
        server
            .send(Message::text("<prompt ti"))
            .await
            .expect("send");
        server
            .send(Message::Ping(Bytes::from_static(b"p")))
            .await
            .expect("send");
        server
            .send(Message::binary(b"me='1'>&gt;</prompt>\n".to_vec()))
            .await
            .expect("send");
        server.close(None).await.expect("close");

        // A buffer smaller than either message: the remainder is kept.
        assert_eq!(
            read_all(&mut client, 4).await,
            b"<prompt time='1'>&gt;</prompt>\n"
        );
    }

    #[tokio::test]
    async fn each_write_is_one_text_message() {
        let (mut client, mut server) = pair().await;
        client.write_all(b"KEY\n").await.expect("write");
        client.write_all(b"<c>\n").await.expect("write");

        for expected in ["KEY\n", "<c>\n"] {
            let message = server.next().await.expect("a message").expect("ok");
            assert_eq!(message, Message::text(expected));
        }
    }

    #[tokio::test]
    async fn a_close_is_the_end_of_the_stream_and_shutdown_is_idempotent() {
        let (mut client, mut server) = pair().await;
        server.close(None).await.expect("close");
        let mut buf = [0; 16];
        assert_eq!(client.read(&mut buf).await.expect("read"), 0);
        client.shutdown().await.expect("first shutdown");
        client.shutdown().await.expect("second shutdown");
    }
}
