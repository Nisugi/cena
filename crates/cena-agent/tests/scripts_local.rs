//! A runner's local copy (`plan/46` §4.2, §11 step 2), over MCP against a
//! scripted game: the first `state` carries every field; a chunk's changes
//! reach the runner **before** the lines of that chunk, as Lich updates its
//! state before a script sees the line; lines with no prompt after them
//! still arrive; and, with a map, the character's room is named and `room`
//! answers it.

use std::sync::Arc;

use axum::body::Body;
use axum::http::Request;
use cena_agent::scripts::local::Atlas;
use cena_agent::scripts::{Runners, router};
use cena_map::{Map, Origin, RoomId, Uid};
use cena_platform::AnsweringSource;
use cena_session::{GameState, Session};
use http_body_util::BodyExt;
use tokio_util::sync::CancellationToken;
use tower::ServiceExt;

/// A tool's JSON answer.
async fn call(
    app: &axum::Router,
    token: &str,
    name: &str,
    arguments: serde_json::Value,
) -> Option<serde_json::Value> {
    let body = serde_json::json!({"jsonrpc": "2.0", "id": 1, "method": "tools/call",
                                  "params": {"name": name, "arguments": arguments}});
    let request = Request::post("/mcp")
        .header("host", "127.0.0.1")
        .header("content-type", "application/json")
        .header("accept", "application/json, text/event-stream")
        .header("mcp-protocol-version", "2025-06-18")
        .header("authorization", format!("Bearer {token}"))
        .body(Body::from(body.to_string()))
        .ok()?;
    let response = app.clone().oneshot(request).await.ok()?;
    let bytes = response.into_body().collect().await.ok()?.to_bytes();
    let reply: serde_json::Value = serde_json::from_slice(&bytes).ok()?;
    serde_json::from_str(reply.pointer("/result/content/0/text")?.as_str()?).ok()
}

/// Every event `listen` gives until `done` holds of them.
async fn heard_until(
    app: &axum::Router,
    token: &str,
    done: impl Fn(&[serde_json::Value]) -> bool,
) -> Option<Vec<serde_json::Value>> {
    let (mut since, mut events) = (0, Vec::new());
    for _ in 0..20 {
        let heard = call(
            app,
            token,
            "listen",
            serde_json::json!({"since": since, "timeout_ms": 2000}),
        )
        .await?;
        events.extend(heard["events"].as_array().cloned().unwrap_or_default());
        since = heard["next"].as_u64()?;
        if done(&events) {
            break;
        }
    }
    Some(events)
}

/// The position of the first event `which` holds of.
fn at(events: &[serde_json::Value], which: impl Fn(&serde_json::Value) -> bool) -> Option<u64> {
    events
        .iter()
        .find(|e| which(e))
        .and_then(|e| e["at"].as_u64())
}

/// A running character; `answers` are the scripted game's `(line, reply)`.
fn character(
    answers: &[(&str, &[u8])],
) -> (cena_session::SessionHandle, cena_session::SessionObserver) {
    let (source, transcript) = AnsweringSource::logged_in(b"<prompt time=\"1\">&gt;</prompt>\n");
    for (line, reply) in answers {
        transcript.answer(line, reply);
    }
    let session = Session::new(source);
    let (handle, observer) = (session.handle(), session.observer());
    tokio::spawn(session.into_actor().run());
    (handle, observer)
}

