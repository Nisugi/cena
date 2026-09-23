//! The real native observer -> loopback WebSocket -> presentation boundary.
//!
//! XML comes verbatim from the committed, scrubbed protocol corpus (see its
//! FIXTURES.md). The reply deliberately combines captures to cover the M4
//! panels; it is not claimed to be one recorded login or a real command reply.
//! `AnsweringSource` supplies only transport timing and a write transcript.

use cena_platform::AnsweringSource;
use cena_session::{
    AuthorityToken, ConnectError, Connector, Event, Generation, Origin, Outcome, Session,
    SessionObserver, State, SupervisedSession,
};
use cena_ui::{
    ClientMessage, HandView, LifecycleView, ReceiptStatus, ServerMessage, SessionView, StoryLine,
    VitalView, WIRE_VERSION,
};
use cena_web::WebServer;
use futures_util::{SinkExt, StreamExt};
use std::collections::VecDeque;
use std::error::Error;
use std::io;
use std::time::Duration;
use tokio::net::TcpStream;
use tokio_tungstenite::tungstenite::{Message, client::IntoClientRequest};
use tokio_tungstenite::{MaybeTlsStream, WebSocketStream, connect_async};
use tokio_util::sync::CancellationToken;

type TestResult<T = ()> = Result<T, Box<dyn Error + Send + Sync>>;
type Browser = WebSocketStream<MaybeTlsStream<TcpStream>>;
const DEADLINE: Duration = Duration::from_secs(5);
const ROOM: &[u8] = include_str!("../../cena-protocol/tests/fixtures/room.xml").as_bytes();

fn panel_reply() -> Vec<u8> {
    [
        include_str!("../../cena-protocol/tests/fixtures/login_burst_full.xml").as_bytes(),
        include_str!("../../cena-protocol/tests/fixtures/vitals.xml").as_bytes(),
        ROOM,
    ]
    .concat()
}

async fn browser(pairing: &str) -> TestResult<Browser> {
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
        },
    )
    .await?;
    Ok(socket)
}

async fn send(socket: &mut Browser, message: &ClientMessage) -> TestResult {
    socket
        .send(Message::Text(serde_json::to_string(message)?.into()))
        .await?;
    Ok(())
}

async fn receive(socket: &mut Browser) -> TestResult<ServerMessage> {
    let message = tokio::time::timeout(DEADLINE, socket.next())
        .await?
        .ok_or_else(|| io::Error::other("viewer closed before its message"))??;
    match message {
        Message::Text(text) => Ok(serde_json::from_str(&text)?),
        other => Err(io::Error::other(format!("expected application text: {other:?}")).into()),
    }
}

fn command(session: &str, generation: &str, request_id: &str, line: &str) -> ClientMessage {
    ClientMessage::Command {
        version: WIRE_VERSION,
        session: session.to_owned(),
        generation: generation.to_owned(),
        request_id: request_id.to_owned(),
        line: line.to_owned(),
    }
}

async fn receipt(
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
            }
        }
    })
    .await?
}

fn line_text(line: &StoryLine) -> String {
    line.runs.iter().map(|run| run.text.as_str()).collect()
}

fn assert_panels(view: &SessionView) {
    assert_eq!(view.lifecycle, LifecycleView::Ready);
    assert_eq!(view.room.id.as_deref(), Some("7503251"));
    assert_eq!(
        view.room.title.as_deref(),
        Some("Rawknuckle's, Watering Hole")
    );
    assert!(
        view.room
            .description
            .as_ref()
            .is_some_and(|runs| runs.iter().any(|run| run.text.contains("Low eaves")))
    );
    // The fixture's later compass authoritatively replaces "east" with "e".
    assert_eq!(
        view.room.exits,
        Some(vec!["e".to_owned(), "out".to_owned()])
    );
    assert_eq!(view.room.creatures.as_ref().map(Vec::len), Some(2));
    assert_eq!(view.room.objects, Some(Vec::new()));
    assert_eq!(view.room.players, Some(Vec::new()));
    assert_eq!(
        view.left_hand,
        HandView::Holding {
            id: Some("364757584".into()),
            noun: Some("gift".into()),
            name: "plain gift".into(),
        }
    );
    assert_eq!(view.right_hand, HandView::Empty);
    assert_eq!(
        view.vitals.health,
        Some(VitalView {
            percent: 99,
            current: Some(225),
            max: Some(226)
        })
    );
}

