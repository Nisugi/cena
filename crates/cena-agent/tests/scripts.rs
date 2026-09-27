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

/// A stand-in for the binary's performer: `go2 bank` arrives at once; `go2
/// far` walks until it is stopped; anything else is not a built-in.
fn walker() -> cena_session::operation::Performer {
    use cena_session::operation::{Ended, Performer, Started, Work};
    let allows: cena_session::operation::Allows = Arc::new(|line: &str| {
        if line.starts_with("go2 ") {
            Ok(line.to_owned())
        } else {
            Err(format!("`{line}` is not a built-in"))
        }
    });
    let start: cena_session::operation::Start = Arc::new(|line: &str, _reporter| {
        let stopped = Arc::new(tokio::sync::Notify::new());
        let far = line == "go2 far";
        let heard = Arc::clone(&stopped);
        Started {
            ended: Box::pin(async move {
                if far {
                    heard.notified().await;
                    Ended::plainly(Work::Interrupted, "stopped")
                } else {
                    Ended::plainly(Work::Completed, "arrived")
                }
            }),
            steer: Arc::new(move |_| {
                stopped.notify_one();
                Ok(())
            }),
            token: None,
        }
    });
    Performer {
        allowed: "go2".to_owned(),
        allows,
        start,
        halt: Arc::new(|| {}),
    }
}

/// A runner starts a built-in by name and hears it end; it stops one that
/// would go on; what Hydra does not run, and a session whose behaviors are
/// not ready, are refused in words.
#[tokio::test(flavor = "current_thread")]
async fn a_runner_starts_a_built_in_and_hears_it_end() {
    let (source, _transcript) = AnsweringSource::logged_in(b"<prompt time=\"1\">&gt;</prompt>\n");
    let session = Session::new(source);
    let handle = session.handle();
    let observer = session.observer();
    tokio::spawn(session.into_actor().run());
    let runners = Runners::default();
    let token = runners
        .admit("Nisugi", handle.script_door(), &observer)
        .await
        .unwrap();
    let app = router(runners.clone(), &CancellationToken::new());

    let early = call(
        &app,
        &token,
        "perform",
        serde_json::json!({"line": "go2 bank"}),
    )
    .await
    .unwrap();
    assert!(
        early["refused"].as_str().unwrap().contains("not ready"),
        "{early}"
    );
    assert!(handle.set_performer(walker()));

    let bank = call(
        &app,
        &token,
        "perform",
        serde_json::json!({"line": "go2 bank"}),
    )
    .await
    .unwrap();
    assert_eq!(bank["line"], "go2 bank");
    let first = bank["run"].as_u64().unwrap();
    let events = heard_until(&app, &token, |events| !kind(events, "ended").is_empty())
        .await
        .unwrap();
    let ended = kind(&events, "ended")[0];
    assert_eq!(
        (ended["run"].as_u64(), &ended["work"], &ended["reason"]),
        (
            Some(first),
            &serde_json::json!("completed"),
            &serde_json::json!("arrived")
        )
    );

    let far = call(
        &app,
        &token,
        "perform",
        serde_json::json!({"line": "go2 far"}),
    )
    .await
    .unwrap();
    let second = far["run"].as_u64().unwrap();
    assert_ne!(first, second);
    let stopping = call(&app, &token, "stop", serde_json::json!({"run": second}))
        .await
        .unwrap();
    assert_eq!(stopping["stopping"], true);
    let events = heard_until(&app, &token, |events| {
        kind(events, "ended")
            .iter()
            .any(|e| e["run"].as_u64() == Some(second))
    })
    .await
    .unwrap();
    let ended = kind(&events, "ended")
        .into_iter()
        .find(|e| e["run"].as_u64() == Some(second))
        .unwrap();
    assert_eq!(ended["reason"], "stopped");

    let gone = call(&app, &token, "stop", serde_json::json!({"run": first}))
        .await
        .unwrap();
    assert!(gone["refused"].as_str().unwrap().contains("not under way"));
    let hunt = call(
        &app,
        &token,
        "perform",
        serde_json::json!({"line": "hunt x"}),
    )
    .await
    .unwrap();
    assert!(hunt["refused"].as_str().unwrap().contains("not a built-in"));
}

/// A scripted character warmed up, and a runner admitted for it with
/// display and input hooks.
struct Hooked {
    handle: cena_session::SessionHandle,
    transcript: cena_platform::TranscriptHandle,
    legacy: tokio::sync::broadcast::Receiver<Event>,
    runners: Runners,
    token: String,
    app: axum::Router,
}

