//! The selling round's planner (`plan/31` Stage 4a): the shops from the
//! bags, the gem sack sold whole, the pawnshop item by item, and what a
//! reply decides.

use cena_behavior::town::{Reply, Seller, Shop, Step, Town};
use cena_map::RoomId;
use cena_session::containers::{ContainerEvent, ItemRef, StowSlot};
use cena_session::{Appraiser, Buyer, Frame, GameState, Link, LinkKind, LootFact, Run, Runs};

fn link(id: &str, noun: &str, text: &str) -> Link {
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
fn inside(state: &mut GameState, container: &str, id: &str, noun: &str, text: &str) {
    state.apply(&Frame::ContainerItem {
        container_id: container.to_owned(),
        content: Runs {
            runs: vec![Run {
                text: text.to_owned(),
                style: Default::default(),
                link: Some(link(id, noun, text)),
                inner_link: None,
            }],
        },
    });
}

fn hand(state: &mut GameState, right: bool, item: Option<(&str, &str, &str)>) {
    let (text, link) = match item {
        Some((id, noun, text)) => (text.to_owned(), Some(link(id, noun, text))),
        None => ("Empty".to_owned(), None),
    };
    state.apply(&if right {
        Frame::RightHand { item: text, link }
    } else {
        Frame::LeftHand { item: text, link }
    });
}

/// A gem sack (901) and a backpack (902) on the stow list, both declared
/// containers, with these items inside each; the hands empty.
fn setup(gems: &[(&str, &str, &str)], pack: &[(&str, &str, &str)]) -> GameState {
    let mut state = GameState::default();
    state.apply(&Frame::Prompt {
        time: "1000".into(),
        text: ">".into(),
    });
    for (id, title) in [("901", "My Sack"), ("902", "My Backpack")] {
        state.apply(&Frame::Container {
            id: id.to_owned(),
            title: Some(title.to_owned()),
            target: None,
        });
    }
    for (id, noun, text) in gems {
        inside(&mut state, "901", id, noun, text);
    }
    for (id, noun, text) in pack {
        inside(&mut state, "902", id, noun, text);
    }
    state.containers.apply(&ContainerEvent::StowListBegins);
    state.containers.apply(&ContainerEvent::StowSet {
        slot: StowSlot::Gem,
        item: ItemRef {
            id: "901".to_owned(),
            noun: "sack".to_owned(),
            text: "sack".to_owned(),
        },
    });
    state.containers.apply(&ContainerEvent::StowSet {
        slot: StowSlot::Default,
        item: ItemRef {
            id: "902".to_owned(),
            noun: "backpack".to_owned(),
            text: "backpack".to_owned(),
        },
    });
    hand(&mut state, true, None);
    hand(&mut state, false, None);
    state
}

fn town() -> Town {
    Town {
        sell_types: ["gem", "weapon", "clothing", "jewelry"]
            .into_iter()
            .map(str::to_owned)
            .collect(),
        containers: ["default", "gem"].into_iter().map(str::to_owned).collect(),
        appraise_types: vec!["jewelry".to_owned()],
        appraise_gemshop: 14_999,
        appraise_pawnshop: 34_999,
        keep_transmogs: true,
        ..Town::default()
    }
}

const HOME: RoomId = RoomId(20);
const GEMSHOP: RoomId = RoomId(31);
const PAWNSHOP: RoomId = RoomId(32);

fn nearest(tag: &str) -> Option<RoomId> {
    match tag {
        "gemshop" => Some(GEMSHOP),
        "pawnshop" => Some(PAWNSHOP),
        _ => None,
    }
}

fn sold(silvers: u64) -> LootFact {
    LootFact::Sold {
        item: None,
        silvers,
        to: Buyer::Pawn,
        note: None,
    }
}

fn appraised(value: u64) -> LootFact {
    LootFact::Appraised {
        item: None,
        value: Some(value),
        by: Appraiser::Shop,
    }
}

#[test]
fn nothing_sellable_means_no_round() {
    let state = setup(&[], &[("5", "rock", "grey rock")]);
    assert!(Seller::new(town(), &state, HOME).is_none());
    let state = setup(&[("1", "pearl", "black pearl")], &[]);
    assert!(
        Seller::new(Town::default(), &state, HOME).is_none(),
        "a profile that sells nothing"
    );
}

#[test]
fn a_sack_of_gems_sells_whole_then_the_note_is_read_and_stowed() {
    let mut state = setup(
        &[("1", "pearl", "black pearl"), ("2", "pearl", "white pearl")],
        &[],
    );
    let mut seller = Seller::new(town(), &state, HOME).expect("a round");
    assert_eq!(seller.shops(), [Shop::Gemshop]);
    assert_eq!(seller.next(&state, &nearest), Step::Walk(GEMSHOP));
    // Arrived: the sack comes to hand and sells whole.
    assert_eq!(seller.next(&state, &nearest), Step::Fetch("901".to_owned()));
    hand(&mut state, true, Some(("901", "sack", "sack")));
    seller.outcome(&[], &[], &state);
    assert_eq!(
        seller.next(&state, &nearest),
        Step::SellSack("901".to_owned())
    );
    seller.outcome(&[sold(8_383)], &[Reply::SackInspected], &state);
    assert_eq!(seller.next(&state, &nearest), Step::Wear("901".to_owned()));
    // Worn again, and the jeweler's note is in the left hand.
    hand(&mut state, true, None);
    hand(&mut state, false, Some(("7", "note", "promissory note")));
    seller.outcome(&[], &[], &state);
    assert_eq!(
        seller.next(&state, &nearest),
        Step::ReadNote("7".to_owned())
    );
    seller.outcome(&[], &[Reply::NoteValue(8_383)], &state);
    assert_eq!(
        seller.next(&state, &nearest),
        Step::Stow {
            item: "7".to_owned(),
            bag: "902".to_owned()
        }
    );
    hand(&mut state, false, None);
    // The sack sold its gems: the inventory no longer lists them.
    state.apply(&Frame::ClearContainer {
        id: "901".to_owned(),
    });
    seller.outcome(&[], &[], &state);
    assert_eq!(
        seller.next(&state, &nearest),
        Step::Walk(HOME),
        "nothing left: home"
    );
    assert_eq!(seller.next(&state, &nearest), Step::Done);
}

#[test]
fn the_pawnshop_appraises_a_weapon_and_sells_it_under_the_limit() {
    let mut state = setup(&[], &[("5", "poignard", "steel poignard")]);
    let mut seller = Seller::new(town(), &state, HOME).expect("a round");
    assert_eq!(seller.shops(), [Shop::Pawnshop]);
    assert_eq!(seller.next(&state, &nearest), Step::Walk(PAWNSHOP));
    assert_eq!(seller.next(&state, &nearest), Step::Fetch("5".to_owned()));
    hand(&mut state, true, Some(("5", "poignard", "steel poignard")));
    seller.outcome(&[], &[], &state);
    assert_eq!(
        seller.next(&state, &nearest),
        Step::Appraise("5".to_owned()),
        "a weapon is always appraised first"
    );
    seller.outcome(&[appraised(1_200)], &[], &state);
    assert_eq!(seller.next(&state, &nearest), Step::Sell("5".to_owned()));
    seller.outcome(&[sold(1_200)], &[], &state);
    hand(&mut state, true, None);
    state.apply(&Frame::ClearContainer {
        id: "902".to_owned(),
    });
    seller.outcome(&[], &[], &state);
    assert_eq!(seller.next(&state, &nearest), Step::Walk(HOME));
}

#[test]
#[expect(
    clippy::default_trait_access,
    reason = "the run's style type is not re-exported for behaviors; only the link matters"
)]
fn over_the_limit_goes_back_to_its_bag_and_a_transmog_is_kept() {
    let mut state = setup(&[], &[("5", "poignard", "steel poignard")]);
    let mut seller = Seller::new(town(), &state, HOME).expect("a round");
    seller.next(&state, &nearest);
    seller.next(&state, &nearest);
    hand(&mut state, true, Some(("5", "poignard", "steel poignard")));
    seller.outcome(&[], &[], &state);
    assert_eq!(
        seller.next(&state, &nearest),
        Step::Appraise("5".to_owned())
    );
    seller.outcome(&[appraised(50_000)], &[], &state);
    assert_eq!(
        seller.next(&state, &nearest),
        Step::Stow {
            item: "5".to_owned(),
            bag: "902".to_owned()
        },
        "over 34,999: kept, back in the backpack"
    );

    // A weapon with an `after` description is analyzed first when the
    // profile keeps transmogs, and kept when it is one.
    let mut state = setup(&[], &[]);
    state.apply(&Frame::ContainerItem {
        container_id: "902".to_owned(),
        content: Runs {
            runs: vec![
                Run {
                    text: "steel poignard".to_owned(),
                    style: Default::default(),
                    link: Some(link("6", "poignard", "steel poignard")),
                    inner_link: None,
                },
                Run {
                    text: " with a gleaming edge".to_owned(),
                    style: Default::default(),
                    link: None,
                    inner_link: None,
                },
            ],
        },
    });
    let mut seller = Seller::new(town(), &state, HOME).expect("a round");
    seller.next(&state, &nearest);
    assert_eq!(seller.next(&state, &nearest), Step::Fetch("6".to_owned()));
    hand(&mut state, true, Some(("6", "poignard", "steel poignard")));
    seller.outcome(&[], &[], &state);
    assert_eq!(seller.next(&state, &nearest), Step::Analyze("6".to_owned()));
    seller.outcome(&[], &[Reply::Transmog], &state);
    assert_eq!(
        seller.next(&state, &nearest),
        Step::Stow {
            item: "6".to_owned(),
            bag: "902".to_owned()
        }
    );
}