async fn assert_story(socket: &mut Browser, mut lines: Vec<StoryLine>) -> TestResult {
    // Real room output fragments span anchors, style changes and nested bold.
    tokio::time::timeout(DEADLINE, async {
        while !lines.iter().any(|line| {
            line_text(line).starts_with("Low eaves") && line.runs.iter().any(|run| run.bold)
        }) {
            match receive(socket).await? {
                ServerMessage::Update { lines: new, .. } => lines.extend(new),
                ServerMessage::Snapshot { story, .. } => lines = story,
                ServerMessage::Receipt { .. } => {
                    return Err(io::Error::other("expected room presentation").into());
                }
            }
        }
        Ok::<(), Box<dyn Error + Send + Sync>>(())
    })
    .await??;
    let title = lines
        .iter()
        .find(|line| line_text(line) == "[Rawknuckle's, Watering Hole]")
        .ok_or_else(|| io::Error::other("missing complete room title"))?;
    assert!(
        title
            .runs
            .iter()
            .any(|run| run.preset.as_deref() == Some("roomName"))
    );
    let description = lines
        .iter()
        .find(|line| {
            line_text(line).starts_with("Low eaves") && line.runs.iter().any(|run| run.bold)
        })
        .ok_or_else(|| io::Error::other("missing complete styled description"))?;
    assert!(
        line_text(description)
            .ends_with("a raw-boned halfling tavernkeeper and a gaunt masked artificer.")
    );
    assert!(
        description
            .runs
            .iter()
            .any(|run| run.bold && run.text.contains("tavernkeeper"))
    );
    assert!(
        lines
            .iter()
            .all(|line| !line_text(line).contains("<a exist="))
    );
    Ok(())
}

#[tokio::test]
async fn late_viewer_gets_native_panels_and_manual_input_preserves_behavior_authority() {
    let (source, transcript) = AnsweringSource::logged_in(&panel_reply());
    let session = Session::new(source);
    let handle = session.handle();
    let observer = session.observer();
    let stop_session = session.cancel_token();
    let actor = tokio::spawn(session.into_actor().run());
    await_ready(&observer, Generation::FIRST).await.unwrap();
    assert!(matches!(
        handle
            .send_manual_at(Generation::FIRST, "look", DEADLINE)
            .await,
        Outcome::Confirmed(_)
    ));

    // The web frontend itself attaches after all panel data was learned.
    let (native, mut events) = observer.subscribe().await.unwrap();
    assert_eq!(native.lifecycle, State::Ready);
    let server = WebServer::bind(observer.clone(), handle.clone())
        .await
        .unwrap();
    let pairing = server.pairing_url();
    let stop_web = CancellationToken::new();
    let web = tokio::spawn(server.run(stop_web.clone().cancelled_owned()));
    let mut socket = browser(&pairing).await.unwrap();
    let ServerMessage::Snapshot {
        version,
        session,
        generation,
        view,
        story,
        history_gap,
        ..
    } = receive(&mut socket).await.unwrap()
    else {
        panic!("the first authenticated application message must be a snapshot");
    };
    assert_eq!(version, WIRE_VERSION);
    assert_eq!(session, native.session.to_string());
    assert_eq!(generation, native.generation.0.to_string());
    assert_panels(&view);
    assert!(
        story.is_empty(),
        "late attachment must not invent missed Story"
    );
    assert!(!history_gap);

    let authority = AuthorityToken(71);
    handle.claim(authority).await.unwrap();
    let input = command(&session, &generation, "manual-1", "look");
    send(&mut socket, &input).await.unwrap();
    let mut lines = Vec::new();
    assert_eq!(
        receipt(&mut socket, "manual-1", &mut lines).await.unwrap(),
        ReceiptStatus::Sent
    );
    let sent = tokio::time::timeout(DEADLINE, events.recv())
        .await
        .unwrap()
        .unwrap();
    assert_eq!(
        sent.event,
        Event::Sent {
            line: "look".into(),
            origin: Origin::Manual
        }
    );
    assert_eq!(sent.cursor, native.cursor + 1);
    assert_eq!(transcript.lines(), ["look", "look"]);
    assert_eq!(
        handle.claim(AuthorityToken(72)).await.unwrap_err().0,
        authority
    );
    assert_story(&mut socket, lines).await.unwrap();

    // Duplicate ids close the offending connection, without another send.
    send(&mut socket, &input).await.unwrap();
    let closed = tokio::time::timeout(DEADLINE, socket.next())
        .await
        .unwrap()
        .unwrap()
        .unwrap();
    assert!(matches!(closed, Message::Close(Some(frame)) if u16::from(frame.code) == 1008));
    assert_eq!(transcript.written_count(), 2);
    let mut reattached = browser(&pairing).await.unwrap();
    assert!(
        matches!(receive(&mut reattached).await.unwrap(), ServerMessage::Snapshot { view, .. } if view.lifecycle == LifecycleView::Ready)
    );
    reattached.close(None).await.unwrap();
    stop_web.cancel();
    web.await.unwrap().unwrap();
    assert!(
        !transcript.is_shutdown(),
        "closing viewers and server must leave the native session alive"
    );
    assert!(matches!(
        handle
            .send_manual_at(native.generation, "look", DEADLINE)
            .await,
        Outcome::Confirmed(_)
    ));
    assert_eq!(transcript.lines(), ["look", "look", "look"]);
    handle.release(authority);
    stop_session.cancel();
    actor.await.unwrap();
}

