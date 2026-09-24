use super::hub::lifecycle;
use super::*;
use super::{MAX_HISTORY_BYTES, MAX_HISTORY_LINES, MAX_WIRE_BYTES};
use cena_session::{Frame, GameState, Generation, SessionId};
use cena_ui::{Closed, LifecycleView, ServerMessage, StoryLine, StyledRun, WIRE_VERSION};

fn snapshot(cursor: u64) -> Snapshot {
    Snapshot {
        session: SessionId::FIRST,
        generation: Generation::FIRST,
        cursor,
        state: GameState::default(),
        lifecycle: State::Ready,
        retry: None,
    }
}

/// A state that has seen the login burst's `speech` declaration.
fn declared_speech(cursor: u64) -> Snapshot {
    let mut snapshot = snapshot(cursor);
    // The frame the parser makes of
    // `<streamWindow id='speech' ifClosed='' .../>`. Built directly rather
    // than parsed: `cena-session` deliberately does not re-export `Parser`
    // (see its lib.rs), and the attribute pair IS the input under test.
    snapshot.state.apply(&Frame::StreamWindow {
        id: "speech".into(),
        title: Some("Speech".into()),
        subtitle: None,
        attrs: vec![
            ("id".into(), "speech".into()),
            ("ifClosed".into(), String::new()),
            ("resident".into(), "true".into()),
        ],
    });
    snapshot
}

fn line(stream: &str) -> StoryLine {
    StoryLine {
        stream: stream.to_owned(),
        runs: vec![StyledRun {
            text: "You say, \"Yep.\"".into(),
            ..StyledRun::default()
        }],
        truncated: false,
        // Deliberately the DEFAULT, so the assertion proves `publish`
        // stamped it rather than that the fixture was pre-stamped.
        closed: Closed::Main,
    }
}

/// **The duplicate the author reported, at the wire boundary.**
///
/// `speech` is declared `ifClosed=''`, which the protocol wiki calls a
/// duplicate the server also sends to main. The viewer cannot know that on
/// its own, so the pump stamps it -- and this test is what says the stamp
/// reaches the bytes a browser receives, not merely the model.
#[test]
fn a_published_line_carries_what_its_stream_does_when_closed() {
    let mut hub = Hub::new();
    let mut viewers = hub.updates.subscribe();
    hub.publish(&declared_speech(1), vec![line("speech"), line("")], false)
        .unwrap();
    let wire: serde_json::Value = serde_json::from_str(&viewers.try_recv().unwrap()).unwrap();
    let lines = wire["lines"].as_array().expect("an update carries lines");
    assert_eq!(
        lines[0]["closed"]["kind"], "drop",
        "the speech copy is a duplicate: {:?}",
        lines[0]["closed"]
    );
    assert_eq!(
        lines[1]["closed"]["kind"], "main",
        "the main-window copy is the one that shows"
    );
}

/// An undeclared stream falls through to main rather than being hidden.
///
/// The unsafe direction is dropping text on a guess: MEASURED, 16 stream
/// ids are declared against 8 ever pushed to, and that set is not closed,
/// so an unknown stream is likelier to be one this build has not seen.
#[test]
fn an_undeclared_stream_is_published_as_main() {
    let mut hub = Hub::new();
    let mut viewers = hub.updates.subscribe();
    hub.publish(&snapshot(1), vec![line("percWindow")], false)
        .unwrap();
    let wire: serde_json::Value = serde_json::from_str(&viewers.try_recv().unwrap()).unwrap();
    assert_eq!(wire["lines"][0]["closed"]["kind"], "main");
}

#[test]
fn subscription_and_cache_share_a_presentation_sequence() {
    let mut hub = Hub::new();
    hub.publish(&snapshot(99), Vec::new(), false).unwrap();
    let first: serde_json::Value = serde_json::from_str(hub.snapshot.as_ref().unwrap()).unwrap();
    assert_eq!(first["cursor"], "1");
    let mut events = hub.updates.subscribe();
    // With a line: an identical refresh publishes nothing at all (see
    // `a_refresh_that_changes_nothing_publishes_nothing`).
    hub.publish(&snapshot(99), vec![line("")], false).unwrap();
    let next: serde_json::Value = serde_json::from_str(&events.try_recv().unwrap()).unwrap();
    assert_eq!(next["cursor"], "2");
    assert_eq!(next["kind"], "update");
}

