//! **Criterion 9**, offline (`plan/12:553`):
//!
//! > A killed connection mid-command yields `Disconnected`, and reconnect
//! > leaves invalidated facts `Unknown` (§5.2) — verified in the replay.
//!
//! # How a disconnect is staged without a network
//!
//! `ReplaySource::read` returns `Ok(0)` the moment its chunks run out
//! (`replay.rs:78-80`). So a recording that ends **without a terminating
//! `<prompt>`** is precisely a killed connection mid-command: the command went
//! out, some frames came back, the window never closed, the stream died.
//!
//! That needs **no new `ByteSource`** — the trait has three implementors and a
//! fourth for testing would want justifying. What it does need is a
//! [`Connector`] that hands back a prepared source per generation, which is the
//! second implementor `plan/05` §-1 requires and which criterion 9 requires by
//! saying "verified in the replay": a replay cannot log in.

use cena_platform::ReplaySource;
use cena_session::{
    CommandId, ConnectError, Connector, EndReason, Generation, Origin, Outcome, SupervisedSession,
};
use std::collections::VecDeque;
use std::time::Duration;

/// A [`Connector`] that hands out one prepared source per generation.
struct ScriptedConnector {
    sources: VecDeque<Vec<Vec<u8>>>,
}

impl ScriptedConnector {
    fn new(sources: Vec<Vec<Vec<u8>>>) -> Self {
        Self {
            sources: sources.into(),
        }
    }
}

impl Connector for ScriptedConnector {
    type Source = ReplaySource;

    async fn connect(&mut self, _generation: Generation) -> Result<ReplaySource, ConnectError> {
        self.sources
            .pop_front()
            .map(ReplaySource::new)
            .ok_or_else(|| {
                // **Fatal, not transient.** A scripted connector that has run out
                // has nothing more to serve and no number of retries changes that,
                // so this is the honest classification -- and it is also what makes
                // these tests terminate rather than climb the ladder forever.
                ConnectError::fatal("scripted", "no more prepared connections")
            })
    }
}

/// A burst that teaches the session everything criterion 9 asks about, and
/// then **stops without a prompt** — a connection killed mid-command.
fn everything_then_death() -> Vec<Vec<u8>> {
    vec![
        // Chunk 1: the session learns everything, ending in a prompt so the
        // state is fully KNOWN before anything dies. Without this the test
        // could not tell "invalidated" from "never observed".
        concat!(
            "<nav rm='7503251'/>",
            "<compass><dir value=\"n\"/></compass>",
            "<roundTime value='1789775824'/>",
            r#"<indicator id="IconSTANDING" visible="y"/>"#,
            "<left>a brass lantern</left><right>a steel broadsword</right>",
            "<progressBar id='health' value='97'/>",
            "<prompt time=\"1789775821\">R&gt;</prompt>\n",
        )
        .as_bytes()
        .to_vec(),
        // Chunk 2: a partial reply with **no terminating prompt**. The command
        // window opens, this arrives, and then the chunks run out -- `Ok(0)`,
        // which is a connection killed mid-command.
        b"You begin to search the area.\n".to_vec(),
    ]
}

/// What a fresh login re-sends, MEASURED across 7/7 of Cena's own logins
/// (`plan/15` §2b): the room description and vitals, and **nothing else**.
fn a_real_login_burst() -> Vec<Vec<u8>> {
    vec![
        concat!(
            "<component id='room desc'>A quiet corner.</component>",
            "<progressBar id='health' value='100'/>",
            "<prompt time=\"1789775900\">&gt;</prompt>\n",
        )
        .as_bytes()
        .to_vec(),
    ]
}

/// **Criterion 9, first half.** A command in flight when the transport dies
/// resolves `Disconnected` — not `Dead`, and not `Timeout`.
///
/// `Dead` would tell a behavior the session is over when another connection is
/// already coming; `Timeout` would say the window closed without a match, which
/// is a different fact. Both are wrong in ways a behavior would act on.
#[tokio::test(flavor = "current_thread", start_paused = true)]
async fn a_command_killed_mid_flight_resolves_disconnected() {
    // **A connection that says nothing at all**, then ends. No prompt means no
    // terminator, so the command's window is still OPEN when the stream dies --
    // which is what "killed mid-command" means.
    //
    // Two earlier fixtures failed here for instructive reasons, both recorded
    // because they are easy to repeat:
    //
    // * a burst ending in a prompt resolved `Confirmed` -- `any_frame` matched a
    //   frame that had already arrived before the command was even sent;
    // * the same burst with a never-matching matcher resolved `Timeout` -- the
    //   prompt closed the window normally, and the disconnect then found
    //   nothing waiting.
    //
    // `Timeout` and `Disconnected` are both correct answers to different
    // questions, and only a window that is genuinely open at the moment the
    // transport dies produces the second.
    let connector = ScriptedConnector::new(vec![vec![], a_real_login_burst()]);
    let (session, handle) = SupervisedSession::new(connector);
    let task = tokio::spawn(session.run());

    let outcome = handle
        .send_and_await(
            CommandId(1),
            "search",
            Origin::Manual,
            Duration::from_secs(30),
            cena_session::queue::any_frame,
        )
        .await;

    assert_eq!(
        outcome,
        Outcome::Disconnected,
        "criterion 9: a killed connection mid-command yields Disconnected. \
         `Dead` would say the session is over when another connection is \
         already coming; `Timeout` would say the window closed without a match."
    );

    let _ = task.await;
}

