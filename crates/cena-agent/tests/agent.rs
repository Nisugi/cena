//! The agent (`plan/35` §8): the projection carries every status the model
//! holds and nothing it does not; `wait` is the difference between
//! snapshots; the whole path works over MCP, behind the token, against a
//! scripted game (step 1); and each character's level decides what a tool
//! may do, with the player asked about an act above it (step 2).

use std::time::Duration;

use axum::body::Body;
use axum::http::{Request, StatusCode};
use cena_agent::Characters;
use cena_agent::happenings::{Happening, Log, diff};
use cena_agent::projection::project;
use cena_platform::AnsweringSource;
use cena_session::agent::Level;
use cena_session::{Event, GameState, Generation, Session, SessionId, Snapshot, State};
use http_body_util::BodyExt;
use tokio_util::sync::CancellationToken;
use tower::ServiceExt;

fn snapshot(state: GameState, cursor: u64) -> Snapshot {
    Snapshot {
        session: SessionId(1),
        state,
        lifecycle: State::Ready,
        generation: Generation(0),
        cursor,
        retry: None,
    }
}

/// `plan/35` §5: the projection passes the status store through whole, so a
/// status the model learns reaches the agent with no edit here. Indicators,
/// poisoned and diseased, and the text-only afflictions all live in the one
/// store; every id written there is in the projection with its value, and
/// an id never written is absent -- unknown, not false.
#[test]
fn every_status_the_model_holds_is_projected_and_unknown_is_absent() {
    let mut state = GameState::default();
    let written = [
        ("IconSTANDING", true),
        ("IconSTUNNED", false),
        ("IconHIDDEN", true),
        ("IconDEAD", false),
        ("IconBLEEDING", true),
        ("poisoned", true),
        ("diseased", false),
        ("calmed", true),
        ("silenced", false),
    ];
    for (id, on) in written {
        state.status.set(id, on);
    }
    let projected = project("Nisugi", &snapshot(state.clone(), 7));
    let held: Vec<(String, bool)> = state
        .status
        .iter()
        .map(|(id, on)| (id.to_owned(), on))
        .collect();
    let shown: Vec<(String, bool)> = projected
        .statuses
        .iter()
        .map(|(id, on)| (id.clone(), *on))
        .collect();
    assert_eq!(shown, held, "the store, whole");
    assert_eq!(projected.statuses.len(), written.len());
    assert!(!projected.statuses.contains_key("webbed"), "never reported");
    assert_eq!(projected.cursor, 7);

    let mut reconnected = state;
    reconnected.status.clear();
    assert!(
        project("Nisugi", &snapshot(reconnected, 8))
            .statuses
            .is_empty(),
        "after a reconnect nothing is known, and nothing is claimed"
    );
}

#[test]
fn the_difference_between_two_states_is_what_happened() {
    let mut before = GameState::default();
    before.status.set("stunned", false);
    before.status.set("hidden", true);
    let mut after = before.clone();
    after.status.set("stunned", true);
    after.status.forget("hidden");
    let happened = diff(
        &project("Nisugi", &snapshot(before.clone(), 1)),
        &project("Nisugi", &snapshot(after, 2)),
    );
    assert!(happened.contains(&Happening::Status {
        id: "stunned".to_owned(),
        now: Some(true)
    }));
    assert!(
        happened.contains(&Happening::Status {
            id: "hidden".to_owned(),
            now: None
        }),
        "known, then unknown: said as unknown, not as off"
    );

    let mut moved = before.clone();
    moved.room.id = Some("228".to_owned());
    let happened = diff(
        &project("Nisugi", &snapshot(before, 1)),
        &project("Nisugi", &snapshot(moved, 2)),
    );
    assert!(matches!(
        happened.as_slice(),
        [Happening::Moved { to: Some(to), .. }] if to == "228"
    ));
}

