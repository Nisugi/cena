//! Operations (`plan/35` §8, step 3): what an agent performs is admitted
//! once per request id (issue #19, point 4), watched to its end as a ticket,
//! and steered only while it runs; below `behaviors` the player is asked, and
//! the yes starts it.
//!
//! The performer here is a stand-in that counts its starts, so "admitted
//! once" is a count, not a claim.

use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration;

use cena_platform::AnsweringSource;
use cena_session::Event;
use cena_session::agent::Change;
use cena_session::agent::{Admitted, Approval, Call, Denied, Door, Level};
use cena_session::operation::{
    Allows, Control, Ended, Lifecycle, Performer, Progress, Released, Report, Reporter, Start,
    Started, Steer, Work,
};
use cena_session::{Generation, Session, SessionHandle};
use tokio_util::sync::CancellationToken;

/// A performer that allows `walk <place>`: each run ends when stopped, or
/// arrives when `arrive` is cancelled. `starts` counts the runs begun.
fn performer(starts: &Arc<AtomicUsize>, arrive: &CancellationToken) -> Performer {
    let allows: Allows = Arc::new(|line: &str| {
        let words: Vec<&str> = line.split_whitespace().collect();
        if words.first() == Some(&"walk") && words.len() > 1 {
            Ok(words.join(" "))
        } else {
            Err("only walk <place>".to_owned())
        }
    });
    let (starts, arrive) = (Arc::clone(starts), arrive.clone());
    let start: Start = Arc::new(move |_line: &str, _reporter| {
        starts.fetch_add(1, Ordering::SeqCst);
        let stop = CancellationToken::new();
        let steer: Steer = {
            let stop = stop.clone();
            Arc::new(move |control: Control| {
                // The stand-in takes every control; only a stop ends it.
                if control == Control::Stop {
                    stop.cancel();
                }
                Ok(())
            })
        };
        let arrive = arrive.clone();
        Started {
            ended: Box::pin(async move {
                tokio::select! {
                    () = stop.cancelled() => Ended::plainly(Work::Interrupted, "stopped"),
                    () = arrive.cancelled() => Ended::plainly(Work::Completed, "arrived"),
                }
            }),
            steer,
            token: None,
        }
    });
    Performer {
        allowed: "walk <place>".to_owned(),
        allows,
        start,
    }
}

/// A session at `level` with the stand-in performer.
fn seated(level: Level) -> (SessionHandle, Arc<AtomicUsize>, CancellationToken) {
    let (source, _) = AnsweringSource::new(b"<prompt time=\"1\">&gt;</prompt>\n");
    let handle = Session::new(source).handle();
    let (starts, arrive) = (Arc::new(AtomicUsize::new(0)), CancellationToken::new());
    assert!(handle.set_performer(performer(&starts, &arrive)));
    handle.set_agent_level(level);
    (handle, starts, arrive)
}

fn call<'a>(request: &'a str, handle: &SessionHandle) -> Call<'a> {
    Call {
        request,
        generation: Some(handle.generation()),
    }
}

/// An operation's report, once it has ended.
async fn ended(door: &Door, id: u64) -> Option<Report> {
    for _ in 0..100 {
        match door.operation(id) {
            Some(report) if report.lifecycle == Lifecycle::Ended => return Some(report),
            _ => tokio::time::sleep(Duration::from_millis(5)).await,
        }
    }
    None
}

fn started(done: Result<Admitted, Denied>) -> Option<Report> {
    match done {
        Ok(Admitted::Operation(report)) => Some(report),
        _ => None,
    }
}

#[tokio::test]
async fn a_request_is_admitted_once_and_read_to_its_end() {
    let (handle, starts, arrive) = seated(Level::Behaviors);
    let door = handle.agent_door();
    let first = started(door.perform("walk   bank", "to sell", call("r1", &handle))).unwrap();
    assert_eq!(first.line, "walk bank", "kept as the performer keeps it");
    assert_eq!(first.lifecycle, Lifecycle::Running);
    let again = started(door.perform("walk bank", "to sell", call("r1", &handle))).unwrap();
    assert_eq!(again.id, first.id, "the same ticket");
    assert_eq!(starts.load(Ordering::SeqCst), 1, "admitted once");

    let other = door.perform("walk gate", "elsewhere", call("r1", &handle));
    assert!(
        matches!(&other, Err(Denied::Invalid(why)) if why.contains("different act")),
        "{other:?}"
    );
    let stale = Call {
        request: "r2",
        generation: Some(Generation(99)),
    };
    assert!(
        matches!(door.perform("walk gate", "x", stale), Err(Denied::Invalid(why)) if why.contains("connection"))
    );
    assert!(
        matches!(door.perform("fly home", "x", call("r3", &handle)), Err(Denied::Invalid(why)) if why.contains("only walk"))
    );
    assert_eq!(
        starts.load(Ordering::SeqCst),
        1,
        "nothing refused was started"
    );

    arrive.cancel();
    let over = ended(&door, first.id).await.unwrap();
    let result = over.ended.unwrap();
    assert_eq!(
        (result.work, result.reason.as_str()),
        (Work::Completed, "arrived")
    );
    assert_eq!(over.authority, Some(Released::NotClaimed));
    let stop = door.control(first.id, Control::Stop, "late", call("c0", &handle));
    assert!(
        matches!(&stop, Err(Denied::Invalid(why)) if why.contains("already ended")),
        "an ended operation is not steered: {stop:?}"
    );
}

