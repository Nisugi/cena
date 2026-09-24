//! Auth-first viewer connections. Each connection has one command in flight,
//! a bounded duplicate set, and no command outbox or retry path.

use crate::presentation::encode;
use crate::server::{Asked, Choice, HubRequest, Shared, Viewed};
use axum::extract::ws::{CloseFrame, Message, WebSocket};
use cena_session::{Generation, Outcome, SessionId};
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
/// The longest the hub page goes without catching up after a change, and the
/// shortest it waits between two sends -- so N characters' roundtimes
/// ticking together cost a card list at most four times a second.
const HUB_REFRESH: Duration = Duration::from_millis(250);
type Submission = Pin<Box<dyn Future<Output = ServerMessage> + Send>>;

pub(crate) async fn serve(
    mut socket: WebSocket,
    shared: Arc<Shared>,
    _permit: OwnedSemaphorePermit,
) {
    let asked = tokio::select! {
        () = shared.stop.cancelled() => None,
        result = tokio::time::timeout(AUTH_TIMEOUT, socket.recv()) => match result {
            Ok(Some(Ok(Message::Text(text)))) => authenticate(&text, &shared.token),
            _ => None,
        }
    };
    let Some(asked) = asked else {
        close(&mut socket, 1008, "Authentication required").await;
        return;
    };
    // Which session this viewer is for: the one it named, or the only one;
    // naming none with several running is the hub page.
    let viewed = match shared.choose(asked) {
        Choice::Session(viewed) => viewed,
        Choice::Hub => return serve_hub(socket, shared).await,
        Choice::Missing => {
            close(&mut socket, 1008, "No such session; open the page for one").await;
            return;
        }
    };
    let initial = tokio::select! {
        () = viewed.stop.cancelled() => None,
        result = tokio::time::timeout(AUTH_TIMEOUT, attach(&viewed)) => result.ok(),
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
            () = viewed.stop.cancelled() => { close(&mut socket, 1001, "Viewer stopped").await; return; }
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
                        // An early refusal only, never the authority. The hub's
                        // generation is the last one PUBLISHED and can lag the
                        // session's; the viewer's echoed `generation` is what
                        // goes to `send_manual_at`, whose actor checks it
                        // against the live connection before anything acts.
                        // This comparison can only refuse a viewer that is
                        // behind the hub, which is behind the session.
                        let refusal = {
                            let hub = viewed.hub.lock().await;
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
                            let handle = viewed.handle.clone();
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

/// A hub page's add or remove request, or `None` for anything else -- which
/// includes a character name that is empty or too long, and a session id
/// that is not canonical.
fn hub_request(text: &str) -> Option<HubRequest> {
    match serde_json::from_str(text).ok()? {
        ClientMessage::AddCharacter { version, character } => {
            let named = !character.trim().is_empty() && character.len() <= 64;
            (version == WIRE_VERSION && named).then_some(HubRequest::Add(character))
        }
        ClientMessage::RemoveSession { version, session } => {
            let id = session_id(&session)?;
            (version == WIRE_VERSION).then_some(HubRequest::Remove(id))
        }
        _ => None,
    }
}

/// A canonical decimal session id: digits only, no leading zero.
fn session_id(id: &str) -> Option<SessionId> {
    let canonical = !id.is_empty()
        && id.bytes().all(|b| b.is_ascii_digit())
        && (id == "0" || !id.starts_with('0'));
    id.parse::<u32>().ok().filter(|_| canonical).map(SessionId)
}

/// The session a correctly authenticated viewer asked for, or `None` when
/// authentication failed.
///
/// A named session must be a canonical decimal id; anything else fails
/// authentication rather than being read as "none named".
fn authenticate(text: &str, expected: &str) -> Option<Asked> {
    let Ok(ClientMessage::Authenticate {
        version,
        token,
        session,
    }) = serde_json::from_str(text)
    else {
        return None;
    };
    // Fixed-width comparison avoids revealing matching token prefixes.
    let matched = version == WIRE_VERSION
        && token.len() == expected.len()
        && token
            .bytes()
            .zip(expected.bytes())
            .fold(0_u8, |difference, (a, b)| difference | (a ^ b))
            == 0;
    if !matched {
        return None;
    }
    match session {
        None => Some(Asked::Only),
        Some(id) => Some(Asked::Session(session_id(&id)?)),
    }
}

async fn attach(viewed: &Viewed) -> (Arc<str>, broadcast::Receiver<Arc<str>>) {
    loop {
        {
            let hub = viewed.hub.lock().await;
            if let Some(snapshot) = &hub.snapshot {
                return (Arc::clone(snapshot), hub.updates.subscribe());
            }
        }
        tokio::time::sleep(Duration::from_millis(25)).await;
    }
}

/// The hub page: every session's card (`plan/29` step 5b), sent when a card
/// changes and never more often than [`HUB_REFRESH`]. It takes no commands:
/// a command belongs to one character, whose own page sends it.
async fn serve_hub(mut socket: WebSocket, shared: Arc<Shared>) {
    let mut changed = shared.changed.subscribe();
    // Subscribed before the history is read, so no merged line falls between
    // the two; one that is in both arrives twice, and a page keys lines by id.
    let mut merged = shared.merged.updates.subscribe();
    let mut last: Option<Arc<str>> = None;
    let mut history_sent = false;
    loop {
        let available = if shared
            .control
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .is_some()
        {
            shared
                .available
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .clone()
        } else {
            Vec::new()
        };
        let cards = ServerMessage::Sessions {
            version: WIRE_VERSION,
            sessions: shared.cards().await,
            available,
        };
        let Ok(message) = encode(&cards) else {
            close(&mut socket, 1011, "The session list is too large to send").await;
            return;
        };
        if last.as_deref() != Some(&*message) {
            if !write(&mut socket, &message).await {
                return;
            }
            last = Some(message);
        }
        if !history_sent {
            history_sent = true;
            let history = ServerMessage::Merged {
                version: WIRE_VERSION,
                lines: shared.merged.history(),
            };
            let Ok(message) = encode(&history) else {
                return;
            };
            if !write(&mut socket, &message).await {
                return;
            }
        }
        tokio::select! {
            () = shared.stop.cancelled() => { close(&mut socket, 1001, "Viewer stopped").await; return; }
            // A lag is only "something changed" said more than once.
            _ = changed.recv() => tokio::time::sleep(HUB_REFRESH).await,
            line = merged.recv() => match line {
                Ok(message) => if !write(&mut socket, &message).await { return; },
                // Behind: the page resynchronises on its fresh history.
                Err(broadcast::error::RecvError::Lagged(_)) => {
                    close(&mut socket, 1013, "Hub lagged; reconnect for the merged history").await;
                    return;
                }
                Err(broadcast::error::RecvError::Closed) => {}
            },
            incoming = socket.recv() => match incoming {
                Some(Ok(Message::Ping(_) | Message::Pong(_))) => {}
                Some(Ok(Message::Close(_)) | Err(_)) | None => return,
                Some(Ok(Message::Text(text))) => {
                    let Some(request) = hub_request(&text) else {
                        close(&mut socket, 1008, "The hub takes only add and remove requests; commands go to a character's page").await;
                        return;
                    };
                    let control = shared
                        .control
                        .lock()
                        .unwrap_or_else(std::sync::PoisonError::into_inner)
                        .clone();
                    let detail = match control {
                        Some(control) => control(request).await,
                        None => "Adding and removing characters is not offered here.".to_owned(),
                    };
                    let note = ServerMessage::HubNote { version: WIRE_VERSION, detail };
                    let Ok(message) = encode(&note) else { return; };
                    if !write(&mut socket, &message).await { return; }
                }
                Some(Ok(_)) => {
                    close(&mut socket, 1008, "Text JSON messages required").await;
                    return;
                }
            },
        }
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

/// The receipt for what `send_manual_at` answered.
///
/// **Every claimed `;` line used to come back as "Bytes sent and subsequent
/// server output observed"**, and neither half was true. The session answered
/// it `Confirmed` with a prompt no server sent, so this module told claimed
/// lines apart by their first character. The session now answers
/// [`Outcome::Handled`], and the receipt reads it off the outcome -- a
/// claimed line from a stale generation is still the session's to refuse,
/// and is reported `Disconnected` like any other.
fn outcome_receipt(outcome: &Outcome) -> (ReceiptStatus, &'static str) {
    match outcome {
        Outcome::Handled => (
            ReceiptStatus::Handled,
            "Handled by Hydra; nothing was sent to the game",
        ),
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
        assert_eq!(
            authenticate(
                &format!(r#"{{"kind":"authenticate","version":1,"token":"{token}"}}"#),
                &token
            ),
            Some(Asked::Only),
            "authenticated, naming no session"
        );
        for text in [
            r#"{"kind":"authenticate","version":2,"token":"a"}"#,
            r#"{"kind":"command","version":1}"#,
            "{}",
            "not json",
        ] {
            assert_eq!(authenticate(text, &token), None);
        }
        assert_eq!(
            authenticate(
                &format!(
                    r#"{{"kind":"authenticate","version":1,"token":"{}b"}}"#,
                    "a".repeat(63)
                ),
                &token
            ),
            None
        );
    }

    /// `plan/29` step 5: a viewer names the session its page is for. A
    /// malformed id fails authentication rather than being read as "none".
    #[test]
    fn a_viewer_names_its_session_by_canonical_decimal_id() {
        let token = "a".repeat(64);
        let with = |session: &str| {
            authenticate(
                &format!(
                    r#"{{"kind":"authenticate","version":1,"token":"{token}","session":"{session}"}}"#
                ),
                &token,
            )
        };
        assert_eq!(with("0"), Some(Asked::Session(SessionId(0))));
        assert_eq!(with("7"), Some(Asked::Session(SessionId(7))));
        for bad in ["", "07", "-1", "x", "4294967296"] {
            assert_eq!(with(bad), None, "{bad:?} is not a session id");
        }
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

    /// **A `;` line never reaches the game**, and its receipt said it had:
    /// "Bytes sent and subsequent server output observed", on a prompt the
    /// session fabricated. The receipt must say what happened. Which lines
    /// are claimed is the session's rule, tested there (`claimed_commands.rs`).
    #[test]
    fn a_line_hydra_handled_is_not_reported_as_sent() {
        let (status, detail) = outcome_receipt(&Outcome::Handled);
        assert_eq!(status, ReceiptStatus::Handled);
        assert!(detail.contains("nothing was sent"), "{detail}");

        let answered = Outcome::Confirmed(Box::new(cena_session::Frame::Prompt {
            text: String::new(),
            time: String::new(),
        }));
        assert_eq!(outcome_receipt(&answered).0, ReceiptStatus::Sent);
    }
}