/// `wait` returns what came after the cursor, waits for the first when there
/// is none, says `lagged` when the cursor fell out of the log, and filters by
/// kind.
#[tokio::test(start_paused = true)]
async fn wait_answers_from_the_log_by_cursor() {
    let log = std::sync::Arc::new(Log::default());
    log.push(5, Happening::Gap);
    let waited = log.wait(0, None, Duration::from_secs(1)).await;
    assert_eq!(waited.happenings.len(), 1);
    assert_eq!(waited.cursor, 5);

    let later = std::sync::Arc::clone(&log);
    let waiting = tokio::spawn(async move { later.wait(5, None, Duration::from_secs(10)).await });
    tokio::time::sleep(Duration::from_millis(100)).await;
    log.push(
        9,
        Happening::Notice {
            text: "hi".to_owned(),
        },
    );
    let waited = waiting.await.unwrap();
    assert_eq!(waited.happenings.len(), 1, "woken by the push");
    assert_eq!(waited.cursor, 9);

    let none = log.wait(9, None, Duration::from_secs(2)).await;
    assert!(
        none.happenings.is_empty() && !none.lagged,
        "a timeout is empty"
    );

    let notices = log
        .wait(0, Some(&["notice".to_owned()]), Duration::ZERO)
        .await;
    assert_eq!(notices.happenings.len(), 1, "only notices");

    for n in 0..=cena_agent::happenings::KEPT as u64 {
        log.push(10 + n, Happening::Gap);
    }
    assert!(log.wait(0, None, Duration::ZERO).await.lagged);
}

/// A body's JSON-RPC answer: plain JSON, or the `data:` of a stream.
async fn answer(response: axum::response::Response) -> Option<serde_json::Value> {
    let bytes = response.into_body().collect().await.ok()?.to_bytes();
    let text = String::from_utf8(bytes.to_vec()).ok()?;
    serde_json::from_str(&text).ok().or_else(|| {
        text.lines()
            .filter_map(|line| line.strip_prefix("data:"))
            .filter_map(|data| serde_json::from_str::<serde_json::Value>(data.trim()).ok())
            .find(|value| value.get("result").is_some() || value.get("error").is_some())
    })
}

fn request(token: Option<&str>, session: Option<&str>, body: &serde_json::Value) -> Request<Body> {
    let mut builder = Request::post("/mcp")
        .header("host", "127.0.0.1")
        .header("content-type", "application/json")
        .header("accept", "application/json, text/event-stream")
        .header("mcp-protocol-version", "2025-06-18");
    if let Some(token) = token {
        builder = builder.header("authorization", format!("Bearer {token}"));
    }
    if let Some(session) = session {
        builder = builder.header("mcp-session-id", session);
    }
    builder
        .body(Body::from(body.to_string()))
        .unwrap_or_default()
}

/// A tool's JSON answer, from a `tools/call` result's first text block.
fn tool_json(answer: &serde_json::Value) -> Option<serde_json::Value> {
    let text = answer.pointer("/result/content/0/text")?.as_str()?;
    serde_json::from_str(text).ok()
}

/// A connected MCP client: the router, and the session it was given.
struct Client {
    router: axum::Router,
    session: Option<String>,
}

impl Client {
    /// `initialize` and `initialized`, with the token.
    async fn connect(router: axum::Router, token: &str) -> Option<Self> {
        let initialize = serde_json::json!({
            "jsonrpc": "2.0", "id": 1, "method": "initialize",
            "params": {"protocolVersion": "2025-06-18", "capabilities": {},
                       "clientInfo": {"name": "test", "version": "0"}}
        });
        let started = router
            .clone()
            .oneshot(request(Some(token), None, &initialize))
            .await
            .ok()?;
        if started.status() != StatusCode::OK {
            return None;
        }
        let session = started
            .headers()
            .get("mcp-session-id")
            .and_then(|v| v.to_str().ok())
            .map(str::to_owned);
        let initialized =
            serde_json::json!({"jsonrpc": "2.0", "method": "notifications/initialized"});
        router
            .clone()
            .oneshot(request(Some(token), session.as_deref(), &initialized))
            .await
            .ok()?;
        Some(Self { router, session })
    }