struct Connections(VecDeque<AnsweringSource>);

impl Connector for Connections {
    type Source = AnsweringSource;

    async fn connect(&mut self, _: Generation) -> Result<Self::Source, ConnectError> {
        self.0
            .pop_front()
            .ok_or_else(|| ConnectError::fatal("fixture", "exhausted"))
    }
}

#[tokio::test]
async fn reconnect_refuses_old_browser_generation_including_quit_without_writing() {
    let (first, first_transcript) = AnsweringSource::logged_in(ROOM);
    let (second, second_transcript) = AnsweringSource::logged_in(ROOM);
    let (session, handle) = SupervisedSession::new(Connections(vec![first, second].into()));
    let observer = session.observer();
    let stop_session = session.cancel_token();
    let actor = tokio::spawn(session.run());
    await_ready(&observer, Generation::FIRST).await.unwrap();
    let (initial, _) = observer.subscribe().await.unwrap();
    assert_eq!(initial.lifecycle, State::Ready);
    assert!(matches!(
        handle
            .send_manual_at(initial.generation, "look", DEADLINE)
            .await,
        Outcome::Confirmed(_)
    ));
    let server = WebServer::bind(observer.clone(), handle.clone())
        .await
        .unwrap();
    let pairing = server.pairing_url();
    let stop_web = CancellationToken::new();
    let web = tokio::spawn(server.run(stop_web.clone().cancelled_owned()));
    let mut socket = browser(&pairing).await.unwrap();
    let ServerMessage::Snapshot {
        session,
        generation,
        view,
        ..
    } = receive(&mut socket).await.unwrap()
    else {
        panic!("first application message was not a snapshot");
    };
    assert_eq!(view.room.creatures.as_ref().unwrap().len(), 2);
    assert_eq!(view.room.objects, Some(Vec::new()));
    assert_eq!(view.room.players, Some(Vec::new()));
    assert_eq!(view.roundtime.remaining_seconds, Some(0));
    first_transcript.hang_up();
    let mut saw_reconnecting = false;
    let next_generation = tokio::time::timeout(DEADLINE, async {
        loop {
            let (current, view) = match receive(&mut socket).await.unwrap() {
                ServerMessage::Update {
                    generation, view, ..
                }
                | ServerMessage::Snapshot {
                    generation, view, ..
                } => (generation, view),
                other @ ServerMessage::Receipt { .. } => {
                    panic!("unexpected reconnect message: {other:?}")
                }
            };
            // Invalidated until the new connection's login burst re-teaches
            // it (`plan/12` §5.2). `Ready` is the prompt that ENDS that
            // burst, so the Ready view may already know the clock again.
            if current != generation && view.lifecycle != LifecycleView::Ready {
                assert_eq!(view.vitals.health, None);
                assert_eq!(view.roundtime.remaining_seconds, None);
                assert_eq!(view.room.objects, None);
                assert_eq!(view.room.creatures, None);
                assert_eq!(view.room.players, None);
                saw_reconnecting |= matches!(view.lifecycle, LifecycleView::Reconnecting { .. });
            }
            if current != generation && view.lifecycle == LifecycleView::Ready {
                break current;
            }
        }
    })
    .await
    .unwrap();
    assert!(
        saw_reconnecting,
        "the browser observes native reconnect lifecycle"
    );
    assert_eq!(next_generation, initial.generation.next().0.to_string());
    for line in ["look", "quit"] {
        send(&mut socket, &command(&session, &generation, line, line))
            .await
            .unwrap();
        assert_eq!(
            receipt(&mut socket, line, &mut Vec::new()).await.unwrap(),
            ReceiptStatus::Refused
        );
    }
    assert_eq!(second_transcript.written_count(), 0);
    assert!(!second_transcript.is_shutdown());

    send(
        &mut socket,
        &command(&session, &next_generation, "current", "look"),
    )
    .await
    .unwrap();
    assert_eq!(
        receipt(&mut socket, "current", &mut Vec::new())
            .await
            .unwrap(),
        ReceiptStatus::Sent
    );
    assert_eq!(second_transcript.lines(), ["look"]);
    socket.close(None).await.unwrap();
    stop_web.cancel();
    web.await.unwrap().unwrap();
    stop_session.cancel();
    actor.await.unwrap();
}

