//! Stocking the herb container (`plan/36` Stage 4): eherbs' minimums, what
//! the town's herbalist sells, the menu, the purchase and the bank.

use cena_behavior::heal::stock::{Bucket, Step};
use cena_behavior::heal::{HealProfile, Reply, Stocker};
use cena_map::RoomId;
use cena_session::herbs::HerbKind;
use cena_session::{Frame, GameState, Link, LinkKind, Run, Runs, TextFrame};

const LANDING: &str = "the town of Wehnimer's Landing";
const HOME: RoomId = RoomId(1);
const SHOP: RoomId = RoomId(2);
const BANK: RoomId = RoomId(3);

fn exist(id: &str, noun: &str, text: &str) -> Link {
    Link {
        kind: LinkKind::Exist {
            id: id.to_owned(),
            noun: noun.to_owned(),
        },
        text: text.to_owned(),
        coord: None,
    }
}

#[expect(
    clippy::default_trait_access,
    reason = "the run's style type is not re-exported for behaviors; only the link matters"
)]
fn inside(state: &mut GameState, id: &str, noun: &str, text: &str) {
    state.apply(&Frame::ContainerItem {
        container_id: "700".to_owned(),
        content: Runs {
            runs: vec![Run {
                text: text.to_owned(),
                style: Default::default(),
                link: Some(exist(id, noun, text)),
                inner_link: None,
            }],
        },
    });
}

fn hand(state: &mut GameState, right: bool, item: Option<(&str, &str, &str)>) {
    let (text, link) = match item {
        Some((id, noun, text)) => (text.to_owned(), Some(exist(id, noun, text))),
        None => ("Empty".to_owned(), None),
    };
    state.apply(&if right {
        Frame::RightHand { item: text, link }
    } else {
        Frame::LeftHand { item: text, link }
    });
}

/// One line of text, pieces with an optional link, then a prompt.
#[expect(
    clippy::default_trait_access,
    reason = "the text's style type is not re-exported for behaviors"
)]
fn say(state: &mut GameState, parts: &[(&str, Option<Link>)]) {
    for (at, (text, link)) in parts.iter().enumerate() {
        state.apply(&Frame::Text(TextFrame {
            content: (*text).to_owned(),
            stream: String::new(),
            style: Default::default(),
            link: link.clone(),
            inner_link: None,
            ends_line: at + 1 == parts.len(),
        }));
    }
    state.apply(&Frame::Prompt {
        time: "1001".into(),
        text: ">".into(),
    });
}

/// The herbalist's `order` menu, as its `<d cmd='order N'>` links, all in
/// one answer as the game sends it.
fn menu(state: &mut GameState, items: &[(u32, &str)]) {
    let parts: Vec<(&str, Option<Link>)> = items
        .iter()
        .flat_map(|(number, name)| {
            let link = Link {
                kind: LinkKind::Direct {
                    cmd: format!("order {number}"),
                },
                text: (*name).to_owned(),
                coord: None,
            };
            [(*name, Some(link)), ("  ", None)]
        })
        .collect();
    say(state, &parts);
}

/// A herb pouch (700) holding these, measured at these doses.
fn setup(herbs: &[(&str, &str, &str, Option<u32>)]) -> GameState {
    let mut state = GameState::default();
    state.apply(&Frame::Container {
        id: "700".to_owned(),
        title: Some("Herb Pouch".to_owned()),
        target: None,
    });
    for (id, noun, text, doses) in herbs {
        inside(&mut state, id, noun, text);
        if let Some(doses) = doses {
            say(
                &mut state,
                &[
                    ("The ", None),
                    (text, Some(exist(id, noun, text))),
                    (&format!(" has {doses} bites left."), None),
                ],
            );
        }
    }
    hand(&mut state, true, None);
    hand(&mut state, false, None);
    state
}

fn profile(stock: Option<u32>) -> HealProfile {
    HealProfile {
        container: "herb pouch".to_owned(),
        stock,
        ..HealProfile::default()
    }
}

fn stocker(state: &GameState, stock: Option<u32>, fill: bool) -> Stocker {
    Stocker::new(
        profile(stock),
        state,
        "700".to_owned(),
        (Some(SHOP), Some(BANK), HOME),
        LANDING.to_owned(),
        fill,
    )
}