#[test]
fn reconnecting_before_retry_decision_keeps_unknown_attempt_and_delay() {
    let mut reconnecting = snapshot(1);
    reconnecting.lifecycle = State::Reconnecting;
    assert_eq!(
        lifecycle(&reconnecting),
        LifecycleView::Reconnecting {
            attempt: None,
            retry_delay_ms: None,
            detail: None,
        }
    );
    reconnecting.retry = Some(cena_session::RetryStatus {
        attempt: 2,
        delay: Duration::from_secs(5),
        detail: "fixture retry".into(),
    });
    assert_eq!(
        lifecycle(&reconnecting),
        LifecycleView::Reconnecting {
            attempt: Some(2),
            retry_delay_ms: Some(5000),
            detail: Some("fixture retry".into()),
        }
    );
}

#[test]
fn missing_fence_clears_partial_lines_and_marks_snapshot_gap() {
    let mut pending = Pending::new(&snapshot(0));
    pending.assembler.push(
        "",
        &StyledRun {
            text: "incomplete".into(),
            ..StyledRun::default()
        },
        false,
    );
    let (_sender, mut events) = broadcast::channel(1);
    pending.fence(&snapshot(3), &mut events);
    assert!(pending.gap);
    assert!(pending.assembler.flush().is_empty());
    let mut hub = Hub::new();
    let mut changes = hub.updates.subscribe();
    hub.publish(&snapshot(3), Vec::new(), pending.gap).unwrap();
    let wire: serde_json::Value = serde_json::from_str(&changes.try_recv().unwrap()).unwrap();
    assert_eq!(wire["kind"], "snapshot");
    assert_eq!(wire["history_gap"], true);
}

#[test]
fn fence_consumes_exactly_the_native_snapshot_prefix() {
    let mut pending = Pending::new(&snapshot(0));
    let (sender, mut events) = broadcast::channel(8);
    for cursor in 1..=3 {
        sender
            .send(ObservedEvent {
                session: SessionId::FIRST,
                generation: Generation::FIRST,
                cursor,
                event: Event::Frame(Box::new(Frame::Prompt {
                    time: cursor.to_string(),
                    text: ">".into(),
                })),
            })
            .unwrap();
    }
    pending.fence(&snapshot(2), &mut events);
    assert_eq!(pending.cursor, 2);
    assert!(!pending.gap);
    assert_eq!(events.try_recv().unwrap().cursor, 3);
}

#[test]
fn changed_generation_discards_unfinished_old_text() {
    let mut pending = Pending::new(&snapshot(0));
    pending.assembler.push(
        "",
        &StyledRun {
            text: "old".into(),
            ..StyledRun::default()
        },
        false,
    );
    pending.observe(ObservedEvent {
        session: SessionId::FIRST,
        generation: Generation(1),
        cursor: 1,
        event: Event::Frame(Box::new(Frame::Prompt {
            time: "1".into(),
            text: ">".into(),
        })),
    });
    assert!(pending.lines.is_empty());
}

#[test]
fn serialization_refuses_oversized_presentation_messages() {
    let message = ServerMessage::Receipt {
        version: WIRE_VERSION,
        session: "0".into(),
        generation: "0".into(),
        request_id: "1".into(),
        status: cena_ui::ReceiptStatus::Uncertain,
        detail: "x".repeat(MAX_WIRE_BYTES),
    };
    assert!(encode(&message).is_err());
}

#[test]
fn history_and_slow_viewers_are_bounded() {
    let mut hub = Hub::new();
    let mut slow = hub.updates.subscribe();
    for _ in 0..300 {
        hub.publish(
            &snapshot(0),
            vec![StoryLine {
                stream: String::new(),
                runs: vec![StyledRun {
                    text: "x".repeat(1024),
                    ..StyledRun::default()
                }],
                truncated: false,
                closed: Closed::Main,
            }],
            false,
        )
        .unwrap();
    }
    assert!(hub.history_bytes <= MAX_HISTORY_BYTES);
    assert!(hub.history.len() <= MAX_HISTORY_LINES);
    assert!(
        !hub.gap_retained(),
        "eviction is bounded history, not a hole"
    );
    assert!(matches!(
        slow.try_recv(),
        Err(broadcast::error::TryRecvError::Lagged(_))
    ));
}

fn story_line(text: &str) -> StoryLine {
    StoryLine {
        stream: String::new(),
        runs: vec![StyledRun {
            text: text.into(),
            ..StyledRun::default()
        }],
        truncated: false,
        closed: Closed::Main,
    }
}