#[tokio::test]
async fn a_stop_is_admitted_and_then_applied() {
    let (handle, _starts, _arrive) = seated(Level::Behaviors);
    let door = handle.agent_door();
    let run = started(door.perform("walk bank", "to sell", call("r1", &handle))).unwrap();
    let stopping =
        started(door.control(run.id, Control::Stop, "enough", call("c1", &handle))).unwrap();
    assert_ne!(
        stopping.lifecycle,
        Lifecycle::Running,
        "admission is not application"
    );
    let again = started(door.control(run.id, Control::Stop, "enough", call("c1", &handle)));
    assert_eq!(
        again.map(|r| r.id),
        Some(run.id),
        "the same request, the same answer"
    );
    let over = ended(&door, run.id).await.unwrap();
    let result = over.ended.unwrap();
    assert_eq!(
        (result.work, result.reason.as_str()),
        (Work::Interrupted, "stopped")
    );
}

/// Below `behaviors` the player is asked; asking again under the same id
/// waits on the same question; the yes starts it once, and the id then names
/// the operation. A no is remembered as a no.
#[tokio::test]
async fn below_behaviors_the_yes_starts_it() {
    let (handle, starts, _arrive) = seated(Level::Advise);
    let door = handle.agent_door();
    let asked = |request| match door.perform("walk bank", "to sell", call(request, &handle)) {
        Err(Denied::Level(refused)) => match refused.approval {
            Approval::Asked { id, .. } => Some(id),
            _ => None,
        },
        _ => None,
    };
    let id = asked("r1").unwrap();
    assert_eq!(asked("r1"), Some(id), "the same question, not a second");
    assert_eq!(handle.agent_requests().len(), 1);
    assert_eq!(starts.load(Ordering::SeqCst), 0);

    handle.approve_agent(id).unwrap();
    assert_eq!(starts.load(Ordering::SeqCst), 1);
    let run = started(door.perform("walk bank", "to sell", call("r1", &handle))).unwrap();
    assert_eq!(run.approval, Some(id), "started on the player's yes");
    assert_eq!(starts.load(Ordering::SeqCst), 1, "and only once");

    let second = asked("r2").unwrap();
    handle.deny_agent(second).unwrap();
    let after = door.perform("walk bank", "to sell", call("r2", &handle));
    assert!(
        matches!(&after, Err(Denied::Invalid(why)) if why.contains("did not let")),
        "{after:?}"
    );
    assert_eq!(starts.load(Ordering::SeqCst), 1);
}

/// Hold, resume and retreat, as the operation reads once each is admitted:
/// held, running again, retreating; a stop after a retreat is still a stop.
#[tokio::test]
async fn the_controls_read_in_the_lifecycle() {
    let (handle, _starts, _arrive) = seated(Level::Behaviors);
    let door = handle.agent_door();
    let run = started(door.perform("walk bank", "to sell", call("r1", &handle))).unwrap();
    let mut read = Vec::new();
    for (request, control) in [
        ("c1", Control::Hold),
        ("c2", Control::Resume),
        ("c3", Control::Retreat),
        ("c4", Control::Stop),
    ] {
        let now = started(door.control(run.id, control, "steering", call(request, &handle)));
        read.push(now.map(|report| report.lifecycle));
    }
    assert_eq!(
        read,
        [
            Some(Lifecycle::Held),
            Some(Lifecycle::Running),
            Some(Lifecycle::Retreating),
            Some(Lifecycle::Stopping),
        ]
    );
}

/// Issue #19, point 6: progress is heard when what the operation is doing,
/// or its stall, changes; counts alone ride along unheard, and an unchanged
/// stall is not said again. Each change heard is numbered.
#[tokio::test]
async fn progress_is_heard_when_it_changes_and_not_otherwise() {
    let (source, _) = AnsweringSource::new(
        b"<prompt time=\"1\">&gt;</prompt>
",
    );
    let session = Session::new(source);
    let handle = session.handle();
    let (_, mut events) = session.subscribe();
    let slot: Arc<std::sync::Mutex<Option<Reporter>>> = Arc::default();
    let kept = Arc::clone(&slot);
    let allows: Allows = Arc::new(|line: &str| Ok(line.to_owned()));
    let start: Start = Arc::new(move |_line: &str, reporter: Reporter| {
        *kept.lock().unwrap() = Some(reporter);
        Started {
            ended: Box::pin(std::future::pending()),
            steer: Arc::new(|_| Ok(())),
            token: None,
        }
    });
    assert!(handle.set_performer(Performer {
        allowed: "anything".to_owned(),
        allows,
        start,
    }));
    handle.set_agent_level(Level::Behaviors);
    let door = handle.agent_door();
    let run = started(door.perform("hunt x", "x", call("r1", &handle))).unwrap();
    let reporter = slot.lock().unwrap().clone().unwrap();
    while events.try_recv().is_ok() {}
    let said = |engaged: u64, stalled: Option<&str>| Progress {
        doing: "hunting".to_owned(),
        counts: std::collections::BTreeMap::from([("engaged".to_owned(), engaged)]),
        stalled: stalled.map(str::to_owned),
    };
    reporter.progress(said(0, None));
    reporter.progress(said(1, None));
    reporter.progress(said(1, Some("nothing engaged for 5 minutes")));
    reporter.progress(said(2, Some("nothing engaged for 5 minutes")));
    let mut heard = Vec::new();
    while let Ok(event) = events.try_recv() {
        if let Event::Agent(Change::Operation(report)) = event {
            heard.push((report.revision, report.progress.and_then(|p| p.stalled)));
        }
    }
    assert_eq!(
        heard,
        [
            (1, None),
            (2, Some("nothing engaged for 5 minutes".to_owned()))
        ]
    );
    let now = door.operation(run.id).unwrap().progress.unwrap();
    assert_eq!(now.counts["engaged"], 2, "the counts ride along");
}