    /// `tools/call`, answered as the whole JSON-RPC reply.
    async fn call(&self, name: &str, arguments: serde_json::Value) -> Option<serde_json::Value> {
        let body = serde_json::json!({"jsonrpc": "2.0", "id": 2, "method": "tools/call",
                                      "params": {"name": name, "arguments": arguments}});
        let response = self
            .router
            .clone()
            .oneshot(request(Some("secret"), self.session.as_deref(), &body))
            .await
            .ok()?;
        answer(response).await
    }

    /// `tools/call`, answered as the tool's own JSON.
    async fn tool(&self, name: &str, arguments: serde_json::Value) -> Option<serde_json::Value> {
        tool_json(&self.call(name, arguments).await?)
    }
}

/// A scripted character, logged in and `Ready`, whose `look` reports a stun.
async fn stunned_on_look() -> (cena_session::SessionHandle, cena_session::SessionObserver) {
    let prompt = b"<prompt time=\"1\">&gt;</prompt>\n";
    let (source, transcript) = AnsweringSource::logged_in(prompt);
    transcript.answer(
        "look",
        b"<indicator id=\"IconSTUNNED\" visible=\"y\"/>\n<prompt time=\"2\">&gt;</prompt>\n",
    );
    let session = Session::new(source);
    let (handle, observer) = (session.handle(), session.observer());
    tokio::spawn(session.into_actor().run());
    for _ in 0..200 {
        if observer
            .subscribe()
            .await
            .is_ok_and(|(s, _)| s.lifecycle == State::Ready)
        {
            break;
        }
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
    (handle, observer)
}

#[tokio::test]
async fn an_mcp_client_reads_a_character_behind_the_token() {
    let (handle, observer) = stunned_on_look().await;
    let characters = Characters::default();
    let door = handle.agent_door();
    characters.seat(SessionId(1), "Nisugi", observer, door, None, None);
    handle.set_agent_level(Level::Observe);
    let stop = CancellationToken::new();
    let router = cena_agent::router(characters, "secret".to_owned(), &stop);

    let hello = serde_json::json!({"jsonrpc": "2.0", "id": 1, "method": "ping"});
    for token in [None, Some("guess")] {
        let refused = router
            .clone()
            .oneshot(request(token, None, &hello))
            .await
            .unwrap();
        assert_eq!(refused.status(), StatusCode::UNAUTHORIZED, "{token:?}");
    }
    let client = Client::connect(router, "secret").await.unwrap();

    let before = client
        .tool("state", serde_json::json!({"character": "nisugi"}))
        .await
        .unwrap();
    assert_eq!(before["character"], "Nisugi", "found ignoring case");
    assert_eq!(before["lifecycle"], "ready");
    assert!(
        before["statuses"].get("stunned").is_none(),
        "not yet reported"
    );
    let since = before["cursor"].as_u64().unwrap();

    handle
        .send_manual_at(handle.generation(), "look", Duration::from_secs(5))
        .await;
    let waited = client
        .tool(
            "wait",
            serde_json::json!({"character": "Nisugi", "since": since, "kinds": ["sent"], "timeout_ms": 5000}),
        )
        .await
        .unwrap();
    assert_eq!(waited["happenings"][0]["kind"], "sent", "{waited}");
    assert_eq!(waited["happenings"][0]["line"], "look");

    let now = client
        .tool("state", serde_json::json!({"character": "Nisugi"}))
        .await
        .unwrap();
    assert_eq!(now["statuses"]["stunned"], true, "{now}");

    let unknown = client
        .call("state", serde_json::json!({"character": "Kiyna"}))
        .await
        .unwrap();
    assert!(
        unknown.to_string().contains("no character"),
        "an unknown name is said: {unknown}"
    );
    stop.cancel();
}

/// Issue #19, point 1: a list the game has not stated is `null`, never
/// empty, so an agent cannot read "nobody said" as "nobody is here".
#[test]
fn an_unstated_list_is_null_and_a_stated_empty_one_is_empty() {
    let mut state = GameState::default();
    let unstated = project("Nisugi", &snapshot(state.clone(), 1));
    assert!(unstated.room.players.is_none(), "never said who is here");
    assert!(unstated.room.creatures.is_none() && unstated.room.objects.is_none());
    for id in ["room players", "room objs"] {
        state.apply(&cena_session::Frame::Component {
            id: id.into(),
            body: cena_session::Runs { runs: Vec::new() },
        });
    }
    let stated = project("Nisugi", &snapshot(state, 2));
    assert_eq!(stated.room.players, Some(Vec::new()), "said: nobody");
    assert_eq!(stated.room.creatures, Some(Vec::new()));
}

/// Issue #19, point 1: `changed` carries each field that changed with its
/// new value, and leaves out the ones that only count the clock.
#[test]
fn changed_carries_what_changed_and_not_the_clock() {
    let mut before = GameState::default();
    before.status.set("stunned", false);
    let mut after = before.clone();
    after.status.set("stunned", true);
    let (was, mut now) = (
        project("Nisugi", &snapshot(before, 1)),
        project("Nisugi", &snapshot(after, 2)),
    );
    now.roundtime.seconds_left = Some(3);
    now.captured_unix_ms += 1_000;
    let Some(Happening::Changed { fields }) = cena_agent::happenings::changed(&was, &now) else {
        panic!("a status changed");
    };
    assert_eq!(
        fields.keys().collect::<Vec<_>>(),
        ["statuses"],
        "{fields:?}"
    );
    assert_eq!(fields["statuses"]["stunned"], true);
    assert!(
        cena_agent::happenings::changed(&was, &was).is_none(),
        "nothing changed, nothing said"
    );
}

/// A seated character's client, the character at `off`.
async fn seated() -> Option<(
    cena_session::SessionHandle,
    cena_session::SessionObserver,
    Client,
    CancellationToken,
)> {
    let (handle, observer) = stunned_on_look().await;
    let characters = Characters::default();
    let door = handle.agent_door();
    characters.seat(SessionId(1), "Nisugi", observer.clone(), door, None, None);
    let stop = CancellationToken::new();
    let router = cena_agent::router(characters, "secret".to_owned(), &stop);
    let client = Client::connect(router, "secret").await?;
    Some((handle, observer, client, stop))
}

/// What the player was told, as text, from a subscription taken earlier.
fn told(events: &mut tokio::sync::broadcast::Receiver<cena_session::ObservedEvent>) -> Vec<String> {
    let mut said = Vec::new();
    while let Ok(observed) = events.try_recv() {
        if let Event::Notice(notice) = observed.event {
            said.push(notice.lines().join("\n"));
        }
    }
    said
}

/// Step 2, at `off`: the agent sees the name and the level and nothing else,
/// cannot even ask to act, and the player is told it tried -- once, however
/// often it tries.
#[tokio::test]
async fn at_off_an_agent_sees_a_name_and_is_refused_the_rest() {
    let (_handle, observer, client, stop) = seated().await.unwrap();
    let (_, mut events) = observer.subscribe().await.unwrap();
    let listed = client
        .tool("characters", serde_json::json!({}))
        .await
        .unwrap();
    assert_eq!(
        listed,
        serde_json::json!([{"character": "Nisugi", "level": "off"}])
    );
    let answer = client
        .call("state", serde_json::json!({"character": "Nisugi"}))
        .await
        .unwrap();
    assert_eq!(
        answer["result"]["isError"], true,
        "a refusal the model reads"
    );
    let refused = tool_json(&answer).unwrap();
    assert_eq!(refused["refused"], "level");
    assert_eq!(refused["needed"], "observe");
    assert_eq!(refused["level"], "off");
    assert!(refused["approval"].is_null(), "a read is never approved");
    let act = client
        .tool(
            "tell_player",
            serde_json::json!({"character": "Nisugi", "text": "hello", "because": "testing"}),
        )
        .await
        .unwrap();
    assert!(act["approval"].is_null(), "at off, not even asked: {act}");
    for tool in ["wait", "records"] {
        let again = client
            .tool(
                tool,
                serde_json::json!({"character": "Nisugi", "since": 0, "sql": "SELECT 1"}),
            )
            .await
            .unwrap();
        assert_eq!(again["needed"], "observe", "{tool}: {again}");
    }
    tokio::time::sleep(Duration::from_millis(50)).await;
    let said = told(&mut events);
    assert_eq!(said.len(), 1, "told once: {said:?}");
    assert!(said[0].contains("agent level observe"), "{said:?}");
    stop.cancel();
}

/// Step 2, above `off`: an act the level does not allow waits for the
/// player, whose answer the agent reads in `wait`; lowering the level and
/// raising it again makes an old cursor `lagged`; at `advise` the act is
/// done at once.
#[tokio::test]
async fn an_act_above_the_level_waits_for_the_player() {
    let (handle, _observer, client, stop) = seated().await.unwrap();
    handle.set_agent_level(Level::Observe);
    let state = client
        .tool("state", serde_json::json!({"character": "Nisugi"}))
        .await
        .unwrap();
    let since = state["cursor"].as_u64().unwrap();
    let tell = serde_json::json!({"character": "Nisugi", "text": "the hunt is over", "because": "you asked to know"});
    let asked = client.tool("tell_player", tell.clone()).await.unwrap();
    assert_eq!(asked["needed"], "advise", "{asked}");
    assert_eq!(asked["approval"]["asked"], true, "{asked}");
    let id = asked["approval"]["id"].as_u64().unwrap();

    handle.approve_agent(id).unwrap();
    assert!(handle.approve_agent(id).is_err(), "once");
    let waited = client
        .tool(
            "wait",
            serde_json::json!({"character": "Nisugi", "since": since, "kinds": ["approval"], "timeout_ms": 5000}),
        )
        .await
        .unwrap();
    assert_eq!(
        waited["happenings"][0],
        serde_json::json!({"cursor": waited["happenings"][0]["cursor"], "kind": "approval", "id": id, "approved": true})
    );
    let notices = client
        .tool(
            "wait",
            serde_json::json!({"character": "Nisugi", "since": since, "kinds": ["notice"], "timeout_ms": 0}),
        )
        .await
        .unwrap();
    let texts = notices["happenings"].to_string();
    assert!(
        texts.contains("An agent asks to tell you something"),
        "{texts}"
    );
    assert!(
        texts.contains("Agent: the hunt is over") && texts.contains("you asked to know"),
        "{texts}"
    );

    handle.set_agent_level(Level::Off);
    handle.set_agent_level(Level::Observe);
    let mut lagged = false;
    for _ in 0..200 {
        let waited = client
            .tool(
                "wait",
                serde_json::json!({"character": "Nisugi", "since": since, "timeout_ms": 0}),
            )
            .await
            .unwrap();
        if waited["lagged"] == true {
            lagged = true;
            break;
        }
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
    assert!(lagged, "what happened at off is not handed over");

    handle.set_agent_level(Level::Advise);
    let told = client.tool("tell_player", tell).await.unwrap();
    assert_eq!(told, serde_json::json!({"told": true}));
    let capabilities = client
        .tool("capabilities", serde_json::json!({"character": "Nisugi"}))
        .await
        .unwrap();
    assert_eq!(capabilities["character"]["level"], "advise");
    assert!(
        capabilities["tools"]
            .as_array()
            .is_some_and(|tools| tools.len() == 6 && tools.iter().all(|t| t["needs"].is_string())),
        "{capabilities}"
    );
    stop.cancel();
}
