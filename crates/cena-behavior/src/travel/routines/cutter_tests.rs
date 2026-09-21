//! Table tests for [`super::Cutter`], beside it so the solver's file stays
//! readable.

use cena_session::hands::Hand;
use cena_session::{ChunkLine, InventoryItem, Link};

use super::super::testing::Scene;
use super::*;

const MINE: &str = "In the Common language, it reads, \"Passage for Walker on the cutter.\"";
const NOT_MINE: &str = "In the Common language, it reads, \"Passage for Other on the cutter.\"";
const PRICE: &str = "Percy whispers, \"Look -- I need 500 coins for something like that.\"";
const TAKEN: &str = "Percy quietly takes your money and hands you a ticket.";

fn holding(id: &str, noun: &str) -> Hand {
    Hand::Holding {
        id: Some(id.into()),
        noun: Some(noun.into()),
        name: noun.into(),
    }
}

fn item(id: &str, noun: &str, parent: &str) -> InventoryItem {
    InventoryItem {
        id: id.into(),
        relation: if parent == "player" { "worn" } else { "in" }.into(),
        parent: parent.into(),
        noun: noun.into(),
        ..InventoryItem::default()
    }
}

/// The pier under the bridge, the walker named, hands empty.
fn pier() -> Scene {
    let mut scene = Scene::at(18677, 2);
    scene.state.character.name = Some("Walker".into());
    scene.state.right_hand = Hand::Empty;
    scene.state.left_hand = Hand::Empty;
    scene
}

fn with(mut scene: Scene, items: &[InventoryItem]) -> Scene {
    scene
        .state
        .inventory_snapshot
        .apply_snapshot("pier", items, &[], None);
    scene
}

fn buying(mut scene: Scene) -> Scene {
    scene
        .walker
        .settings
        .insert("get_silvers".into(), "true".into());
    scene
}

/// A line listing containers, each a link with an id.
fn listing(ids: &[&str]) -> Vec<ChunkLine> {
    ids.iter()
        .map(|id| {
            let mut line = ChunkLine::plain("a cloak");
            line.runs.runs[0].link = Some(Link {
                kind: LinkKind::Exist {
                    id: (*id).into(),
                    noun: "cloak".into(),
                },
                text: "a cloak".into(),
                coord: None,
            });
            line
        })
        .collect()
}

fn put(command: &str) -> Next {
    Next::Put(command.into())
}

fn go(command: &str) -> Next {
    Next::Go(command.into())
}

fn wait(pier: &Pier) -> Next {
    Next::Await(pier.arrived.map(str::to_owned).to_vec(), CUTTER_MS)
}

/// Drive a walker with no ticket anywhere to the point of buying.
fn nothing_found(cutter: &mut Cutter, scene: &Scene) -> Next {
    assert_eq!(scene.ask(cutter), one(Action::EmptyHands));
    assert_eq!(scene.ask(cutter), put("inventory containers"));
    scene.ask(cutter)
}

#[test]
fn a_ticket_in_the_right_hand_boards_at_once() {
    let mut cutter = Cutter::rivers_rest();
    let mut scene = pier().showing("gangplank");
    scene.state.right_hand = holding("5", "scrip");
    assert_eq!(scene.ask(&mut cutter), put("look #5"));
    assert_eq!(scene.answered(&[MINE]).ask(&mut cutter), go("go gangplank"));
    assert_eq!(Scene::at(2, 2).ask(&mut cutter), Next::Done);
}

#[test]
fn a_ticket_in_the_left_hand_is_swapped() {
    let mut cutter = Cutter::rivers_rest();
    let mut scene = pier();
    scene.state.right_hand = holding("4", "note");
    scene.state.left_hand = holding("5", "scrip");
    assert_eq!(scene.ask(&mut cutter), put("look #4"));
    let mut scene = scene.answered(&[NOT_MINE]);
    assert_eq!(scene.ask(&mut cutter), put("look #5"));
    scene = scene.answered(&[MINE]);
    assert_eq!(scene.ask(&mut cutter), put("swap"));
    assert_eq!(scene.ask(&mut cutter), wait(&RIVERS_REST));
}

#[test]
fn somebody_elses_ticket_is_not_the_ticket() {
    let mut cutter = Cutter::rivers_rest();
    let mut scene = pier();
    scene.state.left_hand = holding("5", "scrip");
    assert_eq!(scene.ask(&mut cutter), put("look #5"));
    assert_eq!(
        scene.answered(&[NOT_MINE]).ask(&mut cutter),
        one(Action::EmptyHands)
    );
}

