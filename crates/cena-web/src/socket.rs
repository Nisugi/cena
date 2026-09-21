//! Auth-first viewer connections. Each connection has one command in flight,
//! a bounded duplicate set, and no command outbox or retry path.

use crate::presentation::encode;
use crate::server::Shared;
use axum::extract::ws::{CloseFrame, Message, WebSocket};
use cena_session::{Generation, Outcome};
use cena_ui::{ClientMessage, ReceiptStatus, ServerMessage, WIRE_VERSION, validate_command};
use std::collections::HashSet;
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::{OwnedSemaphorePermit, broadcast};

const AUTH_TIMEOUT: Duration = Duration::from_secs(5);
const WRITE_TIMEOUT: Duration = Duration::from_secs(2);
const COMMAND_TIMEOUT: Duration = Duration::from_secs(10);
const MAX_REQUESTS: usize = 1024;
type Submission = Pin<Box<dyn Future<Output = ServerMessage> + Send>>;

pub(crate) async fn serve(
    mut socket: WebSocket,
    shared: Arc<Shared>,
    _permit: OwnedSemaphorePermit,
) {
    let authenticated = tokio::select! {
        () = shared.stop.cancelled() => false,
        result = tokio::time::timeout(AUTH_TIMEOUT, socket.recv()) => match result {
            Ok(Some(Ok(Message::Text(text)))) => authenticate(&text, &shared.token),
            _ => false,
        }
    };
    if !authenticated {
        close(&mut socket, 1008, "Authentication required").await;
        return;
    }
    let initial = tokio::select! {
        () = shared.stop.cancelled() => None,
        result = tokio::time::timeout(AUTH_TIMEOUT, attach(&shared)) => result.ok(),
    };
    let Some((snapshot, mut events)) = initial else {
        close(&mut socket, 1013, "Session unavailable").await;
        return;
    };
    if !write(&mut socket, &snapshot).await {
        return;
    }
    let mut requests = HashSet::new();
    let mut pending: Option<Submission> = None;
    loop {
        tokio::select! {
            () = shared.stop.cancelled() => { close(&mut socket, 1001, "Viewer stopped").await; return; }
            update = events.recv() => if let Ok(message) = update {
                if !write(&mut socket, &message).await { return; }
            } else {
                // A slow viewer reconnects for an atomic fresh snapshot. There
                // is no attempt to drain an indefinitely growing backlog.
                close(&mut socket, 1013, "Viewer lagged; reconnect for snapshot").await; return;
            },
            receipt = async {
                match pending.as_mut() { Some(future) => Some(future.await), None => None }
            }, if pending.is_some() => {
                pending = None;
                if let Some(receipt) = receipt {
                    let Ok(message) = encode(&receipt) else { return; };
                    if !write(&mut socket, &message).await { return; }
                }
            }
            incoming = socket.recv() => {
                match incoming {
                    Some(Ok(Message::Text(text))) => {
                        let Ok(ClientMessage::Command { version, session, generation, request_id, line })
                            = serde_json::from_str::<ClientMessage>(&text) else {
                                close(&mut socket, 1008, "Expected a command message").await; return;
                            };
                        if version != WIRE_VERSION || validate_command(&session, &generation, &request_id, &line).is_err()
                            || requests.contains(&request_id) {
                            close(&mut socket, 1008, "Invalid or duplicate command").await; return;
                        }
                        if requests.len() >= MAX_REQUESTS {
                            let refused = receipt(session, generation, request_id, ReceiptStatus::Refused,
                                "Connection request limit reached; command was not submitted");
                            if let Ok(message) = encode(&refused) { let _ = write(&mut socket, &message).await; }
                            close(&mut socket, 1013, "Reconnect for a fresh command connection").await;
                            return;
                        }
                        requests.insert(request_id.clone());
                        let refusal = {
                            let hub = shared.hub.lock().await;
                            if session != hub.session || generation != hub.generation {
                                Some("Session or generation changed; command refused")
                            } else if pending.is_some() { Some("A command is already awaiting its receipt") }
                            else { None }
                        };
                        if let Some(detail) = refusal {
                            let receipt = receipt(session, generation, request_id, ReceiptStatus::Refused, detail);
                            let Ok(message) = encode(&receipt) else { return; };
                            if !write(&mut socket, &message).await { return; }
                        } else {
                            let Ok(number) = generation.parse::<u32>() else {
                                close(&mut socket, 1008, "Unsupported generation").await; return;
                            };
                            let handle = shared.handle.clone();
                            pending = Some(Box::pin(async move {
                                let outcome = handle.send_manual_at(Generation(number), &line, COMMAND_TIMEOUT).await;
                                let (status, detail) = outcome_receipt(&outcome);
                                receipt(session, generation, request_id, status, detail)
                            }));
                        }
                    }
                    Some(Ok(Message::Ping(_) | Message::Pong(_))) => {}
                    Some(Ok(Message::Close(_))) | None => return,
                    _ => { close(&mut socket, 1008, "Text JSON messages required").await; return; }
                }
            }
        }
    }
}

