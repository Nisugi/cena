//! The bounty a character is on, and what the guild will do next.
//!
//! `bounty.rs` ports Lich's parser and has its own tests. These are about the
//! **consumer**: that the classifier is reached from a running `GameState` at
//! all, and that the guild's own answers -- which the parser does not read --
//! land somewhere.
//!
//! MEASURED 2026-09-21, before any of this was written: `bounty::` appeared
//! nowhere outside `bounty.rs` and its tests, and `GameState` had no field for
//! one. A fully ported, thoroughly tested classifier that nothing called.
//!
//! Task descriptions are Lich's own (`bounty/parser.rb`); the guild lines are
//! `ebounty.lic`'s (`:950-961`, `:2438`).

use cena_model::GameState;
use cena_model::state::bounty::TaskKind;
use cena_model::state::bounty_status::{
    BountyStatus, Refusal, classify_refusal, vouchers_remaining,
};
use cena_protocol::Parser;

/// A cull task, as the guild words it.
const CULL: &str = "You have been tasked to suppress kobold activity in the Rift.  \
     You need to kill 12 more of them to complete your task.";

const NONE: &str = "You are not currently assigned a task.";

fn fed(wire: &str) -> GameState {
    let mut state = GameState::default();
    for frame in Parser::new().push_bytes(wire.as_bytes()) {
        state.apply(&frame);
    }
    state
}

/// One chunk of main-window text, closed by a prompt.
fn chunk(text: &str) -> GameState {
    fed(&format!("{text}\n<prompt time=\"1\">&gt;</prompt>\n"))
}

#[test]
fn a_task_reaches_the_model_from_the_wire() {
    // The whole point: the classifier is REACHED. Before this, a running
    // session could parse this line and had nowhere to put it.
    let state = chunk(CULL);
    let task = state.bounty.task().expect("the task reached the model");
    assert_eq!(task.kind, TaskKind::Cull);
    assert_eq!(task.creature(), Some("kobold"));
    assert_eq!(task.number(), Some(12));
    assert_eq!(task.area(), Some("Rift"));
    assert_eq!(state.bounty.has_work(), Some(true));
}

#[test]
fn nobody_asked_is_not_no_bounty() {
    // §5.2. A character who has never run `bounty` is not one with none.
    assert_eq!(GameState::default().bounty.task(), None);
    assert_eq!(GameState::default().bounty.has_work(), None);

    let stated = chunk(NONE);
    assert_eq!(stated.bounty.kind(), Some(TaskKind::None));
    assert_eq!(
        stated.bounty.has_work(),
        Some(false),
        "the game SAID there is none"
    );
}

#[test]
fn a_new_task_replaces_the_old_one() {
    let mut state = BountyStatus::default();
    assert!(state.read_line(CULL));
    assert!(state.read_line(NONE), "the task was turned in");
    assert_eq!(state.kind(), Some(TaskKind::None));
}

#[test]
fn reading_the_same_task_twice_reports_no_change() {
    // The shape `consume_standing` uses: a re-read that finds everything
    // identical must not mark the character dirty.
    let mut state = BountyStatus::default();
    assert!(state.read_line(CULL), "the first time is a change");
    assert!(!state.read_line(CULL), "the second is not");
}

#[test]
fn the_guild_refusals_are_read() {
    // `ebounty.lic:950-961`. None of these is a task description, so the
    // parser returns nothing for all three.
    assert_eq!(
        classify_refusal("You have already been assigned a task, Nisugi."),
        Some(Refusal::AlreadyAssigned)
    );
    assert_eq!(
        classify_refusal("Come back in about 27 minutes if you want another task."),
        Some(Refusal::Wait { minutes: 27 })
    );
    assert_eq!(
        classify_refusal("Come back in about 1 minute if you want another task."),
        Some(Refusal::Wait { minutes: 1 }),
        "the singular is the same line"
    );
    assert_eq!(
        classify_refusal("I don't have any tasks for you right now."),
        Some(Refusal::NoneAvailable)
    );
    assert_eq!(classify_refusal(CULL), None, "a task is not a refusal");
}

#[test]
fn a_refusal_reaches_the_model_and_a_task_clears_it() {
    let mut state = BountyStatus::default();
    state.read_line("Come back in about 27 minutes if you want another task.");
    assert_eq!(state.refusal, Some(Refusal::Wait { minutes: 27 }));
    // Being told what the task IS means the guild is no longer refusing.
    assert!(state.read_line(CULL));
    assert_eq!(state.refusal, None);
}

#[test]
fn the_voucher_count_is_read_with_its_separators() {
    // `ebounty.lic:2438`'s pattern is `([0-9,]+)`, so the game spells large
    // counts with commas.
    assert_eq!(
        vouchers_remaining("You have 3 expedited task reassignment vouchers remaining."),
        Some(3)
    );
    assert_eq!(
        vouchers_remaining("You have 1,024 expedited task reassignment vouchers remaining."),
        Some(1024)
    );
    assert_eq!(vouchers_remaining("You have 3 silvers."), None);

    let state = chunk("You have 3 expedited task reassignment vouchers remaining.");
    assert_eq!(state.bounty.vouchers, Some(3));
}

#[test]
fn an_ordinary_line_changes_nothing() {
    // The reader runs on every line of every chunk, so it must recognise
    // rather than assume.
    let state = chunk("You see nothing unusual.");
    assert_eq!(state.bounty.task(), None);
    assert_eq!(state.bounty.refusal, None);
    assert_eq!(state.bounty.vouchers, None);
}

#[test]
fn a_reconnect_forgets_it_because_nothing_resends_it() {
    let mut state = chunk(CULL);
    assert!(state.bounty.task().is_some(), "guard");
    state.invalidate_for_reconnect();
    assert_eq!(state.bounty.task(), None);
    assert_eq!(state.bounty.has_work(), None);
}

#[test]
fn an_assignment_is_work_and_a_finished_task_is_not() {
    // Two stages: the assignment says go and ask, the task says what to do.
    // `is_actionable` is what a caller checks rather than "is it not None".
    let assigned = chunk(
        "You have succeeded in your task and can return to the Adventurer's Guild to report.",
    );
    assert_eq!(assigned.bounty.kind(), Some(TaskKind::Taskmaster));
    assert_eq!(
        assigned.bounty.has_work(),
        Some(false),
        "a finished task is not work to do"
    );
}