/// **The 10 Hz republish.** While a roundtime ran, the pump refreshed
/// every 100 ms and each refresh became a new cursor, a full re-encode and
/// a broadcast -- and in the browser a pane rebuild that snapped scroll to
/// the bottom. A refresh that changes nothing must send nothing.
#[test]
fn a_refresh_that_changes_nothing_publishes_nothing() {
    let mut hub = Hub::new();
    let mut viewers = hub.updates.subscribe();
    hub.publish(&snapshot(1), Vec::new(), false).unwrap();
    assert!(viewers.try_recv().is_ok(), "the first publish is sent");
    let cached = hub.snapshot.clone();
    for cursor in 2..12 {
        hub.publish(&snapshot(cursor), Vec::new(), false).unwrap();
    }
    assert!(
        matches!(
            viewers.try_recv(),
            Err(broadcast::error::TryRecvError::Empty)
        ),
        "ten identical refreshes are not ten messages"
    );
    assert_eq!(hub.sequence, 1, "and not ten presentation cursors");
    assert_eq!(hub.snapshot, cached, "nor a re-encoded cache");

    // What DOES change is still sent: the view, or a line.
    let mut changed = snapshot(12);
    changed.state.prompt = Some("R>".into());
    hub.publish(&changed, Vec::new(), false).unwrap();
    assert!(viewers.try_recv().is_ok(), "a changed view is sent");
    hub.publish(&changed, vec![story_line("You swing.")], false)
        .unwrap();
    assert!(viewers.try_recv().is_ok(), "a new line is sent");
}

/// **The banner that never went away.** The gap flag was set by ordinary
/// eviction and never cleared, so after 256 lines every viewer was told
/// history was missing for the rest of the session.
#[test]
fn eviction_is_not_a_gap_and_a_hole_is_reported_only_while_retained() {
    let mut hub = Hub::new();
    for index in 0..(MAX_HISTORY_LINES + 50) {
        hub.publish(&snapshot(0), vec![story_line(&index.to_string())], false)
            .unwrap();
    }
    let cached = |hub: &Hub| -> serde_json::Value {
        serde_json::from_str(hub.snapshot.as_ref().unwrap()).unwrap()
    };
    assert_eq!(
        cached(&hub)["history_gap"],
        false,
        "a bounded history dropping its oldest lines has lost nothing between them"
    );

    // A real hole: lines lost to lag, between what is retained and what
    // follows. Connected viewers are told, and so is anyone attaching
    // while text still precedes the hole.
    let mut viewers = hub.updates.subscribe();
    hub.publish(&snapshot(0), vec![story_line("after the hole")], true)
        .unwrap();
    let resync: serde_json::Value = serde_json::from_str(&viewers.try_recv().unwrap()).unwrap();
    assert_eq!(resync["kind"], "snapshot");
    assert_eq!(resync["history_gap"], true);
    assert_eq!(cached(&hub)["history_gap"], true);

    // Once eviction has carried every line before the hole away, the hole
    // is at the front -- indistinguishable from bounded history -- and the
    // claim is no longer true of what a new viewer receives.
    for index in 0..MAX_HISTORY_LINES {
        hub.publish(&snapshot(0), vec![story_line(&index.to_string())], false)
            .unwrap();
    }
    assert_eq!(cached(&hub)["history_gap"], false);
}

/// A hole with nothing retained before it is still news to whoever was
/// connected, but not to a viewer that attaches afterwards.
#[test]
fn a_hole_at_the_front_is_told_to_connected_viewers_only() {
    let mut hub = Hub::new();
    let mut viewers = hub.updates.subscribe();
    hub.publish(&snapshot(3), vec![story_line("after")], true)
        .unwrap();
    let resync: serde_json::Value = serde_json::from_str(&viewers.try_recv().unwrap()).unwrap();
    assert_eq!(resync["history_gap"], true);
    let cached: serde_json::Value = serde_json::from_str(hub.snapshot.as_ref().unwrap()).unwrap();
    assert_eq!(cached["history_gap"], false);
}

