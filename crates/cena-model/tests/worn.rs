//! What the character wears and what the wandolier holds: the `inv` and
//! `reserve` streams, typed (`state/worn.rs`).
//!
//! The worn list is read from committed wire: the login cut
//! (`crates/cena-protocol/tests/fixtures/inventory_container.xml`, whose `inv`
//! block is popped) and three hunt captures
//! (`crates/cena-behavior/tests/fixtures/*.xml`, whose 11 blocks are all
//! closed by the prompt, never popped). The login cut ends before its prompt,
//! so the tests append one. Everything about `Placed alongside you:` and the
//! `reserve` stream is SYNTHETIC: no committed fixture carries either, so the
//! lines are built from `VellumFE`'s test shape
//! (`reference/VellumFE/src/core/game_objects/mod.rs:728-745`) and from Lich's
//! parser (`xmlparser.rb:1185-1186`, every `<a>` in the stream).

use cena_model::GameState;
use cena_protocol::Parser;

const LOGIN: &str = include_str!("../../cena-protocol/tests/fixtures/inventory_container.xml");
const ARCH_KILL: &str = include_str!("../../cena-behavior/tests/fixtures/arch_kill.xml");
const SMITHY_KILL: &str = include_str!("../../cena-behavior/tests/fixtures/smithy_kill.xml");
const SMITHY_ENGAGE: &str = include_str!("../../cena-behavior/tests/fixtures/smithy_engage.xml");
const PROMPT: &str = "<prompt time=\"100\">&gt;</prompt>\n";

fn feed(state: &mut GameState, wire: &str) {
    let mut parser = Parser::new();
    for frame in parser.push_bytes(wire.as_bytes()) {
        state.apply(&frame);
    }
}

fn ids(items: Option<&[cena_model::state::containers::ItemRef]>) -> Vec<String> {
    items
        .unwrap_or_default()
        .iter()
        .map(|i| i.id.clone())
        .collect()
}

#[test]
fn the_login_list_is_every_worn_item_kept_at_the_prompt() {
    let mut state = GameState::default();
    assert_eq!(state.worn.items(), None, "never stated");
    feed(&mut state, LOGIN);
    assert_eq!(
        state.worn.items(),
        None,
        "the list is kept at the prompt, and the cut has none yet"
    );
    feed(&mut state, PROMPT);
    let worn = state.worn.items().unwrap_or_default();
    let got: Vec<(&str, &str, &str)> = worn
        .iter()
        .map(|i| (i.id.as_str(), i.noun.as_str(), i.text.as_str()))
        .collect();
    assert_eq!(
        got,
        [
            ("2376165", "arcarium", "dark sapphire vangaris arcarium"),
            ("2376164", "arcarium", "dark sapphire aversaris arcarium"),
            ("2376163", "anklet", "spiderweb-patterned nightshade anklet"),
            ("2376129", "kit", "cire leather herb kit"),
            ("2376128", "locus", "gilded locus"),
            ("2376124", "keyring", "ruby-inlaid carved obsidian keyring"),
        ]
    );
    assert_eq!(state.worn.at_feet(), Some(&[][..]));
    assert_eq!(state.worn.wears("anklet"), Some(true));
    assert_eq!(
        state.worn.wears("cloak"),
        Some(false),
        "the stow container is not worn"
    );
}

#[test]
fn an_unpopped_hunt_list_stops_at_the_rounds_prose() {
    let mut state = GameState::default();
    feed(&mut state, ARCH_KILL);
    let worn = ids(state.worn.items());
    // MEASURED: `arch_kill.xml:29-59` are the 31 item lines of each block.
    assert_eq!(worn.len(), 31);
    assert_eq!(
        worn.first().map(String::as_str),
        Some("407520242"),
        "the gauntlets"
    );
    assert_eq!(
        worn.last().map(String::as_str),
        Some("407520391"),
        "the vangaris arcarium"
    );
    // `arch_kill.xml:60`: the prose still on the `inv` stream names the
    // arrow and the bow in hand. Neither is worn.
    assert!(!worn.contains(&"407647314".to_owned()), "the nocked arrow");
    assert!(!worn.contains(&"407520241".to_owned()), "the bow");
    assert_eq!(state.worn.wears("arrow"), Some(false));
    assert_eq!(state.worn.wears("bow"), Some(false));
}