#[tokio::test]
async fn websocket_requires_local_origin_and_auth_before_state_or_native_commands() {
    let (source, transcript) = AnsweringSource::new(ROOM);
    let session = Session::new(source);
    let handle = session.handle();
    let stop_session = session.cancel_token();
    let server = WebServer::bind(session.observer(), handle).await.unwrap();
    let pairing = server.pairing_url();
    let (base, token) = pairing.split_once("/#token=").unwrap();
    let address = format!("{}/ws", base.replacen("http://", "ws://", 1));
    let actor = tokio::spawn(session.into_actor().run());
    let stop_web = CancellationToken::new();
    let web = tokio::spawn(server.run(stop_web.clone().cancelled_owned()));

    for (origin, host, query) in [
        (None, None, ""),
        (Some("https://evil.example"), None, ""),
        (Some(base), Some("evil.example"), ""),
        (Some(base), None, "?token=not-allowed"),
    ] {
        let mut request = format!("{address}{query}").into_client_request().unwrap();
        if let Some(origin) = origin {
            request
                .headers_mut()
                .insert("origin", origin.parse().unwrap());
        }
        if let Some(host) = host {
            request.headers_mut().insert("host", host.parse().unwrap());
        }
        let error = connect_async(request).await.unwrap_err();
        assert!(
            matches!(error, tokio_tungstenite::tungstenite::Error::Http(response) if response.status().as_u16() == 403)
        );
    }

    for first_message in [
        ClientMessage::Authenticate {
            version: WIRE_VERSION,
            token: "incorrect".into(),
        },
        ClientMessage::Authenticate {
            version: WIRE_VERSION + 1,
            token: token.into(),
        },
        command("0", "0", "unauthenticated", "quit"),
    ] {
        let mut request = address.clone().into_client_request().unwrap();
        request
            .headers_mut()
            .insert("origin", base.parse().unwrap());
        let (mut socket, _) = connect_async(request).await.unwrap();
        assert!(
            tokio::time::timeout(Duration::from_millis(30), socket.next())
                .await
                .is_err(),
            "an unauthenticated viewer must receive no session data"
        );
        send(&mut socket, &first_message).await.unwrap();
        let closed = tokio::time::timeout(DEADLINE, socket.next())
            .await
            .unwrap()
            .unwrap()
            .unwrap();
        assert!(matches!(closed, Message::Close(Some(frame)) if u16::from(frame.code) == 1008));
    }

    // An authenticated oversized frame must never reach native input either.
    let mut socket = browser(&pairing).await.unwrap();
    assert!(matches!(
        receive(&mut socket).await.unwrap(),
        ServerMessage::Snapshot { .. }
    ));
    socket
        .send(Message::Text("x".repeat(16 * 1024 + 1).into()))
        .await
        .unwrap();
    let closed = tokio::time::timeout(DEADLINE, socket.next()).await.unwrap();
    assert!(matches!(
        closed,
        Some(Ok(Message::Close(_)) | Err(_)) | None
    ));
    assert_eq!(transcript.written_count(), 0);
    assert!(!transcript.is_shutdown());
    stop_web.cancel();
    web.await.unwrap().unwrap();
    stop_session.cancel();
    actor.await.unwrap();
}

