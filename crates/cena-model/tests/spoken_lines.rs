//! A line a person said is never the game's: not a bounty, an incident, a
//! shop's offer or a creature's ending (the crate review of 2026-10-01,
//! pattern 2: MO-B-1, MO-B-4, MO-B-5, MO-B-6, MO-E-1).
//!
//! Each test feeds the exact spoken line through the parser and the model,
//! so it reaches the reader the finding names; each has a control line, the
//! same words as the game's own prose, that the reader does take.

use cena_model::state::incident::{Disarm, Incident};
use cena_model::{Ending, GameState, LootFact};
use cena_protocol::Parser;

const PROMPT: &str = "<prompt time=\"100\">&gt;</prompt>\n";

fn feed(state: &mut GameState, wire: &str) {
    let mut parser = Parser::new();
    for frame in parser.push_bytes(wire.as_bytes()) {
        state.apply(&frame);
    }
}

/// `body` said aloud by another player, as the wire writes it, and a prompt.
fn said(body: &str) -> String {
    format!(
        "<preset id='speech'><a exist=\"-10007833\" noun=\"Pukk\">Pukk</a> says</preset>, \"{body}\"\n{PROMPT}"
    )
}

/// The same words with no preset: what the game itself would send.
fn prose(body: &str) -> String {
    format!("{body}\n{PROMPT}")
}

fn after(wire: &str) -> GameState {
    let mut state = GameState::default();
    feed(&mut state, wire);
    state
}

#[test]
fn a_spoken_assignment_is_not_the_bounty_task() {
    let line = "It appears they have a bandit problem they'd like you to solve.";
    assert!(after(&prose(line)).bounty.kind().is_some(), "the control");
    assert_eq!(after(&said(line)).bounty.kind(), None, "MO-B-1");
}

#[test]
fn a_spoken_disarm_is_not_an_incident() {
    let line = "Your sword tears free from your hands and floats away.";
    let mut state = after(&said(line));
    assert_eq!(state.take_incidents(), [], "MO-B-4");
}

#[test]
fn the_game_disarming_you_names_the_weapon_after_your() {
    // Lich's pattern needs the weapon's own link after `Your`
    // (`combat/defs/messages.rb:94`); the control for the test above.
    let mut state = after(&format!(
        "Your <a exist=\"123\" noun=\"sword\">vultite sword</a> tears free from your hands and floats away!\n{PROMPT}"
    ));
    let incidents = state.take_incidents();
    assert!(
        matches!(
            incidents.as_slice(),
            [Incident::Disarmed { how: Disarm::Telekinetic, weapon: Some(w) }] if w.noun == "sword"
        ),
        "{incidents:?}"
    );
}

#[test]
fn a_spoken_hive_trap_is_not_an_incident() {
    let line = "The ground churns violently as flashes of chitin jut from its depths!";
    assert!(
        !after(&prose(line)).take_incidents().is_empty(),
        "the control"
    );
    assert_eq!(after(&said(line)).take_incidents(), [], "MO-B-5");
}

fn appraisals(state: &mut GameState) -> Vec<Option<u64>> {
    state
        .take_loot()
        .into_iter()
        .flat_map(|chunk| chunk.facts)
        .filter_map(|fact| match fact {
            LootFact::Appraised { value, .. } => Some(value),
            _ => None,
        })
        .collect()
}

#[test]
fn a_spoken_offer_is_not_an_appraisal() {
    let line = "I'll give you 5 silver for it.";
    assert_eq!(
        appraisals(&mut after(&prose(line))),
        [Some(5)],
        "the control"
    );
    assert_eq!(appraisals(&mut after(&said(line))), [], "MO-B-6");
}

#[test]
fn a_spoken_collapse_does_not_end_the_captain() {
    let mut state = after(
        "<component id='room objs'>  You also see<crtrStatus exist=\"7001\" hostile=\"1\"/><b> <pushBold/>a <a exist=\"7001\" noun=\"captain\">battle-worn Empyrean captain</a><popBold/></b>.</component>\n",
    );
    feed(
        &mut state,
        &said("the battle-worn Empyrean captain collapses"),
    );
    let captain = state.creatures().get(7001).expect("listed");
    assert_eq!(captain.ending(), None, "MO-E-1");
    assert!(captain.valid_target());
    feed(
        &mut state,
        &prose("the battle-worn Empyrean captain collapses"),
    );
    let captain = state.creatures().get(7001).expect("listed");
    assert_eq!(captain.ending(), Some(Ending::Collapsed), "the control");
}

#[test]
fn a_spoken_line_is_a_message_and_not_the_commands_answer() {
    let state = after(&said("Hello."));
    assert_eq!(state.messages.all().len(), 1, "still heard as speech");
    let mut state = GameState::default();
    feed(
        &mut state,
        "<preset id='speech'><a exist=\"-1\" noun=\"Pukk\">Pukk</a> says</preset>, \"Hello.\"\n",
    );
    assert_eq!(
        state.open_chunk_len(),
        0,
        "not in the chunk automation reads"
    );
}

/// An NPC answering a question is the game's answer: the speaker stands
/// before the preset, bolded, and the preset holds only the verb (MEASURED,
/// the meddler's eleven lines in `E:\Gemstone\dev\lich-5\logs`).
#[test]
fn an_npcs_reply_stays_the_games() {
    let mut state = GameState::default();
    feed(
        &mut state,
        "The <pushBold/><a exist=\"-527782\" noun=\"meddler\">meddler</a><popBold/> <preset id=\"speech\">replies</preset>, \"Here you are,\" and shows you a service menu.\n",
    );
    let lines = state.open_chunk().lines();
    assert_eq!(lines.len(), 1);
    assert!(!lines[0].is_spoken());
}