async fn hooked() -> Option<Hooked> {
    let (source, transcript) = AnsweringSource::logged_in(b"<prompt time=\"1\">&gt;</prompt>\n");
    transcript.answer(
        "glance",
        b"You glance about.\n<prompt time=\"4\">&gt;</prompt>\n",
    );
    transcript.answer(
        "look",
        b"You see a quiet room.\nA breeze blows.\n<prompt time=\"5\">&gt;</prompt>\n",
    );
    let session = Session::new(source);
    let handle = session.handle();
    let observer = session.observer();
    let (_, legacy) = session.subscribe();
    tokio::spawn(session.into_actor().run());
    // The first chunk of text builds the session's classifiers, which a
    // debug build takes past the hooks' half second over: not what these
    // tests are about.
    handle
        .send_manual_at(handle.generation(), "glance", Duration::from_secs(5))
        .await;
    let runners = Runners::default();
    let token = runners
        .admit("Nisugi", handle.script_door(), &observer)
        .await
        .ok()?;
    let app = router(runners.clone(), &CancellationToken::new());
    let hooks = call(
        &app,
        &token,
        "hooks",
        serde_json::json!({"display": true, "input": true}),
    )
    .await
    .ok()?;
    (hooks["display"] == true).then_some(())?;
    Some(Hooked {
        handle,
        transcript,
        legacy,
        runners,
        token,
        app,
    })
}

/// The lines shown since last asked.
fn shown(legacy: &mut tokio::sync::broadcast::Receiver<Event>) -> Vec<String> {
    std::iter::from_fn(|| legacy.try_recv().ok())
        .filter_map(|event| match event {
            Event::Line(line) => Some(line.text()),
            _ => None,
        })
        .collect()
}

/// A runner's display hooks, over MCP: each line waits to be shown until
/// `shown` answers it by its cursor, hidden or changed.
#[tokio::test(flavor = "current_thread")]
async fn a_runners_display_hooks_answer_what_is_shown() {
    let Hooked {
        handle,
        mut legacy,
        token,
        app,
        ..
    } = hooked().await.unwrap();
    handle
        .send_manual_at(handle.generation(), "look", Duration::from_secs(5))
        .await;
    let events = heard_until(&app, &token, |events| kind(events, "line").len() == 2)
        .await
        .unwrap();
    let lines = kind(&events, "line");
    assert_eq!(
        shown(&mut legacy),
        ["You glance about."],
        "the look held for the hooks"
    );
    let answered = call(
        &app,
        &token,
        "shown",
        serde_json::json!({"lines": [
            {"cursor": lines[0]["cursor"], "text": null},
            {"cursor": lines[1]["cursor"], "text": "A gale blows."},
        ]}),
    )
    .await
    .unwrap();
    assert_eq!(answered["answered"], 2);
    let mut seen = Vec::new();
    for _ in 0..50 {
        seen.extend(shown(&mut legacy));
        if !seen.is_empty() {
            break;
        }
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
    assert_eq!(seen, ["A gale blows."]);
}

/// A runner's input hooks, over MCP: the player's line comes as `input`
/// and goes as answered. Dismissed, the runner is asked nothing.
#[tokio::test(flavor = "current_thread")]
async fn a_runners_input_hooks_answer_what_is_typed() {
    let Hooked {
        handle,
        transcript,
        runners,
        token,
        app,
        ..
    } = hooked().await.unwrap();
    let typing = handle.clone();
    let typed = tokio::spawn(async move {
        typing
            .send_typed_at(typing.generation(), "tt", Duration::from_secs(5))
            .await
    });
    let events = heard_until(&app, &token, |events| !kind(events, "input").is_empty())
        .await
        .unwrap();
    let input = kind(&events, "input")[0];
    assert_eq!(input["line"], "tt");
    let answer = call(
        &app,
        &token,
        "input",
        serde_json::json!({"asked": input["asked"], "line": "target"}),
    )
    .await
    .unwrap();
    assert_eq!(answer["late"], false);
    typed.await.unwrap();
    assert_eq!(transcript.lines(), ["glance", "target"]);

    runners.dismiss(&token);
    let typed_at = std::time::Instant::now();
    handle
        .send_typed_at(handle.generation(), "look", Duration::from_secs(5))
        .await;
    assert!(
        typed_at.elapsed() < cena_session::script::HOOK_DEADLINE,
        "not asked of hooks that went with the runner"
    );
    assert_eq!(transcript.lines(), ["glance", "target", "look"]);
}
