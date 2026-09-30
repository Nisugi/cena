//! Hydra's one question to the game (`plan/55` §4a): `health`, sent once
//! when the model cannot work out a nerve rank, its nerve lines left out of
//! what viewers are shown and the rest of it shown.

use std::time::Duration;

use cena_platform::AnsweringSource;
use cena_session::{Event, Generation, Outcome, Session};

const PROMPT: &[u8] = b"<prompt time=\"1\">&gt;</prompt>\n";
const DEADLINE: Duration = Duration::from_secs(5);

/// The injury window ranking the nerves, then a prompt.
fn nerves(rank: u8) -> Vec<u8> {
    format!(
        "<dialogData id='injuries'><image id='nsys' name='Nsys{rank}'/></dialogData>\n\
         <prompt time=\"1\">&gt;</prompt>\n"
    )
    .into_bytes()
}

/// `health`'s report, with a nerve scar of 2.
const REPORT: &[u8] = b"<output class=\"mono\"/>\nYou have constant muscle spasms.\n\n     \
    Maximum Health Points:   158       193\n<output class=\"\"/>\n<prompt time=\"1\">&gt;</prompt>\n";

/// The text of every line viewers were shown.
fn shown(events: &mut tokio::sync::broadcast::Receiver<Event>) -> Vec<String> {
    let mut lines = Vec::new();
    while let Ok(event) = events.try_recv() {
        if let Event::Line(line) = event {
            lines.push(line.runs.plain());
        }
    }
    lines
}

#[tokio::test(flavor = "current_thread", start_paused = true)]
async fn a_confused_nerve_rank_sends_health_once_and_hides_its_nerve_line() {
    let (source, transcript) = AnsweringSource::logged_in(PROMPT);
    transcript.answer("look", &nerves(2));
    transcript.answer("health", REPORT);
    let session = Session::new(source);
    let (handle, observer) = (session.handle(), session.observer());
    let (_, mut events) = session.subscribe();
    tokio::spawn(session.into_actor().run());

    let looked = handle
        .send_manual_at(Generation::FIRST, "look", DEADLINE)
        .await;
    assert!(matches!(looked, Outcome::Confirmed(_)), "{looked:?}");
    // The session's own `health` goes out after, and is answered.
    tokio::time::sleep(Duration::from_millis(50)).await;
    assert_eq!(transcript.lines(), ["look", "health"]);

    let lines = shown(&mut events);
    assert!(
        lines.iter().any(|l| l.contains("Maximum Health Points")),
        "the rest of the report shows: {lines:?}"
    );
    assert!(
        !lines.iter().any(|l| l.contains("muscle spasms")),
        "its nerve line does not: {lines:?}"
    );
    let (snapshot, _) = observer.subscribe().await.unwrap();
    let nsys = snapshot.state.character.injuries.get("nsys").copied();
    assert_eq!(nsys.map(|i| (i.wound, i.scar)), Some((0, 2)), "settled");

    // The same rank again asks nothing.
    transcript.answer("look", &nerves(2));
    let _ = handle
        .send_manual_at(Generation::FIRST, "look", DEADLINE)
        .await;
    tokio::time::sleep(Duration::from_millis(50)).await;
    assert_eq!(transcript.lines(), ["look", "health", "look"]);
}

/// The player's own `health` is the player's to read, all of it.
#[tokio::test(flavor = "current_thread", start_paused = true)]
async fn the_players_own_health_shows_its_nerve_line() {
    let (source, transcript) = AnsweringSource::logged_in(PROMPT);
    transcript.answer("health", REPORT);
    let session = Session::new(source);
    let handle = session.handle();
    let (_, mut events) = session.subscribe();
    tokio::spawn(session.into_actor().run());

    let _ = handle
        .send_manual_at(Generation::FIRST, "health", DEADLINE)
        .await;
    let lines = shown(&mut events);
    assert!(
        lines.iter().any(|l| l.contains("muscle spasms")),
        "{lines:?}"
    );
    assert_eq!(transcript.lines(), ["health"]);
}