#[test]
fn what_is_short_is_eherbs_minimum_less_what_is_held_in_store_doses() {
    // Acantha: 50 doses wanted at 100%, 20 held, 10 a leaf: three leaves.
    let state = setup(&[("81", "leaf", "some acantha leaf", Some(20))]);
    let wants = stocker(&state, None, false).wants(&state);
    let blood = wants
        .iter()
        .find(|w| w.bucket == Bucket::Kind(HerbKind::Blood))
        .expect("blood is short");
    assert_eq!(blood.herb.name, "some acantha leaf");
    assert_eq!(blood.count, 3);
    // At 50%: 25 wanted, 20 held, a leaf is 10: none (eherbs rounds down).
    let wants = stocker(&state, Some(50), false).wants(&state);
    assert!(
        !wants
            .iter()
            .any(|w| w.bucket == Bucket::Kind(HerbKind::Blood))
    );
}

#[test]
fn fill_buys_one_of_each_kind_the_pouch_has_none_of() {
    let state = setup(&[("81", "leaf", "some acantha leaf", Some(1))]);
    let wants = stocker(&state, None, true).wants(&state);
    assert!(wants.iter().all(|w| w.count == 1));
    assert!(
        !wants
            .iter()
            .any(|w| w.bucket == Bucket::Kind(HerbKind::Blood))
    );
    assert_eq!(wants.len(), 18, "every wound, scar, limb and eye");
}

#[test]
fn an_unmeasured_herb_is_measured_before_anything_is_bought() {
    let mut state = setup(&[("81", "leaf", "some acantha leaf", None)]);
    let mut round = stocker(&state, None, false);
    assert_eq!(round.next(&state), Step::Fetch("81".to_owned()));
    hand(&mut state, true, Some(("81", "leaf", "some acantha leaf")));
    assert_eq!(round.next(&state), Step::Measure("81".to_owned()));
    assert_eq!(
        round.next(&state),
        Step::Stow {
            item: "81".to_owned(),
            bag: "700".to_owned()
        }
    );
    hand(&mut state, true, None);
    assert_eq!(round.next(&state), Step::Walk(SHOP));
}

/// The whole Landing herbalist's menu, numbered in the table's order.
fn landing_menu(state: &mut GameState) {
    let names: Vec<(u32, &str)> = cena_session::herbs::sold_in(LANDING)
        .enumerate()
        .map(|(n, h)| (u32::try_from(n).unwrap_or(0) + 1, h.name))
        .collect();
    menu(state, &names);
}

/// The first want's menu number and batch.
fn first_order(state: &GameState, round: &Stocker) -> Option<Step> {
    let want = round.wants(state).into_iter().next()?;
    let number = state.order_menu.number(want.herb.name)?;
    Some(Step::Order {
        count: want.count.min(10),
        number,
    })
}

#[test]
fn the_menu_is_read_then_ordered_by_number_and_a_package_unpacked() {
    let mut state = setup(&[]);
    let mut round = stocker(&state, Some(20), false);
    assert_eq!(round.next(&state), Step::Walk(SHOP));
    assert_eq!(round.next(&state), Step::Menu);
    landing_menu(&mut state);
    let order = first_order(&state, &round).expect("something is short");
    assert_eq!(round.next(&state), order);
    round.outcome(&[Reply::Price(100)], &state);
    assert_eq!(round.next(&state), Step::Buy);
    hand(&mut state, true, Some(("90", "package", "small package")));
    round.outcome(&[Reply::Sold], &state);
    assert_eq!(round.next(&state), Step::Open("90".to_owned()));
    assert_eq!(
        round.next(&state),
        Step::Empty {
            from: "90".to_owned(),
            into: "700".to_owned()
        }
    );
    assert_eq!(round.next(&state), Step::Throw("90".to_owned()));
    hand(&mut state, true, None);
    assert!(
        matches!(round.next(&state), Step::Order { .. }),
        "then the next kind"
    );
}

#[test]
fn too_little_silver_goes_to_the_bank_for_the_price_and_back() {
    let mut state = setup(&[]);
    landing_menu(&mut state);
    let mut round = stocker(&state, Some(20), false);
    let order = first_order(&state, &round).expect("something is short");
    let Step::Order { count, .. } = order else {
        panic!("an order");
    };
    assert_eq!(round.next(&state), Step::Walk(SHOP));
    assert_eq!(round.next(&state), order);
    round.outcome(&[Reply::Price(200)], &state);
    assert_eq!(round.next(&state), Step::Buy);
    round.outcome(&[Reply::NotEnough], &state);
    assert_eq!(round.next(&state), Step::Walk(BANK));
    let batch = u64::from(count);
    assert_eq!(
        round.next(&state),
        Step::Withdraw(200 * batch + 200 * batch / 10),
        "the price and a tenth again"
    );
    assert_eq!(round.next(&state), Step::Walk(SHOP));
    assert_eq!(round.next(&state), order);
}
