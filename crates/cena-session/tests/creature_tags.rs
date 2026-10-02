//! `.targetid` in the session (`cena_model::targetid`): with it on, each
//! creature's tag follows its name in what viewers are shown, after the
//! triggers read the line; and `kill <tag>`, typed, reaches the game as the
//! game's own target for it. Off, neither.

use std::time::Duration;

use cena_model::targetid::{Style, assign, tag};
use cena_platform::AnsweringSource;
use cena_session::{Event, Generation, Outcome, Session};

const PROMPT: &[u8] = b"<prompt time=\"1\">&gt;</prompt>\n";
const DEADLINE: Duration = Duration::from_secs(5);
const KOBOLD: i64 = 123_456;

/// A room with a kobold in it, and a line that names it.
fn look() -> Vec<u8> {
    format!(
        "<component id='room objs'>You also see <pushBold/>a <a exist=\"{KOBOLD}\" noun=\"kobold\">kobold</a><popBold/>.</component>\n\
         <crtrStatus exist=\"{KOBOLD}\" hostile=\"1\"/>\n\
         <pushBold/>A <a exist=\"{KOBOLD}\" noun=\"kobold\">kobold</a><popBold/> growls at you.\n\
         <prompt time=\"1\">&gt;</prompt>\n"
    )
    .into_bytes()
}

/// The kobold's tag, the first creature in the room: its unique letter
/// `A`, its random ones from its id.
fn kobolds_tag() -> String {
    tag(assign(KOBOLD, []), Style::default(), None)
}

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

/// `on`: the lines shown after a look, and what the game got for `kill` of
/// the kobold's tag.
async fn with_tags(on: bool) -> (Vec<String>, Vec<String>) {
    let key = kobolds_tag().to_ascii_lowercase();
    let (source, transcript) = AnsweringSource::logged_in(PROMPT);
    transcript.answer("look", &look());
    // The game always answers a swing: a bare prompt answers nothing.
    let later: &[u8] = b"You swing at the kobold!\n<prompt time=\"2\">&gt;</prompt>\n";
    transcript.answer(&format!("kill #{KOBOLD}"), later);
    transcript.answer(&format!("kill {key}"), later);
    let session = Session::new(source);
    let handle = session.handle();
    handle.tag_creatures(on.then(Style::default));
    let (_, mut events) = session.subscribe();
    tokio::spawn(session.into_actor().run());
    let looked = handle
        .send_manual_at(Generation::FIRST, "look", DEADLINE)
        .await;
    assert!(matches!(looked, Outcome::Confirmed(_)), "{looked:?}");
    // As the player types it, at the GUI or the web page: written at once,
    // not queued (the crate review of 2026-10-01, SE-A-1: this path was the
    // one no test sent a tag through, and it did not resolve one).
    let killed = handle
        .send_typed_at(Generation::FIRST, &format!("kill {key}"), DEADLINE)
        .await;
    assert!(matches!(killed, Outcome::Sent), "{killed:?}");
    tokio::time::sleep(std::time::Duration::from_millis(100)).await;
    (shown(&mut events), transcript.lines())
}

#[tokio::test(flavor = "current_thread", start_paused = true)]
async fn a_creature_is_tagged_and_its_tag_names_it_back() {
    let tagged = format!("A kobold ({}) growls at you.", kobolds_tag());
    let (lines, sent) = with_tags(true).await;
    assert!(lines.contains(&tagged), "{lines:?}");
    assert_eq!(sent.last(), Some(&format!("kill #{KOBOLD}")), "{sent:?}");

    let (lines, sent) = with_tags(false).await;
    assert!(
        lines.contains(&"A kobold growls at you.".to_owned()),
        "{lines:?}"
    );
    assert_eq!(
        sent.last(),
        Some(&format!("kill {}", kobolds_tag().to_ascii_lowercase())),
        "off, the line as typed"
    );
}
