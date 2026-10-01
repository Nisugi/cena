//! Takeover (`plan/35` §4 and §8, step 5): an agent stops what runs and
//! holds the command authority; what it sends is the holder's; it gives the
//! character back, or the player takes it; and each ending is its own --
//! released, revoked, a lowered level, the owner gone quiet, a death, which
//! drops the level to Observe.

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use cena_platform::{AnsweringSource, TranscriptHandle};
use cena_session::agent::{Admitted, Call, Denied, Door, Level, OWNER_IDLE, TOKEN};
use cena_session::operation::{
    Allows, Control, Ended, Lifecycle, Performer, Report, Start, Started, Work,
};
use cena_session::{Event, Origin, Session, SessionHandle};
use tokio::sync::broadcast::Receiver;

const DEADLINE: Duration = Duration::from_secs(5);

/// A session running against a scripted game, at `level`, with a stand-in
/// performer whose halt is recorded; the legacy event stream beside it.
fn running(
    level: Level,
) -> Option<(
    SessionHandle,
    TranscriptHandle,
    Receiver<Event>,
    Arc<AtomicBool>,
)> {
    let (source, transcript) = AnsweringSource::logged_in(b"<prompt time=\"1\">&gt;</prompt>\n");
    transcript.answer(
        "glance",
        b"You glance about.\n<prompt time=\"2\">&gt;</prompt>\n",
    );
    transcript.answer(
        "die",
        b"<indicator id=\"IconDEAD\" visible=\"y\"/>\n<prompt time=\"3\">&gt;</prompt>\n",
    );
    let session = Session::new(source);
    let handle = session.handle();
    let (_, events) = session.subscribe();
    tokio::spawn(session.into_actor().run());
    let halted = Arc::new(AtomicBool::new(false));
    let kept = Arc::clone(&halted);
    let allows: Allows = Arc::new(|line: &str| Ok(line.to_owned()));
    let start: Start = Arc::new(|_line: &str, _reporter| Started {
        ended: Box::pin(std::future::ready(Ended::plainly(Work::Completed, "done"))),
        steer: Arc::new(|_| Ok(())),
        token: None,
    });
    let performer = Performer {
        allowed: "anything".to_owned(),
        allows,
        start,
        halt: Arc::new(move || kept.store(true, Ordering::SeqCst)),
    };
    handle.set_performer(performer).then_some(())?;
    handle.set_agent_level(level);
    Some((handle, transcript, events, halted))
}

fn call<'a>(request: &'a str, handle: &SessionHandle) -> Call<'a> {
    Call {
        request,
        generation: Some(handle.generation()),
    }
}

/// Take the character over, and let the takeover claim the authority.
async fn taken(door: &Door, handle: &SessionHandle, request: &str) -> Option<Report> {
    let Ok(Admitted::Operation(report)) = door.take_over("to steer", call(request, handle)) else {
        return None;
    };
    for _ in 0..100 {
        if handle.holder() == Some(TOKEN) {
            return Some(report);
        }
        tokio::time::sleep(Duration::from_millis(5)).await;
    }
    None
}

/// Operation `id` once it has ended.
async fn ended(door: &Door, id: u64) -> Option<Ended> {
    for _ in 0..200 {
        match door.operation(id) {
            Some(report) if report.lifecycle == Lifecycle::Ended => return report.ended,
            _ => tokio::time::sleep(Duration::from_millis(5)).await,
        }
    }
    None
}

#[tokio::test(flavor = "current_thread", start_paused = true)]
async fn a_takeover_holds_the_character_until_it_is_given_back() {
    let (handle, _transcript, mut events, halted) = running(Level::Takeover).unwrap();
    let door = handle.agent_door();
    let held = taken(&door, &handle, "t1").await.unwrap();
    assert!(halted.load(Ordering::SeqCst), "what ran was stopped first");
    assert_eq!(handle.agent_holds(), Some(held.id));

    let refused = door.perform("hunt x", "go on", call("p1", &handle));
    assert!(
        matches!(&refused, Err(Denied::Invalid(why)) if why.contains("holds this character")),
        "{refused:?}"
    );
    let again = door.take_over("again", call("t2", &handle));
    assert!(
        matches!(&again, Err(Denied::Invalid(why)) if why.contains("already holds")),
        "one at a time: {again:?}"
    );

    let Ok(Admitted::Operation(glance)) = door.command("glance", "to see", call("c1", &handle))
    else {
        panic!("the holder's line was not sent");
    };
    let answered = ended(&door, glance.id).await.unwrap();
    assert_eq!(answered.reason, "answered");
    let mut origins = Vec::new();
    while let Ok(event) = events.try_recv() {
        if let Event::Sent { line, origin, .. } = event {
            origins.push((line, origin));
        }
    }
    assert!(
        origins.contains(&("glance".to_owned(), Origin::Agent(Some(TOKEN)))),
        "sent as the holder's: {origins:?}"
    );

    door.control(held.id, Control::Stop, "done", call("s1", &handle))
        .unwrap();
    let over = ended(&door, held.id).await.unwrap();
    assert_eq!(
        (over.work, over.reason.as_str()),
        (Work::Completed, "released")
    );
    assert_eq!(handle.holder(), None, "given back, and nothing resumes");
    assert_eq!(handle.agent_holds(), None);
}

