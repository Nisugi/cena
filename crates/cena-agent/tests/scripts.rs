//! A script runner's listener (`plan/46`, M7b step 1), over MCP against a
//! scripted game: the runner hears the game's lines as it sent them, a
//! squelched one too; its line goes out as the script's and answers the
//! cursor its `sent` carries; a line marked as Hydra's is Hydra's; what it
//! says reaches the player; the player's command reaches it; and only its
//! own token opens the listener, until it is dismissed.

use std::sync::Arc;
use std::time::Duration;

use axum::body::Body;
use axum::http::{Request, StatusCode};
use cena_agent::scripts::{Runners, router};
use cena_platform::AnsweringSource;
use cena_session::command::claimant::{Claimed, Desk, Runner};
use cena_session::trigger::{Matcher, Pattern, Rule, Trigger};
use cena_session::{Body as Said, Event, Session};
use http_body_util::BodyExt;
use tokio_util::sync::CancellationToken;
use tower::ServiceExt;

fn request(token: &str, body: &serde_json::Value) -> Request<Body> {
    Request::post("/mcp")
        .header("host", "127.0.0.1")
        .header("content-type", "application/json")
        .header("accept", "application/json, text/event-stream")
        .header("mcp-protocol-version", "2025-06-18")
        .header("authorization", format!("Bearer {token}"))
        .body(Body::from(body.to_string()))
        .unwrap_or_default()
}

/// A tool's JSON answer, or the HTTP status when there was none.
async fn call(
    app: &axum::Router,
    token: &str,
    name: &str,
    arguments: serde_json::Value,
) -> Result<serde_json::Value, StatusCode> {
    let body = serde_json::json!({"jsonrpc": "2.0", "id": 1, "method": "tools/call",
                                  "params": {"name": name, "arguments": arguments}});
    let response = app
        .clone()
        .oneshot(request(token, &body))
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    let status = response.status();
    let bytes = response
        .into_body()
        .collect()
        .await
        .map_err(|_| status)?
        .to_bytes();
    let reply: serde_json::Value = serde_json::from_slice(&bytes).map_err(|_| status)?;
    let text = reply
        .pointer("/result/content/0/text")
        .and_then(|t| t.as_str())
        .ok_or(status)?;
    serde_json::from_str(text).map_err(|_| status)
}

/// Every event `listen` gives from the start until `done` holds of them.
async fn heard_until(
    app: &axum::Router,
    token: &str,
    done: impl Fn(&[serde_json::Value]) -> bool,
) -> Option<Vec<serde_json::Value>> {
    let mut since = 0;
    let mut events = Vec::new();
    for _ in 0..20 {
        let heard = call(
            app,
            token,
            "listen",
            serde_json::json!({"since": since, "timeout_ms": 2000}),
        )
        .await
        .ok()?;
        events.extend(heard["events"].as_array().cloned().unwrap_or_default());
        since = heard["next"].as_u64()?;
        if done(&events) {
            break;
        }
    }
    Some(events)
}

fn kind<'a>(events: &'a [serde_json::Value], kind: &str) -> Vec<&'a serde_json::Value> {
    events.iter().filter(|e| e["kind"] == kind).collect()
}

/// A running scripted character whose `look` is answered, with a command
/// table that knows `;known`, and a trigger squelching `You see`.
fn squelching_character() -> Option<(
    cena_session::SessionHandle,
    cena_platform::TranscriptHandle,
    cena_session::SessionObserver,
    tokio::sync::broadcast::Receiver<Event>,
)> {
    let (source, transcript) = AnsweringSource::logged_in(b"<prompt time=\"1\">&gt;</prompt>\n");
    transcript.answer(
        "look",
        b"You see a quiet room.\n<prompt time=\"5\">&gt;</prompt>\n",
    );
    let session = Session::new(source);
    let handle = session.handle();
    let runner: Runner = Arc::new(|line: &str| {
        if line.trim() == "known" {
            Claimed::Done
        } else {
            Claimed::Unknown
        }
    });
    handle.set_desk(Desk::new(None, runner)).then_some(())?;
    handle.set_triggers(
        Matcher::new(vec![Trigger {
            name: "hide".into(),
            rule: Rule {
                pattern: Some(Pattern::Literal {
                    text: "You see".into(),
                    whole_word: true,
                }),
                squelch: true,
                ..Rule::default()
            },
        }])
        .ok()?,
    );
    let observer = session.observer();
    let (_, legacy) = session.subscribe();
    tokio::spawn(session.into_actor().run());
    Some((handle, transcript, observer, legacy))
}