#[test]
fn every_committed_hunt_list_holds_no_hand_item() {
    for (name, wire) in [
        ("arch_kill", ARCH_KILL),
        ("smithy_kill", SMITHY_KILL),
        ("smithy_engage", SMITHY_ENGAGE),
    ] {
        let mut state = GameState::default();
        feed(&mut state, wire);
        let worn = state.worn.items().unwrap_or_default();
        assert!(!worn.is_empty(), "{name}: a list was read");
        let in_hand: Vec<&str> = [state.left_hand.id(), state.right_hand.id()]
            .into_iter()
            .flatten()
            .collect();
        assert!(
            worn.iter().all(|i| !in_hand.contains(&i.id.as_str())),
            "{name}: nothing in hand is worn"
        );
        assert!(
            worn.iter().all(|i| i.noun != "arrow"),
            "{name}: the round's arrows are prose, not the list"
        );
    }
}

#[test]
fn a_list_of_nothing_is_stated_empty() {
    let mut state = GameState::default();
    feed(
        &mut state,
        "<clearStream id='inv'/><pushStream id='inv'/>Your worn items are:\n<popStream/>\n",
    );
    feed(&mut state, PROMPT);
    assert_eq!(state.worn.items(), Some(&[][..]));
    assert_eq!(state.worn.wears("anklet"), Some(false));
}

#[test]
fn a_list_that_does_not_open_with_the_header_is_not_taken() {
    let mut state = GameState::default();
    feed(
        &mut state,
        "<pushStream id='inv'/>Something else entirely:\n  a <a exist=\"1\" noun=\"ring\">ring</a>\n<popStream/>\n",
    );
    feed(&mut state, PROMPT);
    assert_eq!(
        state.worn.items(),
        None,
        "an unseen shape is not 'wears nothing'"
    );
}

#[test]
fn a_new_list_replaces_the_last_whole() {
    let mut state = GameState::default();
    feed(&mut state, LOGIN);
    feed(&mut state, PROMPT);
    feed(
        &mut state,
        "<clearStream id='inv'/><pushStream id='inv'/>Your worn items are:\n  a <a exist=\"2376128\" noun=\"locus\">gilded locus</a>\n<popStream/>\n",
    );
    feed(&mut state, PROMPT);
    assert_eq!(ids(state.worn.items()), ["2376128"]);
    assert_eq!(state.worn.wears("anklet"), Some(false), "removed since");
}

#[test]
fn items_after_placed_alongside_you_are_at_your_feet() {
    let mut state = GameState::default();
    feed(
        &mut state,
        "<pushStream id='inv'/>Your worn items are:\n  some <a exist=\"511640642\" noun=\"gloves\">darkened triton hide gloves</a> with faenor-plated knuckles\n\nPlaced alongside you:\n  a <a exist=\"511640700\" noun=\"jewel\">glimmering jewel</a>\n<popStream/>\n",
    );
    feed(&mut state, PROMPT);
    assert_eq!(ids(state.worn.items()), ["511640642"]);
    assert_eq!(ids(state.worn.at_feet()), ["511640700"]);
    assert_eq!(
        state.worn.wears("jewel"),
        Some(false),
        "at your feet is not worn"
    );
}

#[test]
fn an_indented_bold_line_is_not_an_item() {
    // A combat line indented like an item, naming a creature in bold.
    let mut state = GameState::default();
    feed(
        &mut state,
        "<pushStream id='inv'/>Your worn items are:\n  a <a exist=\"7\" noun=\"ring\">ring</a>\n  <pushBold/>a <a exist=\"8\" noun=\"troll\">troll</a><popBold/> attacks!\n  a <a exist=\"9\" noun=\"cloak\">cloak</a>\n",
    );
    feed(&mut state, PROMPT);
    assert_eq!(ids(state.worn.items()), ["7"], "the list ends at the troll");
}

