//! Criterion 7: the whole session is recorded and **replays deterministically**
//! in a test, with no network.
//!
//! `plan/12:465`. "With no network" is literal here: [`ReplaySource`] has no
//! socket, no DNS and no TLS -- it *cannot* reach a network, rather than being
//! asked not to.
//!
//! # What is recorded, and why it is bytes
//!
//! The recorder holds the **byte stream**, not the frames. Recording frames
//! would make this test tautological: it would feed the parser's own output
//! back through the parser, so a parser regression would move the recording
//! and the replay in lockstep and nothing would go red.
//!
//! # Every source of nondeterminism, named
//!
//! | Source | Handling |
//! |---|---|
//! | wall-clock time | `start_paused = true`; no `Instant::now()` in session code |
//! | task scheduling | `flavor = "current_thread"`; a multi-thread runtime interleaves two ready tasks arbitrarily |
//! | `select!` arm choice | `biased;` in the actor loop -- without it tokio picks a ready arm pseudo-randomly |
//! | `HashMap` iteration | `BTreeMap` in `GameState::vitals`, asserted below |
//! | channel ordering | mpsc and oneshot are FIFO; ordering *across* channels is resolved by `biased` |
//! | read chunk boundaries | replayed from the recording, not regenerated |
//! | `Generation` | a counter seeded at 0, never a clock or a random |
//! | address / DNS / TLS | absent: `ReplaySource` touches none |

mod support;

use cena_platform::{RecordedEvent, ReplaySource};
use cena_session::{GameState, Session};

/// Run one session over these chunks and return what it recorded and knew.
async fn once(chunks: Vec<Vec<u8>>) -> (Vec<RecordedEvent>, GameState) {
    let end = Session::new(ReplaySource::new(chunks))
        .into_actor()
        .run()
        .await;
    (end.recorder.events().to_vec(), end.state)
}

#[tokio::test(flavor = "current_thread", start_paused = true)]
async fn a_recorded_session_replays_into_the_same_recording() {
    let live = vec![support::room_fixture().expect("room fixture")];

    // First run: record.
    let (recorded, first_state) = once(live.clone()).await;
    assert!(
        !recorded.is_empty(),
        "the recorder must have captured something, or every assertion below \
         passes vacuously over two empty transcripts"
    );

    // Replay THE RECORDING, not the original input. This is the loop
    // criterion 7 is about: a recording must be able to stand in for the
    // session it came from.
    let replay_chunks: Vec<Vec<u8>> = recorded
        .iter()
        .filter_map(|e| match e {
            RecordedEvent::Inbound { bytes, .. } => Some(bytes.clone()),
            RecordedEvent::Outbound { .. } => None,
        })
        .collect();
    let (rerecorded, second_state) = once(replay_chunks).await;

    assert_eq!(
        recorded, rerecorded,
        "replaying a recording must produce the same recording. If these \
         differ, the recorded stream is not sufficient to reproduce the \
         session, and criterion 7's replay is testing something other than \
         what ran."
    );
    assert_eq!(
        first_state, second_state,
        "the same bytes must produce the same state"
    );
}

#[tokio::test(flavor = "current_thread", start_paused = true)]
async fn ten_runs_of_the_same_recording_are_identical() {
    // One run proves nothing about determinism: a run is trivially equal to
    // itself. Ten runs is what makes an order-dependent container or an
    // unbiased `select!` show up as a difference rather than as a coin flip
    // that happened to land the same way.
    let chunks = vec![support::room_fixture().expect("room fixture")];
    let (baseline_events, baseline_state) = once(chunks.clone()).await;

    for run in 1..10 {
        let (events, state) = once(chunks.clone()).await;
        assert_eq!(
            events, baseline_events,
            "run {run} recorded a different transcript than run 0"
        );
        assert_eq!(state, baseline_state, "run {run} reached a different state");
    }
}

#[tokio::test(flavor = "current_thread", start_paused = true)]
async fn vitals_iterate_in_a_stable_order() {
    // The specific nondeterminism a replay assertion would hit first. A
    // `HashMap<String, u32>` iterates in an order that varies per process
    // (Rust's default hasher is randomly seeded), so a test that compared
    // two runs' vitals *within one process* would not catch it -- but a
    // golden written on one machine and read on another would flake.
    //
    // Asserting the ORDER rather than the contents is what makes this test
    // about the container choice.
    let wire = concat!(
        "<dialogData id='minivitals'>",
        "<progressBar id='stamina' value='90' text='stamina 90/100'/>",
        "<progressBar id='health' value='97' text='health 97/100'/>",
        "<progressBar id='mana' value='50' text='mana 50/100'/>",
        "</dialogData>\n",
        "<prompt time=\"1\">&gt;</prompt>\n"
    );
    let (_, state) = once(vec![wire.as_bytes().to_vec()]).await;

    let order: Vec<&str> = state.vitals.keys().map(String::as_str).collect();
    assert_eq!(
        order,
        vec!["health", "mana", "stamina"],
        "vitals must iterate in a stable, sorted order. They arrived in the \
         order stamina, health, mana; a BTreeMap yields them sorted, a \
         HashMap yields them in a per-process random order and a golden \
         written against one run would flake against the next."
    );
}

#[tokio::test(flavor = "current_thread", start_paused = true)]
async fn a_tag_split_across_two_reads_is_rejoined() {
    // The correctness question a byte-yielding transport raises, answered by
    // `Parser::push_bytes` (crates/cena-protocol/src/parser/read.rs:16-22)
    // rather than by anything in this crate. Asserted here because the
    // SESSION is what feeds it chunks, and a session that called `parse_line`
    // per read instead would desync on exactly this input.
    let whole = b"<nav rm='7503251'/><prompt time=\"1\">&gt;</prompt>\n";

    // Split INSIDE the `<nav` tag -- the boundary that breaks a naive reader.
    let split_at = 5;
    let chunks = vec![whole[..split_at].to_vec(), whole[split_at..].to_vec()];
    let (_, split_state) = once(chunks).await;
    let (_, whole_state) = once(vec![whole.to_vec()]).await;

    assert_eq!(
        split_state.room.id.as_deref(),
        Some("7503251"),
        "a tag split across two reads must be rejoined before parsing. Split \
         at byte {split_at}, mid-tag."
    );
    assert_eq!(
        split_state, whole_state,
        "the chunk boundary must not change the result. A session that fed \
         each read to parse_line instead of push_bytes would produce a \
         MalformedTag here and the two would differ."
    );
}