#[tokio::test(flavor = "current_thread")]
async fn the_copy_changes_before_the_lines_that_changed_it() {
    let (handle, observer) = character(&[(
        "look",
        b"<indicator id=\"IconSTUNNED\" visible=\"y\"/>You are stunned!\n<prompt time=\"5\">&gt;</prompt>\n",
    )]);
    let runners = Runners::default();
    let token = runners
        .admit("Nisugi", handle.script_door(), &observer)
        .await
        .unwrap();
    let app = router(runners.clone(), &CancellationToken::new());

    let first = heard_until(&app, &token, |events| !events.is_empty())
        .await
        .unwrap();
    let state = &first[0];
    assert_eq!(state["kind"], "state", "the copy comes first: {first:?}");
    for field in [
        "room",
        "hands",
        "vitals",
        "statuses",
        "room_count",
        "map_room",
    ] {
        assert!(
            state["fields"].get(field).is_some(),
            "{field} missing from the first copy"
        );
    }
    assert_eq!(
        state["fields"]["map_room"],
        serde_json::Value::Null,
        "no map"
    );

    call(&app, &token, "send", serde_json::json!({"line": "look"}))
        .await
        .unwrap();
    let events = heard_until(&app, &token, |events| {
        events
            .iter()
            .any(|e| e["kind"] == "prompt" && e["time"] == 5)
    })
    .await
    .unwrap();
    let stunned = at(&events, |e| {
        e["kind"] == "state" && e["fields"]["statuses"]["stunned"] == true
    })
    .expect("the copy said stunned");
    let line = at(&events, |e| e["text"] == "You are stunned!").expect("the line came");
    let prompt = at(&events, |e| e["kind"] == "prompt" && e["time"] == 5).unwrap();
    assert!(stunned < line, "state {stunned} before the line {line}");
    assert!(line < prompt);

    // A line the game never answers has no prompt after it; its `sent`
    // still arrives, once the hold lets it go.
    call(&app, &token, "send", serde_json::json!({"line": "ponder"}))
        .await
        .unwrap();
    let events = heard_until(&app, &token, |events| {
        events
            .iter()
            .any(|e| e["kind"] == "sent" && e["line"] == "ponder")
    })
    .await
    .unwrap();
    assert!(events.iter().any(|e| e["line"] == "ponder"));
}

/// Travel's way of naming a room, cut down for a test: by the game's number.
fn by_number(map: &Map, state: &GameState, _: Origin) -> Option<RoomId> {
    let uid: i64 = state.room.id.as_deref()?.parse().ok()?;
    match map.ids_for_uid(Uid(uid)) {
        [room] => Some(*room),
        _ => None,
    }
}

#[tokio::test(flavor = "current_thread")]
async fn a_mapped_room_is_named_and_answered() {
    let rooms = serde_json::from_str(
        r#"[
        {"id":228,"uid":[7000],"title":["[Town Square]"],"exits":[{"to":229,"kind":"cardinal","cmd":"north","cost":0.2}]},
        {"id":229,"uid":[7001],"title":["[North Street]"]}
    ]"#,
    )
    .unwrap();
    let map = Arc::new(Map::from_rooms(rooms).unwrap());
    let (handle, observer) = character(&[(
        "look",
        b"<nav rm='7000'/>\n<prompt time=\"5\">&gt;</prompt>\n",
    )]);
    let runners = Runners::with_atlas(Atlas {
        map,
        locate: by_number,
    });
    let token = runners
        .admit("Nisugi", handle.script_door(), &observer)
        .await
        .unwrap();
    let app = router(runners.clone(), &CancellationToken::new());
    call(&app, &token, "send", serde_json::json!({"line": "look"}))
        .await
        .unwrap();
    let events = heard_until(&app, &token, |events| {
        events.iter().any(|e| e["fields"]["map_room"] == 228)
    })
    .await
    .unwrap();
    assert!(
        events.iter().any(|e| e["fields"]["map_room"] == 228),
        "{events:?}"
    );

    let room = call(&app, &token, "room", serde_json::json!({"id": 228}))
        .await
        .unwrap();
    assert_eq!(room["map"], true);
    assert_eq!(room["room"]["title"][0], "[Town Square]");
    assert_eq!(room["room"]["exits"][0]["to"], 229);
    let none = call(&app, &token, "room", serde_json::json!({"id": 9}))
        .await
        .unwrap();
    assert_eq!(none["room"], serde_json::Value::Null);

    let unmapped = Runners::default();
    let token = unmapped
        .admit("Nisugi", handle.script_door(), &observer)
        .await
        .unwrap();
    let app = router(unmapped, &CancellationToken::new());
    let room = call(&app, &token, "room", serde_json::json!({"id": 228}))
        .await
        .unwrap();
    assert_eq!(room["map"], false);
}