fn authenticate(text: &str, expected: &str) -> bool {
    let Ok(ClientMessage::Authenticate { version, token }) = serde_json::from_str(text) else {
        return false;
    };
    // Fixed-width comparison avoids revealing matching token prefixes.
    version == WIRE_VERSION
        && token.len() == expected.len()
        && token
            .bytes()
            .zip(expected.bytes())
            .fold(0_u8, |difference, (a, b)| difference | (a ^ b))
            == 0
}

async fn attach(shared: &Shared) -> (Arc<str>, broadcast::Receiver<Arc<str>>) {
    loop {
        {
            let hub = shared.hub.lock().await;
            if let Some(snapshot) = &hub.snapshot {
                return (Arc::clone(snapshot), hub.updates.subscribe());
            }
        }
        tokio::time::sleep(Duration::from_millis(25)).await;
    }
}

fn receipt(
    session: String,
    generation: String,
    request_id: String,
    status: ReceiptStatus,
    detail: &str,
) -> ServerMessage {
    ServerMessage::Receipt {
        version: WIRE_VERSION,
        session,
        generation,
        request_id,
        status,
        detail: detail.to_owned(),
    }
}

fn outcome_receipt(outcome: &Outcome) -> (ReceiptStatus, &'static str) {
    match outcome {
        Outcome::Confirmed(_) => (
            ReceiptStatus::Sent,
            "Bytes sent and subsequent server output observed; action completion is not established",
        ),
        Outcome::Refused(_) => (
            ReceiptStatus::Refused,
            "Native session refused the command before sending",
        ),
        Outcome::Timeout => (
            ReceiptStatus::Uncertain,
            "Receipt timed out; command may have reached the game. Do not automatically resend",
        ),
        Outcome::Interrupted | Outcome::Dead | Outcome::Disconnected => (
            ReceiptStatus::Uncertain,
            "Session interrupted; delivery is unknown. Do not automatically resend",
        ),
    }
}

async fn write(socket: &mut WebSocket, message: &str) -> bool {
    matches!(
        tokio::time::timeout(
            WRITE_TIMEOUT,
            socket.send(Message::Text(message.to_owned().into()))
        )
        .await,
        Ok(Ok(()))
    )
}

async fn close(socket: &mut WebSocket, code: u16, reason: &'static str) {
    let _ = tokio::time::timeout(
        WRITE_TIMEOUT,
        socket.send(Message::Close(Some(CloseFrame {
            code,
            reason: reason.into(),
        }))),
    )
    .await;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn authentication_requires_first_message_kind_version_and_full_token() {
        let token = "a".repeat(64);
        assert!(authenticate(
            &format!(r#"{{"kind":"authenticate","version":1,"token":"{token}"}}"#),
            &token
        ));
        for text in [
            r#"{"kind":"authenticate","version":2,"token":"a"}"#,
            r#"{"kind":"command","version":1}"#,
            "{}",
            "not json",
        ] {
            assert!(!authenticate(text, &token));
        }
        assert!(!authenticate(
            &format!(
                r#"{{"kind":"authenticate","version":1,"token":"{}b"}}"#,
                "a".repeat(63)
            ),
            &token
        ));
    }

    #[test]
    fn missing_receipts_never_claim_refusal_or_action_completion() {
        for outcome in [
            Outcome::Timeout,
            Outcome::Interrupted,
            Outcome::Dead,
            Outcome::Disconnected,
        ] {
            assert_eq!(outcome_receipt(&outcome).0, ReceiptStatus::Uncertain);
        }
        assert_eq!(
            outcome_receipt(&Outcome::Refused(cena_session::Refusal::Transient)).0,
            ReceiptStatus::Refused
        );
    }
}