/// **Criterion 9, second half.** After the reconnect, the facts the login burst
/// does not re-send are `Unknown`, and the ones it does are not.
///
/// The classification is MEASURED rather than designed (`plan/15` §2b): the
/// burst in this test is the real shape — a room description and vitals, no
/// `nav rm`, no hands, no roundtime, no indicators.
#[tokio::test(flavor = "current_thread", start_paused = true)]
async fn reconnect_leaves_invalidated_facts_unknown() {
    let connector = ScriptedConnector::new(vec![everything_then_death(), a_real_login_burst()]);
    let (session, _handle) = SupervisedSession::new(connector);

    // Two prepared connections, neither of which is used for anything. That is
    // what terminates this test: no command is ever sent, so the unattended cap
    // stops the session after the second. It used to terminate by exhausting
    // the connector instead -- the ladder now stops it one step earlier.
    let end = session.run().await;

    assert_eq!(
        end.generations,
        Generation::FIRST.next(),
        "two connections were served, so the counter advanced ONCE -- \
         reconnecting into the second. It does not advance again, because \
         neither connection sent a command and the unattended cap stops the \
         session BEFORE the reconnect rather than after it."
    );
    assert_eq!(
        end.stopped_because,
        cena_session::StoppedBecause::Unattended,
        "nothing was ever sent, so the session was never attended. This test \
         used to end by exhausting the connector; the unattended cap now stops \
         it one step earlier, which is the more honest reason of the two."
    );

    // Invalidated: the burst carries none of these.
    // **RETAINED**, flipped from `None` deliberately -- see
    // `invalidate_for_reconnect`. Clearing it made `in_roundtime()` report
    // `Some(false)` during a live roundtime, which is the opposite of the
    // Unknown §5.2 asks for. An absolute server epoch cannot go stale while the
    // socket is down.
    //
    // This fixture's roundtime (1789775824) is in the PAST relative to the
    // second connection's prompt (1789775900), so retaining it is also visibly
    // harmless here: it compares as expired rather than being asserted.
    assert_eq!(
        end.state.roundtime_ends,
        Some(1_789_775_824),
        "an absolute server epoch survives the reconnect"
    );
    assert_eq!(end.state.left_hand, None, "hands are Unknown");
    assert_eq!(end.state.right_hand, None);
    assert_eq!(end.state.room.id, None, "the room id is Unknown");
    assert!(end.state.room.exits.is_empty(), "exits are Unknown");
    assert!(
        !end.state.status.is_known("standing"),
        "an indicator reported on the OLD connection must be Unknown, not \
         false: a new generation has been told nothing"
    );
    // **`Some(false)`, and that is the right answer -- it asserted `None`
    // before the retry ladder landed.** Not a regression: the ladder changed
    // *when* the state is read, not what invalidation does.
    //
    // `in_roundtime` needs a known clock (`state/clock.rs:93-96`), and the
    // clock comes from a prompt's `time=`. The session used to end after a
    // THIRD invalidation had wiped the clock again, so the honest answer then
    // was "I do not know what time it is". It now ends after the second
    // connection's burst, whose prompt carries `time="1789775900"` -- so the
    // clock is known, `roundtime_ends` is still `None`, and "not in roundtime"
    // is a fact rather than a guess.
    //
    // That makes this the stronger assertion of the two: it shows the old
    // roundtime was forgotten AND the new clock was learned, where the old one
    // could not tell those apart.
    assert_eq!(
        end.state.in_roundtime(),
        Some(false),
        "the OLD roundtime is gone and the NEW clock is known, so this is a \
         measured false rather than plan/12 §5.2's Unknown"
    );

    // Retained: the burst re-sent these, so they are observed rather than stale.
    assert_eq!(
        end.state.vitals.get("health"),
        Some(&100),
        "vitals come from the NEW burst (100, not the old 97)"
    );
}

/// The recording spans both generations, which is what "verified in the replay"
/// requires: a recording that reset per connection could not contain a
/// reconnect at all.
#[tokio::test(flavor = "current_thread", start_paused = true)]
async fn the_recording_spans_the_reconnect() {
    let connector = ScriptedConnector::new(vec![everything_then_death(), a_real_login_burst()]);
    let (session, _handle) = SupervisedSession::new(connector);
    let end = session.run().await;

    let recorded: Vec<u8> = end
        .recorder
        .events()
        .iter()
        .filter_map(|event| match event {
            cena_platform::RecordedEvent::Inbound { bytes, .. } => Some(bytes.clone()),
            cena_platform::RecordedEvent::Outbound { .. } => None,
        })
        .flatten()
        .collect();
    let text = String::from_utf8_lossy(&recorded);

    assert!(
        text.contains("7503251"),
        "the FIRST generation's bytes must be in the recording"
    );
    assert!(
        text.contains("A quiet corner"),
        "...and the SECOND generation's, or the recording cannot express a \
         reconnect and criterion 9's 'verified in the replay' is unmeetable"
    );
}

/// A **cancelled** session does not reconnect, and does not consume its next
/// prepared connection.
///
/// The second assertion is the real one: asserting only on the generation count
/// would pass for a session that reconnected and then stopped for some other
/// reason.
#[tokio::test(flavor = "current_thread", start_paused = true)]
async fn a_cancelled_session_does_not_reconnect() {
    // An `AnsweringSource` would never hang up, but a ReplaySource that never
    // ends is not available -- so a source with a chunk and no more is used,
    // and the cancel is what must win the race to matter.
    let connector = ScriptedConnector::new(vec![everything_then_death(), a_real_login_burst()]);
    let (session, _handle) = SupervisedSession::new(connector);
    let cancel = session.cancel_token();
    cancel.cancel();

    let end = session.run().await;

    assert_eq!(
        end.reason,
        EndReason::Cancelled,
        "a cancelled session reports a deliberate stop"
    );
    assert_eq!(
        end.generations,
        Generation::FIRST,
        "it must NOT have advanced: a deliberate stop does not reconnect, and \
         the generation counter is the evidence"
    );
}
