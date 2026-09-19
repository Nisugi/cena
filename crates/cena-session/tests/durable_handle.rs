//! A handle must outlive a connection, and `Reconnecting` must gate automation.
//!
//! # The bug this file exists to prevent
//!
//! `SessionHandle` stamps its generation into every command, and the actor
//! discards anything from a prior one (`plan/12` §4.4). While nothing
//! reconnected, holding a **copy** of the generation was fine.
//!
//! It stops being fine the moment a second connection exists. A handle cloned
//! in generation 0 — by a frontend, or by a behavior — would keep stamping `0`
//! forever, and after one reconnect every command it sent would be answered
//! `Outcome::Interrupted` for the rest of the session. **The session would
//! reconnect successfully and then be deaf to every caller that existed before
//! the drop**, which is a silent, permanent wedge.
//!
//! So the handle reads a shared [`GenerationCell`] instead. These tests pin
//! both halves of why that is safe: the handle keeps working, *and* a command
//! stamped before the bump is still discarded.

use cena_session::{Generation, GenerationCell, State};

/// A cell starts at the first generation and advances by one.
#[test]
fn a_cell_counts_connections_from_the_first() {
    let cell = GenerationCell::first();
    assert_eq!(cell.get(), Generation::FIRST);
    assert_eq!(cell.advance(), Generation::FIRST.next());
    assert_eq!(cell.get(), Generation::FIRST.next());
}

/// **Every clone sees the change.** This is the whole point of the type.
///
/// A handle cloned into a behavior task in generation 0 must stamp generation 1
/// after a reconnect, without the behavior knowing a reconnect happened.
#[test]
fn every_clone_of_a_cell_sees_an_advance() {
    let held_since_generation_zero = GenerationCell::first();
    let supervisor = held_since_generation_zero.clone();

    assert_eq!(held_since_generation_zero.get(), Generation::FIRST);
    supervisor.advance();

    assert_eq!(
        held_since_generation_zero.get(),
        Generation::FIRST.next(),
        "a clone taken before the reconnect must read the NEW generation, or a \
         handle held across a reconnect is permanently wedged"
    );
}

/// **The discard rule survives**, and this is the subtlety that makes the
/// durable handle correct rather than a loophole.
///
/// `plan/12` §4.4: "anything from a prior generation is discarded." A durable
/// handle must not make that toothless. It does not, because the stamp happens
/// at **send time**: a command already in flight when the connection died was
/// stamped before the bump and still carries the old value.
#[test]
fn a_value_stamped_before_the_bump_is_still_stale() {
    let cell = GenerationCell::first();

    // What a command in flight carries: read at send time, before the drop.
    let stamped_in_flight = cell.get();

    cell.advance();

    assert_ne!(
        stamped_in_flight,
        cell.get(),
        "a command stamped before the reconnect must NOT match the new \
         generation -- otherwise it would be sent on the new connection as \
         though it were fresh, which is exactly what §4.4 forbids"
    );
    assert_eq!(
        cell.get(),
        Generation::FIRST.next(),
        "...while the handle's NEXT send reads the new generation"
    );
}

/// `Reconnecting` gates automation with no change to the gate.
///
/// `plan/12` §5.1: a session with no transport runs **no automation**. The
/// readiness gate is `matches!(self, Ready)`, so the new variant enforces that
/// rule structurally rather than by a check someone has to remember to write.
#[test]
fn no_automation_runs_while_reconnecting() {
    assert!(
        !State::Reconnecting.behaviors_may_run(),
        "plan/12 §5.1: 'No automation runs.' A behavior's command must be \
         refused by the gate that already existed, with no new branch."
    );
    assert!(
        State::Ready.behaviors_may_run(),
        "guard: Ready still allows it"
    );
}

/// Every non-`Ready` state refuses automation, so a new one cannot default open.
///
/// Enumerated rather than spot-checked: this is the assertion that goes red if
/// someone adds a state and the gate silently admits it.
#[test]
fn only_ready_admits_automation() {
    for state in [
        State::Connecting,
        State::Authenticating,
        State::Syncing,
        State::Reconnecting,
        State::Closed,
    ] {
        assert!(
            !state.behaviors_may_run(),
            "{state:?} must not admit automation -- only Ready does"
        );
    }
    assert!(State::Ready.behaviors_may_run());
}

/// **The handle itself reads the cell**, not a copy taken at construction.
///
/// The tests above exercise `GenerationCell` and would all pass against a
/// `SessionHandle` that ignored it entirely — VERIFIED by falsifying the stamp
/// to a constant, which left them green. This is the test that goes red.
///
/// It reaches through the real handle rather than the cell, which is the only
/// way to observe what a command would actually be stamped with.
#[tokio::test(flavor = "current_thread", start_paused = true)]
async fn a_handle_cloned_before_a_reconnect_reports_the_new_generation() {
    let session = cena_session::Session::new(cena_platform::ReplaySource::new(vec![]));
    // Cloned in generation 0, as a frontend or a behavior would hold it.
    let held_across_the_reconnect = session.handle();
    let cell = session.generation_cell();

    assert_eq!(held_across_the_reconnect.generation(), Generation::FIRST);

    // What a supervisor does between connections.
    cell.advance();

    assert_eq!(
        held_across_the_reconnect.generation(),
        Generation::FIRST.next(),
        "a handle cloned before the reconnect must stamp the NEW generation. \
         If it carries a copy instead, every command it sends after a \
         reconnect is answered Interrupted forever and the session is deaf to \
         every caller that existed before the drop."
    );
}