#[test]
fn a_ticket_in_a_known_container_is_found_and_taken() {
    let mut cutter = Cutter::rivers_rest();
    let scene = with(
        pier(),
        &[
            item("10", "cloak", "player"),
            item("11", "note", "10"),
            item("12", "scrip", "10"),
            item("13", "dagger", "10"),
        ],
    );
    assert_eq!(scene.ask(&mut cutter), one(Action::EmptyHands));
    assert_eq!(scene.ask(&mut cutter), put("look #11"));
    let scene = scene.answered(&["You see nothing unusual."]);
    assert_eq!(scene.ask(&mut cutter), put("look #12"));
    let scene = scene.answered(&[MINE]);
    assert_eq!(scene.ask(&mut cutter), put("get #12"));
    assert_eq!(scene.ask(&mut cutter), wait(&RIVERS_REST));
}

#[test]
fn a_shut_container_is_opened_searched_and_shut_again() {
    let mut cutter = Cutter::rivers_rest();
    // 10's contents are known, so it is not opened; 20 is not the walker's.
    let mut scene = with(
        pier(),
        &[
            item("10", "cloak", "player"),
            item("13", "dagger", "10"),
            item("30", "sack", "player"),
        ],
    );
    assert_eq!(scene.ask(&mut cutter), one(Action::EmptyHands));
    assert_eq!(scene.ask(&mut cutter), put("inventory containers"));
    scene.answer = listing(&["10", "20", "30"]);
    assert_eq!(scene.ask(&mut cutter), put("open #30"));
    // Opening it is what teaches the model what it holds.
    let scene = with(
        scene,
        &[item("30", "sack", "player"), item("31", "paper", "30")],
    )
    .answered(&["You open a sack."]);
    assert_eq!(scene.ask(&mut cutter), put("look #31"));
    let scene = scene.answered(&[MINE]);
    assert_eq!(scene.ask(&mut cutter), put("get #31"));
    assert_eq!(scene.ask(&mut cutter), put("close #30"));
    assert_eq!(scene.ask(&mut cutter), wait(&RIVERS_REST));
}

#[test]
fn a_container_already_open_is_looked_in_and_left_open() {
    let mut cutter = Cutter::rivers_rest();
    let mut scene = with(pier(), &[item("30", "sack", "player")]);
    scene.ask(&mut cutter);
    scene.ask(&mut cutter);
    scene.answer = listing(&["30"]);
    assert_eq!(scene.ask(&mut cutter), put("open #30"));
    let scene = scene.answered(&["That is already open."]);
    assert_eq!(scene.ask(&mut cutter), put("look in #30"));
    // Nothing in it: no `close`, and with no leave to buy, the trip stops.
    assert_eq!(scene.ask(&mut cutter), Next::Stop(NO_TICKET.into()));
}

#[test]
fn an_opened_container_without_the_ticket_is_shut_before_the_next() {
    let mut cutter = Cutter::rivers_rest();
    let mut scene = with(
        pier(),
        &[item("30", "sack", "player"), item("40", "pack", "player")],
    );
    scene.ask(&mut cutter);
    scene.ask(&mut cutter);
    scene.answer = listing(&["30", "40"]);
    assert_eq!(scene.ask(&mut cutter), put("open #30"));
    let scene = scene.answered(&["You open a sack."]);
    assert_eq!(scene.ask(&mut cutter), put("close #30"));
    assert_eq!(scene.ask(&mut cutter), put("open #40"));
}

#[test]
fn no_ticket_and_no_leave_to_buy_stops_the_trip() {
    let mut cutter = Cutter::rivers_rest();
    assert_eq!(
        nothing_found(&mut cutter, &pier()),
        Next::Stop(NO_TICKET.into())
    );
}

#[test]
fn rivers_rest_buys_by_walking_to_the_seller_the_bank_and_back() {
    let mut cutter = Cutter::rivers_rest();
    let scene = buying(pier());
    assert_eq!(nothing_found(&mut cutter, &scene), Next::Pause(2000));
    assert_eq!(scene.ask(&mut cutter), Next::WalkTo(RoomId(11748)));
    assert_eq!(scene.ask(&mut cutter), put("ask percy about ticket"));
    let scene = scene.answered(&[PRICE]);
    assert_eq!(scene.ask(&mut cutter), Next::WalkTo(RoomId(10911)));
    assert_eq!(scene.ask(&mut cutter), put("withdraw 500 silvers"));
    let scene = scene.answered(&["The teller hands you 500 silvers."]);
    assert_eq!(scene.ask(&mut cutter), Next::WalkTo(RoomId(11748)));
    assert_eq!(scene.ask(&mut cutter), put("ask percy about ticket"));
    // Whatever he says the second time, it is back to where this began.
    let scene = scene.answered(&[PRICE]);
    assert_eq!(scene.ask(&mut cutter), Next::WalkTo(RoomId(18677)));
    assert_eq!(scene.ask(&mut cutter), wait(&RIVERS_REST));
}

#[test]
fn silver_already_in_hand_skips_the_bank() {
    let mut cutter = Cutter::rivers_rest();
    let scene = buying(pier());
    nothing_found(&mut cutter, &scene);
    scene.ask(&mut cutter);
    scene.ask(&mut cutter);
    let scene = scene.answered(&[TAKEN]);
    assert_eq!(scene.ask(&mut cutter), Next::WalkTo(RoomId(18677)));
}

