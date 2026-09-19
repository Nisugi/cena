//! A session with a sink writes what crossed the wire; a session without one
//! behaves exactly as before.
//!
//! # Why this test exists
//!
//! `SessionSink` was written, unit-tested and committed **before anything
//! constructed one**. It compiled, its redaction was covered by seven tests,
//! and it logged nothing at all, because the session never called it. A green
//! suite said nothing about that.
//!
//! So these drive a real session actor and then read the files back off disk.
//! The assertion is not "the code compiles" but "the bytes are in the file".
//!
//! # These touch the filesystem, deliberately
//!
//! Every other test in this workspace avoids I/O. These cannot: the thing
//! under test *is* the write. They use a temp directory keyed to the test
//! name, and clean up after themselves.

use cena_platform::{AnsweringSource, Redactions, SessionSink};
use cena_session::{CommandId, Origin, Session};
use std::path::PathBuf;
use std::time::Duration;

/// The terminator that closes a round-trip window (`plan/12` §4.4).
const PROMPT: &[u8] = b"You see nothing unusual.\n<prompt time=\"1\">&gt;</prompt>\n";

/// A scratch directory for one test, removed on the way in so a previous
/// failure cannot make the next run pass.
fn scratch(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join("cena-session-logging").join(name);
    let _ = std::fs::remove_dir_all(&dir);
    dir
}

#[tokio::test(flavor = "current_thread")]
async fn a_session_with_a_sink_writes_both_directions_to_disk() {
    let dir = scratch("both_directions");
    let sink = SessionSink::create(&dir, "Tester", "stamp", Redactions::new())
        .expect("the sink must open");
    let bytes_path = sink.bytes_path().to_path_buf();
    let events_path = sink.events_path().to_path_buf();

    let (source, _transcript) = AnsweringSource::new(PROMPT);
    let session = Session::new(source).with_sink(sink);
    let handle = session.handle();
    let cancel = session.cancel_token();
    let actor = tokio::spawn(session.into_actor().run());

    let outcome = handle
        .send_and_await(
            CommandId(1),
            "look",
            Origin::Manual,
            Duration::from_secs(5),
            cena_session::queue::any_frame,
        )
        .await;
    assert!(
        matches!(outcome, cena_session::Outcome::Confirmed(_)),
        "the command must complete, or there is nothing to have logged: {outcome:?}"
    );

    cancel.cancel();
    actor.await.expect("the actor must not panic");

    let wire = std::fs::read_to_string(&bytes_path).expect("the bytes file must exist");
    let events = std::fs::read_to_string(&events_path).expect("the events file must exist");

    // OUTBOUND: wrapped in logxml.lic's markers, which is what makes a Cena
    // capture readable by the tools that read the corpus.
    assert!(
        wire.contains("<!-- CLIENT -->look<!-- ENDCLIENT -->"),
        "the command must be in the bytes file, wrapped as logxml.lic wraps \
         client input. Got:\n{wire}"
    );
    // INBOUND: verbatim, no wrapper.
    assert!(
        wire.contains("You see nothing unusual."),
        "the game's reply must be in the bytes file verbatim. Got:\n{wire}"
    );
    // The lifecycle reached the events file, which is what makes the log
    // useful for "when did it stop" rather than only "what crossed the wire".
    assert!(
        events.contains("lifecycle Ready"),
        "lifecycle transitions must reach the events file. Got:\n{events}"
    );
    assert!(
        events.contains("lifecycle Closed"),
        "the FINAL transition must survive the flush -- a buffered writer that \
         is not flushed after the last write loses exactly the lines that say \
         why a session ended. Got:\n{events}"
    );

    let _ = std::fs::remove_dir_all(&dir);
}

/// A credential registered with the sink must not reach the file, even though
/// the session writes the bytes through unchanged.
#[tokio::test(flavor = "current_thread")]
async fn a_registered_secret_does_not_reach_the_session_log() {
    let dir = scratch("redaction");
    let mut redactions = Redactions::new();
    redactions.key("supersecretkey123");
    let sink =
        SessionSink::create(&dir, "Tester", "stamp", redactions).expect("the sink must open");
    let bytes_path = sink.bytes_path().to_path_buf();

    let (source, _transcript) = AnsweringSource::new(PROMPT);
    let session = Session::new(source).with_sink(sink);
    let handle = session.handle();
    let cancel = session.cancel_token();
    let actor = tokio::spawn(session.into_actor().run());

    // A command carrying the secret, as a `send key supersecretkey123` would.
    let _ = handle
        .send_and_await(
            CommandId(1),
            "auth supersecretkey123",
            Origin::Manual,
            Duration::from_secs(5),
            cena_session::queue::any_frame,
        )
        .await;

    cancel.cancel();
    actor.await.expect("the actor must not panic");

    let wire = std::fs::read_to_string(&bytes_path).expect("the bytes file must exist");
    assert!(
        !wire.contains("supersecretkey123"),
        "a registered secret must not reach disk through the session path. \
         Got:\n{wire}"
    );
    assert!(
        wire.contains("<KEY>"),
        "the redaction must leave its marker, so a reader knows something was \
         removed rather than never sent. Got:\n{wire}"
    );

    let _ = std::fs::remove_dir_all(&dir);
}

/// A session with **no** sink runs exactly as it did before logging existed.
///
/// This is the one that protects criterion 7: `replay_determinism.rs` and every
/// other session test constructs a session without a sink, and none of them
/// should have to know this feature exists.
#[tokio::test(flavor = "current_thread")]
async fn a_session_without_a_sink_still_runs_and_writes_nothing() {
    let (source, transcript) = AnsweringSource::new(PROMPT);
    let session = Session::new(source);
    let handle = session.handle();
    let cancel = session.cancel_token();
    let actor = tokio::spawn(session.into_actor().run());

    let outcome = handle
        .send_and_await(
            CommandId(1),
            "look",
            Origin::Manual,
            Duration::from_secs(5),
            cena_session::queue::any_frame,
        )
        .await;
    assert!(
        matches!(outcome, cena_session::Outcome::Confirmed(_)),
        "a session with no sink must behave identically: {outcome:?}"
    );
    assert!(
        transcript.lines().contains(&"look".to_owned()),
        "the command still reaches the wire"
    );

    cancel.cancel();
    let end = actor.await.expect("the actor must not panic");
    // The recorder still records: the sink is an ADDITION to it, not a
    // replacement, which is what keeps the replay path unchanged.
    assert!(
        !end.recorder.is_empty(),
        "the in-memory recorder is unaffected by whether a sink exists"
    );
}