/// **JSON is not 1:1 with text.** A control character encodes as
/// `\u00XX`, six bytes for one, so a history held to its byte budget in
/// raw text encoded past `MAX_WIRE_BYTES` -- and that stopped the server.
#[test]
fn escaped_text_cannot_push_a_snapshot_past_the_wire_cap() {
    let mut hub = Hub::new();
    let hostile = "\u{1}".repeat(1024);
    for _ in 0..MAX_HISTORY_LINES {
        hub.publish(&snapshot(0), vec![story_line(&hostile)], false)
            .expect("a publish is never fatal for its size");
    }
    let cached = hub.snapshot.as_ref().unwrap();
    assert!(cached.len() <= MAX_WIRE_BYTES);
    let story = serde_json::to_string(&hub.history).unwrap();
    assert!(
        story.len() <= hub.history_bytes,
        "the budget is an upper bound on the encoding: {} > {}",
        story.len(),
        hub.history_bytes
    );
    assert!(story.len() <= MAX_HISTORY_BYTES);
}

/// A view too large for one message degrades to unknowns; it does not
/// stop the server and disconnect every viewer.
#[test]
fn a_view_too_large_to_send_is_sent_as_unknown() {
    let mut hub = Hub::new();
    let mut viewers = hub.updates.subscribe();
    let mut giant = snapshot(1);
    giant.state.room.title = Some("x".repeat(MAX_WIRE_BYTES));
    hub.publish(&giant, vec![story_line("still delivered")], false)
        .expect("an oversized view is degraded, not fatal");
    let wire: serde_json::Value = serde_json::from_str(&viewers.try_recv().unwrap()).unwrap();
    assert_eq!(wire["view"]["room"]["title"], serde_json::Value::Null);
    assert_eq!(wire["lines"][0]["runs"][0]["text"], "still delivered");
}

fn scripted(
    script: Vec<Result<(), ObserveError>>,
    events: broadcast::Sender<ObservedEvent>,
) -> (
    Arc<std::sync::atomic::AtomicUsize>,
    impl FnMut() -> std::future::Ready<Result<Subscription, ObserveError>>,
) {
    let calls = Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let counter = Arc::clone(&calls);
    let subscribe = move || {
        let at = counter.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        let step = script.get(at).copied().unwrap_or(Ok(()));
        std::future::ready(step.map(|()| (snapshot(0), events.subscribe())))
    };
    (calls, subscribe)
}

fn state_changed(events: &broadcast::Sender<ObservedEvent>) {
    let _ = events.send(ObservedEvent {
        session: SessionId::FIRST,
        generation: Generation::FIRST,
        cursor: 1,
        event: Event::StateChanged(State::Ready),
    });
}

/// **`Busy` and `Timeout` are not an ending.** They were mapped to a
/// fatal error by `?`, which returned from `WebServer::run` and
/// disconnected every viewer over a full inbox or a slow answer --
/// failures `cena_session::observation` documents as retryable.
#[tokio::test(start_paused = true)]
async fn a_busy_or_slow_owner_is_retried_and_the_viewer_stays_up() {
    let shared = Shared::for_test();
    let (events, _keep) = broadcast::channel(8);
    let (calls, subscribe) = scripted(
        vec![
            Err(ObserveError::Busy),
            Ok(()),
            Err(ObserveError::Busy),
            Err(ObserveError::Timeout),
            Ok(()),
        ],
        events.clone(),
    );
    let pump = tokio::spawn(project(subscribe, Arc::clone(&shared)));
    tokio::time::sleep(Duration::from_secs(1)).await;
    assert_eq!(calls.load(std::sync::atomic::Ordering::SeqCst), 2);
    state_changed(&events);
    tokio::time::sleep(Duration::from_secs(5)).await;
    assert_eq!(calls.load(std::sync::atomic::Ordering::SeqCst), 5);
    assert!(
        !pump.is_finished(),
        "still serving after three retryable failures"
    );
    shared.stop.cancel();
    assert!(pump.await.unwrap().is_ok());
}

/// `Closed` -- the owner gone without a final snapshot -- is the one
/// answer that ends observation.
#[tokio::test(start_paused = true)]
async fn an_owner_that_is_gone_ends_the_pump() {
    let shared = Shared::for_test();
    let (events, _keep) = broadcast::channel(8);
    let (calls, subscribe) = scripted(vec![Ok(()), Err(ObserveError::Closed)], events.clone());
    let pump = tokio::spawn(project(subscribe, Arc::clone(&shared)));
    tokio::time::sleep(Duration::from_millis(10)).await;
    state_changed(&events);
    let result = tokio::time::timeout(Duration::from_secs(1), pump)
        .await
        .expect("Closed is not retried")
        .unwrap();
    assert!(result.is_err());
    assert_eq!(calls.load(std::sync::atomic::Ordering::SeqCst), 2);
}
