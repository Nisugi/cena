//! Hydra speaking to the player: `cena_session::notice`.

use cena_session::{Body, Event, GenerationCell, Notice, NoticeKind, SessionHandle};

fn handle() -> (
    SessionHandle,
    tokio::sync::broadcast::Receiver<Event>,
    tokio::sync::mpsc::Receiver<cena_session::command::Inbox>,
) {
    let (tx, rx) = tokio::sync::mpsc::channel(32);
    let (events, listening) = tokio::sync::broadcast::channel(8);
    let handle = SessionHandle::new(tx, GenerationCell::first(), events);
    (handle, listening, rx)
}

#[test]
fn what_is_said_is_an_event_beside_the_frames() {
    let (handle, mut listening, _inbox) = handle();
    let table = vec!["STEP  MOVE".to_owned(), "   1  north".to_owned()];
    handle.say(Notice::table(NoticeKind::Info, table.clone()));
    handle.say(Notice::line(NoticeKind::Error, "no way there"));

    let Ok(Event::Notice(first)) = listening.try_recv() else {
        panic!("the table was not published");
    };
    assert_eq!(first.kind, NoticeKind::Info);
    assert_eq!(first.body, Body::Mono(table), "a table stays a table");
    let Ok(Event::Notice(second)) = listening.try_recv() else {
        panic!("the line was not published");
    };
    assert_eq!(second.lines(), ["no way there"]);
}

/// The reason it does not go through the command inbox: a behavior saying why
/// it stopped must get through when the queue it was filling is full, and
/// must not take the slot a `release` is owed.
#[test]
fn a_full_command_queue_does_not_silence_it_and_it_takes_no_slot() {
    let (handle, mut listening, _inbox) = handle();
    while handle.try_send_traffic_for_test() {}
    let free = handle.capacity_for_debug();
    assert_eq!(free, 1, "the fixture: only the reserved slot is left");

    handle.say(Notice::line(NoticeKind::Warn, "the queue is full"));
    assert!(matches!(listening.try_recv(), Ok(Event::Notice(_))));
    assert_eq!(handle.capacity_for_debug(), free, "it spent a command slot");
}

/// With nobody listening there is nobody to tell, and that is not an error.
#[test]
fn saying_it_to_nobody_is_harmless() {
    let (handle, listening, _inbox) = handle();
    drop(listening);
    handle.say(Notice::line(NoticeKind::Info, "anyone?"));
}
