//! The selling round's planner (`plan/31` Stage 4a): the shops from the
//! bags, the gem sack sold whole, the pawnshop item by item, what a reply
//! decides; and what the gem shop sends on to the pawnshop, scrolls kept,
//! and the hands as they were.

mod town_support;

use town_support::*;

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
        Step::Analyze("5".to_owned()),
        "the pawnshop analyzes everything, for ALTER 41"
    );
    seller.outcome(&[], &[], &state);
    assert_eq!(
        seller.next(&state, &nearest),
        Step::Appraise("5".to_owned()),
        "a weapon is always appraised"
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
    assert_eq!(seller.next(&state, &nearest), Step::Analyze("5".to_owned()));
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
    assert_eq!(seller.next(&state, &nearest), Step::Analyze("8".to_owned()));
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

#[test]
fn the_jewelers_not_my_field_is_sold_at_the_pawnshop_instead() {
    let mut state = setup(&[], &[("5", "ring", "etched silver ring")]);
    let mut seller = Seller::new(town(), &state, HOME).expect("a round");
    assert_eq!(seller.shops(), [Shop::Gemshop], "jewelry is the jeweler's");
    seller.next(&state, &nearest);
    assert_eq!(seller.next(&state, &nearest), Step::Fetch("5".to_owned()));
    hand(&mut state, true, Some(("5", "ring", "etched silver ring")));
    seller.outcome(&[], &[], &state);
    assert_eq!(
        seller.next(&state, &nearest),
        Step::Appraise("5".to_owned())
    );
    seller.outcome(&[appraised(200)], &[], &state);
    assert_eq!(seller.next(&state, &nearest), Step::Sell("5".to_owned()));
    seller.outcome(&[], &[Reply::WrongShop], &state);
    assert_eq!(
        seller.shops(),
        [Shop::Gemshop, Shop::Pawnshop],
        "the pawnshop joins the round"
    );
    assert_eq!(
        seller.next(&state, &nearest),
        Step::Stow {
            item: "5".to_owned(),
            bag: "902".to_owned()
        }
    );
    hand(&mut state, true, None);
    seller.outcome(&[], &[], &state);
    assert_eq!(seller.next(&state, &nearest), Step::Walk(PAWNSHOP));
    assert_eq!(seller.next(&state, &nearest), Step::Fetch("5".to_owned()));
    hand(&mut state, true, Some(("5", "ring", "etched silver ring")));
    seller.outcome(&[], &[], &state);
    assert_eq!(seller.next(&state, &nearest), Step::Analyze("5".to_owned()));
    seller.outcome(&[], &[], &state);
    assert_eq!(
        seller.next(&state, &nearest),
        Step::Appraise("5".to_owned()),
        "a real sale there, appraised against the pawnshop's limit"
    );
}

#[test]
fn too_valuable_for_the_jeweler_is_appraised_at_the_pawnshop_when_asked() {
    let mut state = setup(&[], &[("5", "ring", "etched silver ring")]);
    let asks = Town {
        pawn_recheck: true,
        ..town()
    };
    let mut seller = Seller::new(asks, &state, HOME).expect("a round");
    seller.next(&state, &nearest);
    seller.next(&state, &nearest);
    hand(&mut state, true, Some(("5", "ring", "etched silver ring")));
    seller.outcome(&[], &[], &state);
    seller.next(&state, &nearest);
    seller.outcome(&[appraised(90_000)], &[], &state);
    assert_eq!(seller.shops(), [Shop::Gemshop, Shop::Pawnshop]);
    seller.next(&state, &nearest);
    hand(&mut state, true, None);
    seller.outcome(&[], &[], &state);
    assert_eq!(seller.next(&state, &nearest), Step::Walk(PAWNSHOP));
    seller.next(&state, &nearest);
    hand(&mut state, true, Some(("5", "ring", "etched silver ring")));
    seller.outcome(&[], &[], &state);
    assert_eq!(
        seller.next(&state, &nearest),
        Step::Appraise("5".to_owned())
    );
    seller.outcome(&[appraised(90_000)], &[], &state);
    assert_eq!(
        seller.next(&state, &nearest),
        Step::Stow {
            item: "5".to_owned(),
            bag: "902".to_owned()
        },
        "appraised only, never sold"
    );
}

#[test]
fn a_scroll_holding_a_kept_spell_is_read_and_kept() {
    let mut state = setup(&[], &[("5", "scroll", "faded vellum scroll")]);
    let keeps = Town {
        sell_types: vec!["scroll".to_owned()],
        keep_scrolls: vec!["215".to_owned(), "240v".to_owned()],
        ..town()
    };
    let mut seller = Seller::new(keeps, &state, HOME).expect("a round");
    seller.next(&state, &nearest);
    seller.next(&state, &nearest);
    hand(
        &mut state,
        true,
        Some(("5", "scroll", "faded vellum scroll")),
    );
    seller.outcome(&[], &[], &state);
    assert_eq!(
        seller.next(&state, &nearest),
        Step::ReadScroll("5".to_owned())
    );
    let spell = |spell, vibrant| Reply::ScrollSpell { spell, vibrant };
    seller.outcome(&[], &[spell(240, false)], &state);
    assert_eq!(
        seller.next(&state, &nearest),
        Step::Analyze("5".to_owned()),
        "240 is kept only vibrant: this one sells"
    );

    let mut state = setup(&[], &[("6", "scroll", "faded vellum scroll")]);
    let keeps = Town {
        sell_types: vec!["scroll".to_owned()],
        keep_scrolls: vec!["215".to_owned()],
        ..town()
    };
    let mut seller = Seller::new(keeps, &state, HOME).expect("a round");
    seller.next(&state, &nearest);
    seller.next(&state, &nearest);
    hand(
        &mut state,
        true,
        Some(("6", "scroll", "faded vellum scroll")),
    );
    seller.outcome(&[], &[], &state);
    seller.next(&state, &nearest);
    seller.outcome(&[], &[spell(215, false)], &state);
    assert_eq!(
        seller.next(&state, &nearest),
        Step::Stow {
            item: "6".to_owned(),
            bag: "902".to_owned()
        }
    );
}

#[test]
fn what_the_hands_held_comes_back_after_the_round() {
    let mut state = setup(&[], &[("5", "tunic", "linen tunic")]);
    hand(&mut state, true, Some(("70", "sword", "steel broadsword")));
    hand(&mut state, false, Some(("71", "shield", "iron buckler")));
    let mut seller = Seller::new(town(), &state, HOME).expect("a round");
    assert_eq!(seller.next(&state, &nearest), Step::Walk(PAWNSHOP));
    assert_eq!(
        seller.next(&state, &nearest),
        Step::Stow {
            item: "70".to_owned(),
            bag: "902".to_owned()
        },
        "both hands full: the right one is emptied to fetch"
    );
    hand(&mut state, true, None);
    seller.outcome(&[], &[], &state);
    seller.next(&state, &nearest);
    hand(&mut state, true, Some(("5", "tunic", "linen tunic")));
    seller.outcome(&[], &[], &state);
    seller.next(&state, &nearest);
    seller.outcome(&[], &[], &state);
    assert_eq!(seller.next(&state, &nearest), Step::Sell("5".to_owned()));
    hand(&mut state, true, None);
    state.apply(&Frame::ClearContainer {
        id: "902".to_owned(),
    });
    seller.outcome(&[sold(10)], &[], &state);
    assert_eq!(
        seller.next(&state, &nearest),
        Step::Fetch("70".to_owned()),
        "the sword back before home"
    );
    hand(&mut state, true, Some(("70", "sword", "steel broadsword")));
    seller.outcome(&[], &[], &state);
    assert_eq!(seller.next(&state, &nearest), Step::Walk(HOME));
}

#[test]
fn sell_again_to_be_sure_sends_the_sale_again() {
    let mut state = setup(&[], &[("8", "tunic", "linen tunic")]);
    let mut seller = Seller::new(town(), &state, HOME).expect("a round");
    seller.next(&state, &nearest);
    seller.next(&state, &nearest);
    hand(&mut state, true, Some(("8", "tunic", "linen tunic")));
    seller.outcome(&[], &[], &state);
    seller.next(&state, &nearest);
    seller.outcome(&[], &[], &state);
    assert_eq!(seller.next(&state, &nearest), Step::Sell("8".to_owned()));
    seller.outcome(&[], &[Reply::Again], &state);
    assert_eq!(
        seller.next(&state, &nearest),
        Step::Sell("8".to_owned()),
        "the pawnshop asked for it again"
    );
    assert_eq!(
        cena_behavior::town::classify("Not my line, really."),
        Some(Reply::WrongShop)
    );
}