/// Wait until connection `generation` is `Ready`: the first prompt after
/// `<endSetup/>`, which `AnsweringSource::logged_in` sends on connect.
async fn await_ready(observer: &SessionObserver, generation: Generation) -> TestResult {
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

#[tokio::test]
async fn exhausted_request_budget_refuses_before_close_and_fresh_connection_can_send() {
    let (first, disconnected) = AnsweringSource::new(ROOM);
    let (source, transcript) = AnsweringSource::logged_in(ROOM);
    let (session, handle) = SupervisedSession::new(Connections(vec![first, source].into()));
    let observer = session.observer();
    let stop_session = session.cancel_token();
    disconnected.hang_up();
    let actor = tokio::spawn(session.run());
    await_ready(&observer, Generation::FIRST.next())
        .await
        .unwrap();
    let server = WebServer::bind(observer, handle).await.unwrap();
    let pairing = server.pairing_url();
    let stop_web = CancellationToken::new();
    let web = tokio::spawn(server.run(stop_web.clone().cancelled_owned()));
    let mut socket = browser(&pairing).await.unwrap();
    let ServerMessage::Snapshot {
        session,
        generation,
        view,
        ..
    } = receive(&mut socket).await.unwrap()
    else {
        panic!("first application message must be a snapshot");
    };
    assert_eq!(generation, "1");
    assert_eq!(view.lifecycle, LifecycleView::Ready);
    // Use the actual production budget, with a deadline for the entire loop.
    tokio::time::timeout(DEADLINE, async {
        for index in 0..1024 {
            let request_id = format!("stale-{index}");
            send(&mut socket, &command(&session, "0", &request_id, "look"))
                .await
                .unwrap();
            assert_eq!(
                receipt(&mut socket, &request_id, &mut Vec::new())
                    .await
                    .unwrap(),
                ReceiptStatus::Refused
            );
        }
    })
    .await
    .unwrap();
    assert_eq!(transcript.written_count(), 0);

    // This request is otherwise valid: only the full request budget refuses it.
    send(
        &mut socket,
        &command(&session, &generation, "over-budget", "look"),
    )
    .await
    .unwrap();
    let ServerMessage::Receipt {
        request_id,
        status,
        detail,
        ..
    } = receive(&mut socket).await.unwrap()
    else {
        panic!("exhaustion must produce its refusal before closing");
    };
    assert_eq!(request_id, "over-budget");
    assert_eq!(status, ReceiptStatus::Refused);
    assert!(detail.contains("request limit"));
    let closed = tokio::time::timeout(DEADLINE, socket.next())
        .await
        .unwrap()
        .unwrap()
        .unwrap();
    assert!(matches!(closed, Message::Close(Some(frame)) if u16::from(frame.code) == 1013));
    assert_eq!(transcript.written_count(), 0);
    assert!(!transcript.is_shutdown());

    let mut fresh = browser(&pairing).await.unwrap();
    let ServerMessage::Snapshot {
        session: fresh_session,
        generation: fresh_generation,
        view,
        ..
    } = receive(&mut fresh).await.unwrap()
    else {
        panic!("reconnecting must provide a fresh snapshot");
    };
    assert_eq!(fresh_session, session);
    assert_eq!(fresh_generation, generation);
    assert_eq!(view.lifecycle, LifecycleView::Ready);
    send(
        &mut fresh,
        &command(&fresh_session, &fresh_generation, "fresh-command", "look"),
    )
    .await
    .unwrap();
    assert_eq!(
        receipt(&mut fresh, "fresh-command", &mut Vec::new())
            .await
            .unwrap(),
        ReceiptStatus::Sent
    );
    assert_eq!(transcript.lines(), ["look"]);
    fresh.close(None).await.unwrap();
    stop_web.cancel();
    web.await.unwrap().unwrap();
    stop_session.cancel();
    actor.await.unwrap();
}