#[test]
fn a_shop_that_will_not_buy_it_sends_it_back_to_the_bag() {
    let mut state = setup(&[], &[("8", "tunic", "linen tunic")]);
    let mut seller = Seller::new(town(), &state, HOME).expect("a round");
    seller.next(&state, &nearest);
    assert_eq!(seller.next(&state, &nearest), Step::Fetch("8".to_owned()));
    hand(&mut state, true, Some(("8", "tunic", "linen tunic")));
    seller.outcome(&[], &[], &state);
    assert_eq!(seller.next(&state, &nearest), Step::Sell("8".to_owned()));
    seller.outcome(&[LootFact::Worthless { item: None }], &[], &state);
    assert_eq!(
        seller.next(&state, &nearest),
        Step::Stow {
            item: "8".to_owned(),
            bag: "902".to_owned()
        }
    );
}

#[test]
fn a_shop_the_map_lacks_is_skipped_and_gems_come_before_the_pawnshop() {
    let state = setup(
        &[("1", "pearl", "black pearl")],
        &[("5", "poignard", "steel poignard")],
    );
    let mut seller = Seller::new(town(), &state, HOME).expect("a round");
    assert_eq!(seller.shops(), [Shop::Gemshop, Shop::Pawnshop]);
    let no_gemshop = |tag: &str| (tag == "pawnshop").then_some(PAWNSHOP);
    assert_eq!(
        seller.next(&state, &no_gemshop),
        Step::Walk(PAWNSHOP),
        "no gem shop here: straight to the pawnshop"
    );
}
