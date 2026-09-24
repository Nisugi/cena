//! `plan/29` step 5: one web listener serving several characters -- each on
//! its own page -- and the hub page that lists them, adds and removes them.

mod web_support;

use cena_platform::AnsweringSource;
use cena_session::{Generation, Outcome, Session, SessionId};
use cena_ui::{ClientMessage, LifecycleView, ReceiptStatus, ServerMessage, WIRE_VERSION};
use cena_web::{HubRequest, WebServer};
use tokio_util::sync::CancellationToken;
use web_support::*;

/// One listener serves every character, each on its own page. A page names
/// its session; one that names none, with several running, is the hub --
/// which follows its characters' cards and takes no commands.
#[tokio::test]
async fn one_listener_serves_each_session_on_its_own_page() {
    let (a_source, a_transcript) = AnsweringSource::logged_in(ROOM);
    let (b_source, b_transcript) = AnsweringSource::logged_in(ROOM);
    let a = Session::numbered(SessionId(0), a_source);
    let b = Session::numbered(SessionId(1), b_source);
    let (a_handle, a_observer, a_stop) = (a.handle(), a.observer(), a.cancel_token());
    let (b_handle, b_observer, b_stop) = (b.handle(), b.observer(), b.cancel_token());
    let a_actor = tokio::spawn(a.into_actor().run());
    let b_actor = tokio::spawn(b.into_actor().run());
    await_ready(&a_observer, Generation::FIRST).await.unwrap();
    await_ready(&b_observer, Generation::FIRST).await.unwrap();

    let server = WebServer::open().await.unwrap();
    let sessions = server.sessions();
    sessions.attach("Nisugi", a_observer, a_handle);
    let b_handle_for_hurt = b_handle.clone();
    sessions.attach("Nerten", b_observer, b_handle);
    let pairing = server.pairing_url();
    let stop_web = CancellationToken::new();
    let web = tokio::spawn(server.run(stop_web.clone().cancelled_owned()));

    // Naming none, with two running, is the hub: every character's card.
    let mut hub = browser(&pairing).await.unwrap();
    let ServerMessage::Sessions {
        sessions: cards, ..
    } = receive(&mut hub).await.unwrap()
    else {
        panic!("two sessions and none named opens the hub");
    };
    let named: Vec<(&str, &str)> = cards
        .iter()
        .map(|card| (card.session.as_str(), card.name.as_str()))
        .collect();
    assert_eq!(named, [("0", "Nisugi"), ("1", "Nerten")]);
    assert!(
        cards
            .iter()
            .all(|card| card.lifecycle == LifecycleView::Ready),
        "each card reads its session's own view: {cards:?}"
    );
    // A card follows its character: a change in Nerten's health reaches the
    // hub without the hub being asked.
    b_transcript.answer(
        "hurt",
        b"You wince.\n<progressBar id='health' value='42'/><prompt time='1'>&gt;</prompt>\n",
    );
    assert!(matches!(
        b_handle_for_hurt
            .send_manual_at(Generation::FIRST, "hurt", DEADLINE)
            .await,
        Outcome::Confirmed(_)
    ));
    let updated = tokio::time::timeout(DEADLINE, async {
        loop {
            if let Ok(ServerMessage::Sessions {
                sessions: cards, ..
            }) = receive(&mut hub).await
                && let Some(health) = cards[1].vitals.health.as_ref()
                && health.percent == 42
            {
                return true;
            }
        }
    })
    .await;
    assert!(updated.is_ok(), "the hub's card for Nerten never showed 42");

    // The hub takes no commands: a command belongs to one character's page.
    send(&mut hub, &command("0", "0", "from-hub", "look"))
        .await
        .unwrap();
    assert!(
        closes(&mut hub).await,
        "a command sent to the hub is refused"
    );
    assert!(a_transcript.lines().is_empty());
    assert_eq!(b_transcript.lines(), ["hurt"], "only Nerten's own command");

    let mut page = browser_for(&pairing, Some("1")).await.unwrap();
    let ServerMessage::Snapshot {
        session,
        generation,
        ..
    } = receive(&mut page).await.unwrap()
    else {
        panic!("a named session's page opens on its snapshot");
    };
    assert_eq!(session, "1");
    send(&mut page, &command(&session, &generation, "look-1", "look"))
        .await
        .unwrap();
    assert_eq!(
        receipt(&mut page, "look-1", &mut Vec::new()).await.unwrap(),
        ReceiptStatus::Sent
    );
    assert_eq!(
        b_transcript.lines(),
        ["hurt", "look"],
        "the page's own session"
    );
    assert!(a_transcript.lines().is_empty(), "and no other");

    // Detached, its page is closed; the session itself runs on.
    sessions.detach(SessionId(1));
    assert!(closes(&mut page).await, "a detached session's page closes");
    assert!(!b_actor.is_finished());

    stop_web.cancel();
    web.await.unwrap().unwrap();
    a_stop.cancel();
    b_stop.cancel();
    let _ = (a_actor.await, b_actor.await);
}

