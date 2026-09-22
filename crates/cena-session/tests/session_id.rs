//! A session's identity, and how it differs from a connection's.
//!
//! `SessionId` was added in M4 (`plan/23` §D1a) on the argument `Generation`
//! already makes for itself in `lifecycle.rs:45-53`: *retrofitting an id onto
//! every event and snapshot after they exist is the expensive version of this
//! decision.*
//!
//! The pair is not redundant, and the test that matters is
//! [`the_id_survives_a_reconnect_where_the_generation_does_not`] — if it ever
//! fails, the two have collapsed into one concept and routing by session id
//! would send a reconnected character's output to the wrong place.

use cena_platform::ReplaySource;
use cena_session::{ConnectError, Connector, Generation, SessionId, SupervisedSession};
use std::collections::VecDeque;

/// A [`Connector`] that hands out one prepared source per generation.
///
/// The same shape as `reconnect.rs`'s, minus the call counter this file does
/// not need. Two connections' worth of bytes, so a reconnect really happens.
struct ScriptedConnector {
    sources: VecDeque<Vec<Vec<u8>>>,
}

impl Connector for ScriptedConnector {
    type Source = ReplaySource;

    async fn connect(&mut self, _generation: Generation) -> Result<ReplaySource, ConnectError> {
        self.sources
            .pop_front()
            .map(ReplaySource::new)
            // Fatal rather than transient, for `reconnect.rs`'s reason: a
            // scripted connector that has run out has nothing more to serve,
            // and this is what makes the test terminate rather than climb the
            // backoff ladder forever.
            .ok_or_else(|| ConnectError::fatal("scripted", "no more prepared connections"))
    }
}

#[test]
fn a_new_session_is_the_first_one() {
    // A single-session process uses FIRST and nothing else, which is why M4
    // can add the id before M5's session manager exists.
    let (session, _handle) = SupervisedSession::new(ScriptedConnector {
        sources: VecDeque::new(),
    });
    let (snapshot, _events) = session.subscribe();
    assert_eq!(snapshot.session, SessionId::FIRST);
}

#[test]
fn the_id_is_deterministic_rather_than_drawn_from_a_clock() {
    // Criterion 7's replay must produce the same ids on every run. Seeded at
    // 0 and counted up -- never a clock reading, never random.
    for _ in 0..3 {
        let (session, _handle) = SupervisedSession::new(ScriptedConnector {
            sources: VecDeque::new(),
        });
        assert_eq!(session.subscribe().0.session, SessionId(0));
    }
}

#[tokio::test]
async fn the_id_survives_a_reconnect_where_the_generation_does_not() {
    // **THE TEST THAT JUSTIFIES TWO TYPES.** A session keeps its id for its
    // whole life; its generation advances on every reconnect. Collapsing them
    // would mean a frontend routing on "session id" sent a reconnected
    // character's output somewhere else -- or, worse, to another character.
    //
    // Two scripted connections, the first ending with no terminating prompt,
    // which `reconnect.rs` establishes is exactly a killed connection.
    let (session, _handle) = SupervisedSession::new(ScriptedConnector {
        sources: VecDeque::from(vec![
            vec![b"<nav rm='7503251'/>\n".to_vec()],
            vec![b"<nav rm='7503252'/>\n<prompt time='1'>&gt;</prompt>\n".to_vec()],
        ]),
    });

    let (before, _events) = session.subscribe();
    assert_eq!(before.session, SessionId::FIRST, "guard: starts at FIRST");
    assert_eq!(
        before.generation,
        Generation::FIRST,
        "guard: starts at generation 0"
    );

    let end = Box::pin(session.run()).await;

    // The session reconnected, so the generation moved...
    assert!(
        end.generations > Generation::FIRST,
        "guard: this test is vacuous unless a reconnect happened, got {:?}",
        end.generations
    );

    // ...and the id did not.
    //
    // **`SupervisedEnd::session` was added so this line could exist.** A first
    // draft asserted only that the generation advanced, then re-asserted a
    // COPY of the id taken before the run -- which proves nothing, since a
    // captured value cannot change. `run` consumes the session, so without an
    // id on the ending there was no way to read the real one afterwards, and
    // the test's own name was a claim nothing checked.
    assert_eq!(
        end.session,
        SessionId::FIRST,
        "the id changed across {:?} connections",
        end.generations
    );
    assert_eq!(
        end.session, before.session,
        "and it is the one we started with"
    );
}

#[test]
fn the_next_id_is_a_counter_not_a_constant() {
    // Unused until M5 -- nothing allocates a second session yet -- but it is
    // the one line that makes this a counter. Without it the type would be a
    // constant wearing a newtype, which Rule -1 would reject.
    assert_eq!(SessionId::FIRST.next(), SessionId(1));
    assert_eq!(SessionId(7).next(), SessionId(8));
}

#[test]
fn ids_order_and_display_as_their_number() {
    // Ordering so a session manager can hold them in a BTreeMap, which
    // criterion 7 needs: iterating a HashMap would be non-deterministic.
    assert!(SessionId(0) < SessionId(1));
    assert_eq!(SessionId(3).to_string(), "3");
}