#[test]
fn a_line_not_indented_by_exactly_two_spaces_ends_the_list() {
    // `arch_kill.xml:68`, a line of the round's prose indented by one space
    // and naming the arrow unbolded, as if it followed the list.
    let mut state = GameState::default();
    feed(
        &mut state,
        "<pushStream id='inv'/>Your worn items are:\n  a <a exist=\"7\" noun=\"ring\">ring</a>\n ** A radiant afterimage of the <a exist=\"407647314\" noun=\"arrow\">arrow</a> appears in your ready hand, coalescing to replace its predecessor! **\n",
    );
    feed(&mut state, PROMPT);
    assert_eq!(ids(state.worn.items()), ["7"]);

    // SYNTHETIC: four spaces, the same unbolded link.
    let mut state = GameState::default();
    feed(
        &mut state,
        "<pushStream id='inv'/>Your worn items are:\n  a <a exist=\"7\" noun=\"ring\">ring</a>\n    the <a exist=\"407647314\" noun=\"arrow\">arrow</a> falls\n",
    );
    feed(&mut state, PROMPT);
    assert_eq!(ids(state.worn.items()), ["7"]);
}

#[test]
fn a_reconnect_keeps_the_list_and_drops_one_half_arrived() {
    let mut state = GameState::default();
    feed(&mut state, LOGIN);
    feed(&mut state, PROMPT);
    feed(
        &mut state,
        "<pushStream id='inv'/>Your worn items are:\n  a <a exist=\"1\" noun=\"ring\">ring</a>\n",
    );
    state.invalidate_for_reconnect();
    feed(&mut state, PROMPT);
    assert_eq!(
        ids(state.worn.items()).len(),
        6,
        "the login's list, kept whole"
    );
}

// --- the reserve --------------------------------------------------------------

#[test]
fn a_popped_reserve_list_keeps_every_object() {
    let mut state = GameState::default();
    assert_eq!(
        state.reserve.items(),
        None,
        "nil until the first list, as Lich's is"
    );
    feed(
        &mut state,
        "<clearStream id='reserve'/><pushStream id='reserve'/>  a <a exist=\"41\" noun=\"wand\">polished oak wand</a>\n  a <a exist=\"42\" noun=\"wand\">twisted iron wand</a> and a <a exist=\"43\" noun=\"rod\">thin rod</a>\n<popStream/>\n",
    );
    feed(&mut state, PROMPT);
    assert_eq!(
        ids(state.reserve.items()),
        ["41", "42", "43"],
        "every <a>, Lich's rule"
    );
}

#[test]
fn a_reserve_list_the_prompt_closed_is_dropped() {
    // The prompt must reach the parser that holds the stream open: it is the
    // parser that force-closes it (`Frame::StreamPopForced`).
    let mut state = GameState::default();
    feed(
        &mut state,
        &format!(
            "<pushStream id='reserve'/>  a <a exist=\"41\" noun=\"wand\">polished oak wand</a>\nYou swing a <a exist=\"9\" noun=\"sword\">sword</a>.\n{PROMPT}"
        ),
    );
    assert_eq!(
        state.reserve.items(),
        None,
        "no line shape is known to stop at"
    );

    // A popped list after it is kept, and then a torn one leaves it standing.
    feed(
        &mut state,
        &format!(
            "<pushStream id='reserve'/>  a <a exist=\"41\" noun=\"wand\">polished oak wand</a>\n<popStream/>\n{PROMPT}"
        ),
    );
    assert_eq!(ids(state.reserve.items()), ["41"]);
    feed(
        &mut state,
        &format!(
            "<pushStream id='reserve'/>  a <a exist=\"99\" noun=\"arrow\">arrow</a>\n{PROMPT}"
        ),
    );
    assert_eq!(ids(state.reserve.items()), ["41"]);
}

#[test]
fn a_reserve_list_of_nothing_is_stated_empty() {
    let mut state = GameState::default();
    feed(&mut state, "<pushStream id='reserve'/><popStream/>\n");
    feed(&mut state, PROMPT);
    assert_eq!(state.reserve.items(), Some(&[][..]));
}