/// `plan/29` step 5c: the hub adds and removes characters through whoever
/// runs the sessions -- the binary, which alone knows the roster and the
/// keyring. The web side only carries the request and the answer.
#[tokio::test]
async fn the_hub_asks_its_control_to_add_and_remove_and_shows_the_answer() {
    let (a_source, _) = AnsweringSource::logged_in(ROOM);
    let (b_source, _) = AnsweringSource::logged_in(ROOM);
    let a = Session::numbered(SessionId(0), a_source);
    let b = Session::numbered(SessionId(1), b_source);
    let (a_stop, b_stop) = (a.cancel_token(), b.cancel_token());
    let server = WebServer::open().await.unwrap();
    let sessions = server.sessions();
    sessions.attach("Nisugi", a.observer(), a.handle());
    sessions.attach("Nerten", b.observer(), b.handle());
    let a_actor = tokio::spawn(a.into_actor().run());
    let b_actor = tokio::spawn(b.into_actor().run());
    // Offered before any control exists: the hub must not show it, since
    // nothing could act on a click.
    sessions.offer(vec!["Sugiin".into()]);
    let pairing = server.pairing_url();
    let stop_web = CancellationToken::new();
    let web = tokio::spawn(server.run(stop_web.clone().cancelled_owned()));

    let mut hub = browser(&pairing).await.unwrap();
    let ServerMessage::Sessions { available, .. } = receive(&mut hub).await.unwrap() else {
        panic!("the hub opens on its cards");
    };
    assert!(
        available.is_empty(),
        "no control, nothing offered: {available:?}"
    );
    send(&mut hub, &add("Sugiin")).await.unwrap();
    assert!(matches!(
        next_note(&mut hub).await.as_deref(),
        Some("Adding and removing characters is not offered here.")
    ));

    // With a control, the offer shows and requests reach it.
    let asked = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
    let seen = std::sync::Arc::clone(&asked);
    sessions.control(std::sync::Arc::new(move |request: HubRequest| {
        let said = format!("handled {request:?}");
        seen.lock().unwrap().push(request);
        Box::pin(async move { said })
    }));
    let offered = tokio::time::timeout(DEADLINE, async {
        loop {
            if let Ok(ServerMessage::Sessions { available, .. }) = receive(&mut hub).await
                && available == ["Sugiin"]
            {
                return;
            }
        }
    })
    .await;
    assert!(
        offered.is_ok(),
        "the offer reached the hub once a control existed"
    );
    send(&mut hub, &add("Sugiin")).await.unwrap();
    assert_eq!(
        next_note(&mut hub).await.as_deref(),
        Some(r#"handled Add("Sugiin")"#)
    );
    send(&mut hub, &remove("1")).await.unwrap();
    assert!(next_note(&mut hub).await.is_some());
    assert_eq!(
        *asked.lock().unwrap(),
        [
            HubRequest::Add("Sugiin".into()),
            HubRequest::Remove(SessionId(1))
        ]
    );

    // A malformed id is not a request: the socket is closed, and nothing
    // reaches the control.
    send(&mut hub, &remove("01")).await.unwrap();
    assert!(closes(&mut hub).await);
    assert_eq!(asked.lock().unwrap().len(), 2);

    stop_web.cancel();
    web.await.unwrap().unwrap();
    a_stop.cancel();
    b_stop.cancel();
    let _ = (a_actor.await, b_actor.await);
}

fn add(character: &str) -> ClientMessage {
    ClientMessage::AddCharacter {
        version: WIRE_VERSION,
        character: character.to_owned(),
    }
}

fn remove(session: &str) -> ClientMessage {
    ClientMessage::RemoveSession {
        version: WIRE_VERSION,
        session: session.to_owned(),
    }
}

/// The next hub note on `socket`, skipping card lists; `None` on timeout.
async fn next_note(socket: &mut Browser) -> Option<String> {
    tokio::time::timeout(DEADLINE, async {
        loop {
            if let Ok(ServerMessage::HubNote { detail, .. }) = receive(socket).await {
                return detail;
            }
        }
    })
    .await
    .ok()
}

/// `plan/29` step 5d: a thought both characters hear is one line on the hub,
/// tagged with both -- the author's rule, "the same line is a duplicate line".
#[tokio::test]
async fn a_thought_two_characters_hear_is_one_line_on_the_hub_tagged_with_both() {
    let (a_source, a_transcript) = AnsweringSource::logged_in(ROOM);
    let (b_source, b_transcript) = AnsweringSource::logged_in(ROOM);
    let a = Session::numbered(SessionId(0), a_source);
    let b = Session::numbered(SessionId(1), b_source);
    let (a_handle, b_handle) = (a.handle(), b.handle());
    let (a_observer, b_observer) = (a.observer(), b.observer());
    let (a_stop, b_stop) = (a.cancel_token(), b.cancel_token());
    let a_actor = tokio::spawn(a.into_actor().run());
    let b_actor = tokio::spawn(b.into_actor().run());
    await_ready(&a_observer, Generation::FIRST).await.unwrap();
    await_ready(&b_observer, Generation::FIRST).await.unwrap();

    let server = WebServer::open().await.unwrap();
    let sessions = server.sessions();
    sessions.attach("Nisugi", a_observer, a_handle.clone());
    sessions.attach("Nerten", b_observer, b_handle.clone());
    let pairing = server.pairing_url();
    let stop_web = CancellationToken::new();
    let web = tokio::spawn(server.run(stop_web.clone().cancelled_owned()));
    let mut hub = browser(&pairing).await.unwrap();

    let thought: &[u8] =
        b"<pushStream id='thoughts'/>[General] Someone: hello there\n<popStream/><prompt time='1'>&gt;</prompt>\n";
    a_transcript.answer("listen", thought);
    b_transcript.answer("listen", thought);
    for handle in [&a_handle, &b_handle] {
        let _ = handle
            .send_manual_at(Generation::FIRST, "listen", DEADLINE)
            .await;
    }

    let merged = tokio::time::timeout(DEADLINE, async {
        loop {
            if let Ok(ServerMessage::Merged { lines, .. }) = receive(&mut hub).await
                && let Some(line) = lines.iter().find(|line| line.from.len() == 2)
            {
                return line.clone();
            }
        }
    })
    .await
    .expect("the hub never showed the thought tagged with both characters");
    assert_eq!(merged.stream, "thoughts");
    assert_eq!(merged.from, ["Nisugi", "Nerten"]);
    let text: String = merged.runs.iter().map(|run| run.text.as_str()).collect();
    assert!(text.contains("hello there"), "{text:?}");

    stop_web.cancel();
    web.await.unwrap().unwrap();
    a_stop.cancel();
    b_stop.cancel();
    let _ = (a_actor.await, b_actor.await);
}