#[test]
fn too_poor_stops_the_trip_and_says_so() {
    let mut cutter = Cutter::rivers_rest();
    let scene = buying(pier());
    nothing_found(&mut cutter, &scene);
    scene.ask(&mut cutter);
    scene.ask(&mut cutter);
    let scene = scene.answered(&[PRICE]);
    scene.ask(&mut cutter);
    scene.ask(&mut cutter);
    let scene = scene.answered(&["You don't seem to have that much in your account."]);
    assert_eq!(scene.ask(&mut cutter), Next::Stop(RIVERS_REST.poor.into()));
}

#[test]
fn marshtown_buys_by_upstreams_moves() {
    let mut cutter = Cutter::marshtown();
    let scene = buying(pier());
    assert_eq!(nothing_found(&mut cutter, &scene), Next::Pause(3000));
    let Way::Moves(there) = MARSHTOWN.to_seller else {
        panic!("Marshtown walks by moves");
    };
    for way in there {
        assert_eq!(scene.ask(&mut cutter), go(way));
    }
    assert_eq!(scene.ask(&mut cutter), put("ask jyhm about ticket"));
    let scene = scene.answered(&["Jyhm quietly takes your money."]);
    let sent: Vec<Next> = (0..8).map(|_| scene.ask(&mut cutter)).collect();
    let mut back: Vec<Next> = ["out", "east", "east", "southeast", "south", "west"]
        .map(go)
        .to_vec();
    back.extend([go("go pier"), wait(&MARSHTOWN)]);
    assert_eq!(sent, back);
}

#[test]
fn a_move_that_fails_on_the_way_fails_the_errand() {
    let mut cutter = Cutter::marshtown();
    let scene = buying(pier());
    nothing_found(&mut cutter, &scene);
    assert_eq!(scene.ask(&mut cutter), go("north"));
    let mut stuck = buying(pier());
    stuck.failed = true;
    assert_eq!(stuck.ask(&mut cutter), Next::Failed);
}

#[test]
fn a_hidden_walker_unhides_before_asking_and_before_boarding() {
    let mut cutter = Cutter::rivers_rest();
    let mut scene = buying(pier());
    scene.walker.flags.insert("hidden".into(), true);
    nothing_found(&mut cutter, &scene);
    assert_eq!(scene.ask(&mut cutter), Next::WalkTo(RoomId(11748)));
    assert_eq!(scene.ask(&mut cutter), put("unhide"));
    assert_eq!(scene.ask(&mut cutter), put("ask percy about ticket"));
    let scene = scene.answered(&[TAKEN]);
    scene.ask(&mut cutter);
    let mut scene = scene.showing("gangplank");
    scene.walker.flags.insert("invisible".into(), true);
    scene.walker.flags.insert("hidden".into(), false);
    assert_eq!(scene.ask(&mut cutter), put("unhide"));
    assert_eq!(scene.ask(&mut cutter), go("go gangplank"));
}

#[test]
fn the_cutter_is_waited_for_and_boarded_when_it_comes() {
    let mut cutter = Cutter::rivers_rest();
    let mut scene = pier();
    scene.state.right_hand = holding("5", "scrip");
    scene.ask(&mut cutter);
    let scene = scene.answered(&[MINE]);
    assert_eq!(scene.ask(&mut cutter), wait(&RIVERS_REST));
    assert_eq!(scene.ask(&mut cutter), go("go gangplank"));
    assert_eq!(scene.ask(&mut cutter), Next::Done);
}

#[test]
fn a_cutter_that_never_comes_fails_and_the_wait_is_not_repeated() {
    let mut cutter = Cutter::rivers_rest();
    let mut scene = pier();
    scene.state.right_hand = holding("5", "scrip");
    scene.ask(&mut cutter);
    let mut scene = scene.answered(&[MINE]);
    assert_eq!(scene.ask(&mut cutter), wait(&RIVERS_REST));
    scene.failed = true;
    assert_eq!(scene.ask(&mut cutter), Next::Failed);
}

#[test]
fn a_gangplank_that_cannot_be_boarded_waits_for_the_next_call_once() {
    let mut cutter = Cutter::rivers_rest();
    let mut scene = pier().showing("gangplank");
    scene.state.right_hand = holding("5", "scrip");
    scene.ask(&mut cutter);
    let mut scene = scene.answered(&[MINE]);
    assert_eq!(scene.ask(&mut cutter), go("go gangplank"));
    scene.failed = true;
    assert_eq!(scene.ask(&mut cutter), wait(&RIVERS_REST));
    scene.failed = false;
    assert_eq!(scene.ask(&mut cutter), go("go gangplank"));
    // The second refusal is the end of it: upstream does not loop either.
    scene.failed = true;
    assert_eq!(scene.ask(&mut cutter), Next::Done);
}