#[tokio::test(flavor = "current_thread")]
async fn a_runner_hears_its_character_and_acts_as_a_script() {
    let (handle, transcript, observer, mut legacy) = squelching_character().unwrap();

    let runners = Runners::default();
    let token = runners
        .admit("Nisugi", handle.script_door(), &observer)
        .await
        .unwrap();
    let stop = CancellationToken::new();
    let app = router(runners.clone(), &stop);

    let sent = call(&app, &token, "send", serde_json::json!({"line": "look"}))
        .await
        .unwrap();
    assert_eq!(sent["outcome"], "sent", "{sent}");
    let cursor = sent["cursor"].as_u64().unwrap();
    let events = heard_until(&app, &token, |events| {
        !kind(events, "prompt").is_empty() && !kind(events, "line").is_empty()
    })
    .await
    .unwrap();
    let mine = kind(&events, "sent");
    assert_eq!(mine.len(), 1, "{events:?}");
    assert_eq!(mine[0]["cursor"].as_u64(), Some(cursor));
    assert_eq!(mine[0]["origin"], "script");
    let lines = kind(&events, "line");
    assert_eq!(
        lines[0]["text"], "You see a quiet room.",
        "squelched, heard"
    );
    assert!(
        lines[0]["cursor"].as_u64().unwrap() > cursor,
        "after the send"
    );
    let prompts = kind(&events, "prompt");
    assert_eq!(prompts.last().unwrap()["time"], 5);

    for (line, outcome) in [(";known", "ran"), (";nosuch", "unknown")] {
        let answer = call(&app, &token, "send", serde_json::json!({"line": line}))
            .await
            .unwrap();
        assert_eq!(answer["outcome"], outcome, "{line}");
    }
    assert_eq!(
        transcript.lines(),
        ["look"],
        "only the game's line was sent"
    );

    let said = call(
        &app,
        &token,
        "say",
        serde_json::json!({"text": "[trollspeak: hi]", "mono": true}),
    )
    .await
    .unwrap();
    assert_eq!(said["said"], true);
    let told = std::iter::from_fn(|| legacy.try_recv().ok()).find_map(|event| match event {
        Event::Notice(notice) => Some(notice.body),
        _ => None,
    });
    assert_eq!(told, Some(Said::Mono(vec!["[trollspeak: hi]".to_owned()])));

    assert!(runners.typed(&token, "trollspeak say hi"));
    let events = heard_until(&app, &token, |events| !kind(events, "typed").is_empty())
        .await
        .unwrap();
    assert_eq!(kind(&events, "typed")[0]["line"], "trollspeak say hi");

    assert_eq!(
        call(&app, "not-the-token", "listen", serde_json::json!({})).await,
        Err(StatusCode::UNAUTHORIZED)
    );
    runners.dismiss(&token);
    assert!(!runners.typed(&token, "k trollspeak"));
    assert_eq!(
        call(&app, &token, "listen", serde_json::json!({})).await,
        Err(StatusCode::UNAUTHORIZED),
        "a dismissed runner's token opens nothing"
    );
    stop.cancel();
}

/// While nobody listens, the character publishes each line once; a runner
/// turns heard lines on, and its dismissal turns them off.
#[tokio::test(flavor = "current_thread")]
async fn heard_lines_last_as_long_as_a_runner() {
    let (source, transcript) = AnsweringSource::logged_in(b"<prompt time=\"1\">&gt;</prompt>\n");
    for _ in 0..2 {
        transcript.answer("look", b"A room.\n<prompt time=\"2\">&gt;</prompt>\n");
    }
    let session = Session::new(source);
    let handle = session.handle();
    let observer = session.observer();
    let (_, mut legacy) = session.subscribe();
    tokio::spawn(session.into_actor().run());
    let heard = |legacy: &mut tokio::sync::broadcast::Receiver<Event>| {
        std::iter::from_fn(|| legacy.try_recv().ok())
            .filter(|event| matches!(event, Event::Heard(_)))
            .count()
    };

    let runners = Runners::default();
    let token = runners
        .admit("Nisugi", handle.script_door(), &observer)
        .await
        .unwrap();
    handle
        .send_manual_at(handle.generation(), "look", Duration::from_secs(5))
        .await;
    assert_eq!(heard(&mut legacy), 1);
    runners.dismiss(&token);
    handle
        .send_manual_at(handle.generation(), "look", Duration::from_secs(5))
        .await;
    assert_eq!(heard(&mut legacy), 0);
}