/// Two `take_over`s at the same moment, from two threads: one is admitted
/// and the other refused, never both, so the authority is never held with
/// nothing recording the takeover that holds it (the crate review of
/// 2026-10-01, SE-B-3). A race, so it is run many times.
#[test]
fn two_takeovers_at_once_admit_one() {
    // The racers are plain threads; the runtime only takes what they spawn.
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();
    for round in 0..300 {
        let _inside = runtime.enter();
        let (source, _transcript) =
            AnsweringSource::logged_in(b"<prompt time=\"1\">&gt;</prompt>\n");
        let session = Session::new(source);
        let handle = session.handle();
        handle.set_agent_level(Level::Takeover);
        let start = Arc::new(std::sync::Barrier::new(2));
        let racers: Vec<_> = ["a", "b"]
            .into_iter()
            .map(|request| {
                let (handle, start) = (handle.clone(), Arc::clone(&start));
                let runtime = runtime.handle().clone();
                std::thread::spawn(move || {
                    let _inside = runtime.enter();
                    let door = handle.agent_door();
                    let call = Call {
                        request,
                        generation: Some(handle.generation()),
                    };
                    start.wait();
                    matches!(door.take_over("to steer", call), Ok(Admitted::Operation(_)))
                })
            })
            .collect();
        let admitted = racers
            .into_iter()
            .map(|racer| racer.join().unwrap())
            .filter(|admitted| *admitted)
            .count();
        assert_eq!(admitted, 1, "round {round}");
        drop(session);
    }
}

/// The player's stop outranks the agent: the authority comes back at once,
/// before the takeover has even noticed, and the ending says revoked.
#[tokio::test(flavor = "current_thread", start_paused = true)]
async fn the_player_takes_the_character_back_at_once() {
    let (handle, ..) = running(Level::Takeover).unwrap();
    let door = handle.agent_door();
    let held = taken(&door, &handle, "t1").await.unwrap();
    assert_eq!(handle.stop_agent(), (0, true));
    assert_eq!(handle.holder(), None, "at once");
    let over = ended(&door, held.id).await.unwrap();
    assert_eq!(
        (over.work, over.reason.as_str()),
        (Work::Interrupted, "revoked")
    );
    assert_eq!(
        handle.agent_level(),
        Level::Takeover,
        "the level is the player's to lower"
    );
}

/// A lowered level, an agent gone quiet, and a death each end it, each said;
/// a death also drops the level to Observe.
#[tokio::test(flavor = "current_thread", start_paused = true)]
async fn a_takeover_ends_on_a_lowered_level_on_silence_and_on_death() {
    let (handle, ..) = running(Level::Takeover).unwrap();
    let door = handle.agent_door();
    let held = taken(&door, &handle, "t1").await.unwrap();
    handle.set_agent_level(Level::Commands);
    assert_eq!(handle.holder(), None);
    assert_eq!(ended(&door, held.id).await.unwrap().reason, "level_lowered");

    handle.set_agent_level(Level::Takeover);
    let held = taken(&door, &handle, "t2").await.unwrap();
    tokio::time::sleep(OWNER_IDLE + Duration::from_secs(10)).await;
    assert_eq!(ended(&door, held.id).await.unwrap().reason, "owner_idle");
    assert_eq!(handle.holder(), None);

    let held = taken(&door, &handle, "t3").await.unwrap();
    handle
        .send_manual_at(handle.generation(), "die", DEADLINE)
        .await;
    let over = ended(&door, held.id).await.unwrap();
    assert_eq!((over.work, over.reason.as_str()), (Work::Failed, "dead"));
    for _ in 0..100 {
        if handle.agent_level() == Level::Observe {
            break;
        }
        tokio::time::sleep(Duration::from_millis(5)).await;
    }
    assert_eq!(
        handle.agent_level(),
        Level::Observe,
        "a run that ended badly"
    );
}