/// What the scripted game says to `info`, `skills` and `society`, in the
/// game's words (`crates/cena-model/tests/character_blocks.rs`,
/// `character_skills.rs`), and an experience window.
const SHEET: &[(&str, &[u8])] = &[
    (
        "info",
        b"Name: Nisugi Race: Half-Elf  Profession: Ranger\n\
Gender: Male    Age: 36    Expr: 43904921    Level: 100\n\
    Strength (STR):   110 (30)    ...  115 (32)    ...  120 (35)\n\
<prompt time=\"2\">&gt;</prompt>\n",
    ),
    (
        "skills",
        b" Nisugi (at level 100), your current skill bonuses and ranks (including all modifiers) are:\n\
  Skill Name                         | Current Current\n\
                                     |   Bonus   Ranks\n\
  Two Weapon Combat..................|     312     212\n\
  Elemental Lore - Air...............|     150      50\n\
\n\
Spell Lists\n\
  Minor Elemental....................|              75\n\
\n\
Training Points: 3673 Phy 0 Mnt\n\
<prompt time=\"3\">&gt;</prompt>\n",
    ),
    (
        "society",
        b"   You are a member in the Order of Voln at step 12.\n<prompt time=\"4\">&gt;</prompt>\n",
    ),
    (
        "exp",
        b"<dialogData id='expr'><label id='yourLvl' value='Level 100' top='0' left='0'/>\
<progressBar id='mindState' value='25' text='fresh and clear' top='45' left='3' field_exp='270' max_field_exp='1403' ascension_exp='24899176' lumnis='4' exp='43904921' until_next='79'/>\
</dialogData>\n<prompt time=\"5\">&gt;</prompt>\n",
    ),
];

#[tokio::test(flavor = "current_thread")]
async fn the_copy_carries_the_characters_sheet() {
    let (handle, observer) = character(SHEET);
    let runners = Runners::default();
    let token = runners
        .admit("Nisugi", handle.script_door(), &observer)
        .await
        .unwrap();
    let app = router(runners.clone(), &CancellationToken::new());

    let first = heard_until(&app, &token, |events| !events.is_empty())
        .await
        .unwrap();
    let fields = &first[0]["fields"];
    assert_eq!(
        fields["skills"],
        serde_json::json!({}),
        "never read: {fields}"
    );
    assert_eq!(fields["psms"], serde_json::json!({}));
    assert_eq!(fields["society"], serde_json::Value::Null);
    assert_eq!(
        fields["experience"]["field_experience"],
        serde_json::Value::Null
    );

    for (line, _) in SHEET {
        call(&app, &token, "send", serde_json::json!({ "line": line }))
            .await
            .unwrap();
    }
    let events = heard_until(&app, &token, |events| {
        events
            .iter()
            .any(|e| e["kind"] == "prompt" && e["time"] == 5)
    })
    .await
    .unwrap();
    let mut copy = serde_json::Map::new();
    for event in events.iter().filter(|e| e["kind"] == "state") {
        copy.extend(event["fields"].as_object().cloned().unwrap_or_default());
    }

    assert_eq!(copy["identity"]["race"], "Half-Elf");
    assert_eq!(copy["identity"]["profession"], "Ranger");
    assert_eq!(copy["identity"]["age"], 36);
    let strength = &copy["stats"]["strength"];
    assert_eq!(
        strength["base"],
        serde_json::json!({"value": 110, "bonus": 30})
    );
    assert_eq!(
        strength["ascended"],
        serde_json::json!({"value": 115, "bonus": 32})
    );
    assert_eq!(
        strength["enhanced"],
        serde_json::json!({"value": 120, "bonus": 35})
    );
    assert_eq!(
        copy["skills"]["two_weapon_combat"],
        serde_json::json!({"ranks": 212, "bonus": 312})
    );
    assert_eq!(copy["skills"]["elemental_lore_air"]["ranks"], 50);
    assert_eq!(copy["circles"]["Minor Elemental"], 75);
    assert_eq!(
        copy["society"],
        serde_json::json!({"name": "Order of Voln", "rank": 12})
    );
    let experience = &copy["experience"];
    assert_eq!(experience["level"], 100);
    assert_eq!(experience["field_experience"], 270);
    assert_eq!(experience["field_experience_max"], 1403);
    assert_eq!(experience["until_next"], 79);
    assert_eq!(experience["lumnis"], 4);
}
