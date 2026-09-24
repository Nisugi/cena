//! What the web tests share: a browser that pairs, sends and receives, and a
//! wait for a session's `Ready`.
//!
//! Moved down out of `m4_native_web.rs` under `plan/05` Rule 4.1 when the
//! multi-session tests (`web_hub.rs`, `plan/29` step 5) took it past its cap
//! (900 of 800).

use cena_session::{Event, Generation, SessionObserver, State};
use cena_ui::{ClientMessage, ReceiptStatus, ServerMessage, StoryLine, WIRE_VERSION};
use futures_util::{SinkExt, StreamExt};
use std::error::Error;
use std::io;
use std::time::Duration;
use tokio::net::TcpStream;
use tokio_tungstenite::tungstenite::{Message, client::IntoClientRequest};
use tokio_tungstenite::{MaybeTlsStream, WebSocketStream, connect_async};

pub type TestResult<T = ()> = Result<T, Box<dyn Error + Send + Sync>>;
pub type Browser = WebSocketStream<MaybeTlsStream<TcpStream>>;
pub const DEADLINE: Duration = Duration::from_secs(5);
pub const ROOM: &[u8] = include_str!("../../../cena-protocol/tests/fixtures/room.xml").as_bytes();

pub async fn browser(pairing: &str) -> TestResult<Browser> {
    browser_for(pairing, None).await
}

/// A viewer for session `session`, as a page opened from its own link is.
pub async fn browser_for(pairing: &str, session: Option<&str>) -> TestResult<Browser> {
    let (base, token) = pairing
        .split_once("/#token=")
        .ok_or_else(|| io::Error::other("missing pairing fragment"))?;
    let mut request =
        format!("{}/ws", base.replacen("http://", "ws://", 1)).into_client_request()?;
    request.headers_mut().insert("origin", base.parse()?);
    let (mut socket, _) = connect_async(request).await?;
    send(
        &mut socket,
        &ClientMessage::Authenticate {
            version: WIRE_VERSION,
            token: token.to_owned(),
            session: session.map(str::to_owned),
        },
    )
    .await?;
    Ok(socket)
}

/// The next thing the server does with `socket` is close it.
pub async fn closes(socket: &mut Browser) -> bool {
    matches!(
        tokio::time::timeout(DEADLINE, socket.next()).await,
        Ok(Some(Ok(Message::Close(_)) | Err(_)) | None)
    )
}

pub async fn send(socket: &mut Browser, message: &ClientMessage) -> TestResult {
    socket
        .send(Message::Text(serde_json::to_string(message)?.into()))
        .await?;
    Ok(())
}

pub async fn receive(socket: &mut Browser) -> TestResult<ServerMessage> {
    let message = tokio::time::timeout(DEADLINE, socket.next())
        .await?
        .ok_or_else(|| io::Error::other("viewer closed before its message"))??;
    match message {
        Message::Text(text) => Ok(serde_json::from_str(&text)?),
        other => Err(io::Error::other(format!("expected application text: {other:?}")).into()),
    }
}

pub fn command(session: &str, generation: &str, request_id: &str, line: &str) -> ClientMessage {
    ClientMessage::Command {
        version: WIRE_VERSION,
        session: session.to_owned(),
        generation: generation.to_owned(),
        request_id: request_id.to_owned(),
        line: line.to_owned(),
    }
}

pub async fn receipt(
    socket: &mut Browser,
    expected_id: &str,
    lines: &mut Vec<StoryLine>,
) -> TestResult<ReceiptStatus> {
    tokio::time::timeout(DEADLINE, async {
        loop {
            match receive(socket).await? {
                ServerMessage::Receipt {
                    request_id, status, ..
                } if request_id == expected_id => return Ok(status),
                ServerMessage::Update { lines: new, .. } => lines.extend(new),
                ServerMessage::Snapshot { story, .. } => *lines = story,
                ServerMessage::Receipt { .. } => {
                    return Err(io::Error::other("unexpected command receipt").into());
                }
                ServerMessage::Sessions { .. }
                | ServerMessage::HubNote { .. }
                | ServerMessage::Merged { .. } => {
                    return Err(io::Error::other("a session's page was sent the hub").into());
                }
            }
        }
    })
    .await?
}

/// Wait until connection `generation` is `Ready`: the first prompt after
/// `<endSetup/>`, which `AnsweringSource::logged_in` sends on connect.
pub async fn await_ready(observer: &SessionObserver, generation: Generation) -> TestResult {
    let (snapshot, mut events) = observer
        .subscribe()
        .await
        .map_err(|error| io::Error::other(format!("native observation failed: {error:?}")))?;
    if snapshot.generation == generation && snapshot.lifecycle == State::Ready {
        return Ok(());
    }
    tokio::time::timeout(DEADLINE, async {
        loop {
            let event = events.recv().await?;
            if event.generation == generation && event.event == Event::StateChanged(State::Ready) {
                return Ok(());
            }
        }
    })
    .await?
}
