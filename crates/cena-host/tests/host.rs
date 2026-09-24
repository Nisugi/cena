//! `plan/29` step 3: the session table, over scripted game connections.

use std::collections::VecDeque;
use std::time::Duration;

use cena_host::{AddError, Host, Who, stop_all};
use cena_platform::{AnsweringSource, TranscriptHandle};
use cena_session::{
    ConnectError, Connector, Event, Generation, Outcome, SessionId, SessionObserver, State,
    StoppedBecause,
};

const REPLY: &[u8] = b"You see.\n<prompt time='1'>&gt;</prompt>\n";
const DEADLINE: Duration = Duration::from_secs(5);

struct Connections(VecDeque<AnsweringSource>);

impl Connector for Connections {
    type Source = AnsweringSource;

    async fn connect(&mut self, _: Generation) -> Result<AnsweringSource, ConnectError> {
        self.0
            .pop_front()
            .ok_or_else(|| ConnectError::fatal("fixture", "exhausted"))
    }
}

fn who(account: &str, character: &str) -> Who {
    Who {
        account: account.to_owned(),
        character: character.to_owned(),
    }
}

/// A logged-in scripted connection, and its transcript.
fn game() -> (Connections, TranscriptHandle) {
    let (source, transcript) = AnsweringSource::logged_in(REPLY);
    (Connections(vec![source].into()), transcript)
}

/// Wait for `Ready`. A `Result`: not a `#[test]` function.
async fn ready(observer: &SessionObserver) -> Result<(), String> {
    let (snapshot, mut events) = observer.subscribe().await.map_err(|e| format!("{e:?}"))?;
    if snapshot.lifecycle == State::Ready {
        return Ok(());
    }
    loop {
        if events.recv().await.map_err(|e| e.to_string())?.event
            == Event::StateChanged(State::Ready)
        {
            return Ok(());
        }
    }
}

async fn answers(host: &Host, id: SessionId) -> bool {
    let Some(hosted) = host.get(id) else {
        return false;
    };
    matches!(
        hosted
            .handle
            .send_manual_at(Generation::FIRST, "look", DEADLINE)
            .await,
        Outcome::Confirmed(_)
    )
}

#[tokio::test(start_paused = true)]
async fn sessions_are_added_numbered_and_run_side_by_side() {
    let mut host = Host::new();
    let (a_game, _) = game();
    let (b_game, _) = game();
    let a = host
        .add(who("ACCT1", "Nisugi"), a_game, |s| s)
        .expect("added");
    let b = host
        .add(who("ACCT2", "Nerten"), b_game, |s| s)
        .expect("added");
    assert_eq!((a, b), (SessionId(0), SessionId(1)));

    for id in [a, b] {
        let hosted = host.get(id).expect("in the table");
        ready(&hosted.observer).await.expect("logs in");
        let (snapshot, _) = hosted.observer.subscribe().await.expect("answers");
        assert_eq!(snapshot.session, id, "each session names its own id");
        assert!(answers(&host, id).await);
    }
    let _ = stop_all(host.take_all()).await;
}

#[tokio::test(start_paused = true)]
async fn a_second_session_on_one_account_is_refused_until_the_first_is_removed() {
    let mut host = Host::new();
    let (first, _) = game();
    let (second, _) = game();
    let (third, _) = game();
    let nisugi = host
        .add(who("ACCT1", "Nisugi"), first, |s| s)
        .expect("added");
    ready(&host.get(nisugi).expect("in").observer)
        .await
        .expect("logs in");

    // Accounts compare without case, as the game's do.
    assert_eq!(
        host.add(who("acct1", "Sugiin"), second, |s| s),
        Err(AddError::AccountInUse {
            account: "acct1".to_owned(),
            by: "Nisugi".to_owned(),
        })
    );

    // Removed, the account is free at once -- before the stop finishes.
    let removed = host.take(nisugi).expect("in the table");
    let sugiin = host
        .add(who("ACCT1", "Sugiin"), third, |s| s)
        .expect("the account is free");
    assert_ne!(sugiin, nisugi, "an id is never reused");
    let _ = removed.stop().await;
    let _ = stop_all(host.take_all()).await;
}

#[tokio::test(start_paused = true)]
async fn removing_one_sends_its_quit_and_leaves_the_others_running() {
    let mut host = Host::new();
    let (a_game, a_transcript) = game();
    let (b_game, b_transcript) = game();
    let a = host
        .add(who("ACCT1", "Nisugi"), a_game, |s| s)
        .expect("added");
    let b = host
        .add(who("ACCT2", "Nerten"), b_game, |s| s)
        .expect("added");
    for id in [a, b] {
        ready(&host.get(id).expect("in").observer)
            .await
            .expect("logs in");
    }

    let end = host.take(a).expect("in the table").stop().await;
    assert!(
        a_transcript.lines().iter().any(|line| line == "quit"),
        "a removed session says goodbye to the game: {:?}",
        a_transcript.lines()
    );
    assert!(
        matches!(
            end.map(|e| e.stopped_because),
            Some(StoppedBecause::Cancelled)
        ),
        "a removal is a deliberate stop, never a reconnect"
    );
    assert!(host.get(a).is_none());
    assert!(answers(&host, b).await, "the other session is untouched");
    assert!(!b_transcript.lines().iter().any(|line| line == "quit"));
    let _ = stop_all(host.take_all()).await;
}

#[tokio::test(start_paused = true)]
async fn stopping_all_quits_every_session() {
    let mut host = Host::new();
    let mut transcripts = Vec::new();
    for (account, character) in [("A1", "One"), ("A2", "Two"), ("A3", "Three")] {
        let (connections, transcript) = game();
        let id = host
            .add(who(account, character), connections, |s| s)
            .expect("added");
        ready(&host.get(id).expect("in").observer)
            .await
            .expect("logs in");
        transcripts.push(transcript);
    }

    let ended = stop_all(host.take_all()).await;
    assert_eq!(ended.len(), 3);
    assert!(
        ended.iter().all(Option::is_some),
        "every session ended in order"
    );
    for transcript in transcripts {
        assert!(transcript.lines().iter().any(|line| line == "quit"));
    }
    assert_eq!(host.sessions().count(), 0);
}

#[tokio::test(start_paused = true)]
async fn a_session_that_stopped_on_its_own_does_not_hold_its_account() {
    // A connector with nothing to hand out: the login is refused, fatally,
    // and the session stops by itself -- as a mistyped password does.
    let mut host = Host::new();
    let refused = host
        .add(who("ACCT1", "Nisugi"), Connections(VecDeque::new()), |s| s)
        .expect("added");
    let end = tokio::time::timeout(DEADLINE, async {
        while host.get(refused).is_some_and(cena_host::Hosted::is_running) {
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await;
    assert!(end.is_ok(), "the refused session stopped by itself");
    assert!(
        host.get(refused).is_some(),
        "it stays in the table, so a frontend can say why"
    );

    let (retry, _) = game();
    assert!(
        host.add(who("ACCT1", "Nisugi"), retry, |s| s).is_ok(),
        "a stopped session does not hold its account"
    );
    let _ = stop_all(host.take_all()).await;
}
