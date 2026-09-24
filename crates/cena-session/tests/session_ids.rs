//! `plan/29` step 1: every session names itself, so N of them can be told
//! apart.
//!
//! Before this, every session was `SessionId::FIRST` whatever built it, and a
//! frontend serving two characters would have received two streams that
//! claimed to be the same session. These run two sessions in one process and
//! check that each one's snapshots, observed events and player-log lines
//! carry its own id and never the other's.

use std::collections::VecDeque;

use cena_platform::{AnsweringSource, ReplaySource};
use cena_session::player_log::Capture;
use cena_session::{
    ConnectError, Connector, Generation, PlayerLog, Session, SessionId, SupervisedSession,
};

struct Connections(VecDeque<AnsweringSource>);

impl Connector for Connections {
    type Source = AnsweringSource;

    async fn connect(&mut self, _: Generation) -> Result<Self::Source, ConnectError> {
        self.0
            .pop_front()
            .ok_or_else(|| ConnectError::fatal("fixture", "exhausted"))
    }
}

const REPLY: &[u8] = b"You see.\n<prompt time='1'>&gt;</prompt>\n";

#[tokio::test(start_paused = true)]
async fn two_supervised_sessions_name_themselves_in_snapshots_and_events() {
    let (first, _) = AnsweringSource::logged_in(REPLY);
    let (second, _) = AnsweringSource::logged_in(REPLY);
    let (a, a_handle) = SupervisedSession::numbered(SessionId(0), Connections(vec![first].into()));
    let (b, b_handle) = SupervisedSession::numbered(SessionId(7), Connections(vec![second].into()));
    let (a_observer, b_observer) = (a.observer(), b.observer());
    let (a_cancel, b_cancel) = (a.cancel_token(), b.cancel_token());
    let a_run = tokio::spawn(a.run());
    let b_run = tokio::spawn(b.run());

    let sessions = [
        (&a_observer, &a_handle, SessionId(0)),
        (&b_observer, &b_handle, SessionId(7)),
    ];
    for (observer, handle, id) in sessions {
        let (snapshot, mut events) = observer.subscribe().await.expect("running owner");
        assert_eq!(snapshot.session, id, "the snapshot names its own session");
        // A command, so there is an event to read: the scripted game says
        // nothing unprompted after its login burst.
        let _ = handle
            .send_manual_at(
                snapshot.generation,
                "look",
                std::time::Duration::from_secs(5),
            )
            .await;
        let event = events.recv().await.expect("the command is published");
        assert_eq!(event.session, id, "{event:?}");
    }

    a_cancel.cancel();
    b_cancel.cancel();
    let _ = (a_run.await, b_run.await);
}

#[tokio::test(flavor = "current_thread", start_paused = true)]
async fn a_plain_session_names_itself_in_the_player_log() {
    // `with_player_log` on a plain session used `SessionId::FIRST` whatever
    // the session was; the tap now reads the id the session was built with.
    let (log, mut sink) = PlayerLog::new();
    let session = Session::numbered(
        SessionId(3),
        ReplaySource::from_bytes(b"You see nothing unusual.\n"),
    )
    .with_player_log(log, Capture::default(), None);
    // The pre-run subscription: nothing answers an observer until it runs.
    let (snapshot, _) = session.subscribe();
    assert_eq!(snapshot.session, SessionId(3));
    let _ = session.into_actor().run().await;

    let line = sink.try_recv().expect("the line was logged");
    assert_eq!(line.session, SessionId(3));
}

#[test]
fn new_is_the_first_session() {
    // The single-session callers -- every other test, the binary today -- keep
    // the id they always had.
    let (snapshot, _) = Session::new(ReplaySource::from_bytes(b"")).subscribe();
    assert_eq!(snapshot.session, SessionId::FIRST);
}
