//! What the pool's returns bring to the round (`plan/61` step 5b): the shops
//! for what the boxes held, a box still holding what no bag would take held
//! in hand while the round sells and come back to, a gold ingot sold from it,
//! and what the hands held when the round began never sold.

mod town_support;

use town_support::*;

/// The shops of a small town and its pool.
fn rooms(tag: &str) -> Option<RoomId> {
    match tag {
        "locksmith pool" => Some(POOL),
        "gemshop" => Some(GEMSHOP),
        "pawnshop" => Some(PAWNSHOP),
        "bank" => Some(BANK),
        _ => None,
    }
}

/// A profile that pools its boxes and sells gems.
fn gems_and_pool() -> Town {
    Town {
        sell_types: vec!["gem".to_owned()],
        containers: ["default", "gem"].into_iter().map(str::to_owned).collect(),
        pool: true,
        always_check_pool: true,
        pool_tip: 300,
        ..Town::default()
    }
}

/// The box `id` as the game lists it after `look in`.
fn listed(state: &mut GameState, id: &str, things: &[(&str, &str, &str)]) {
    state.apply(&Frame::Container {
        id: id.to_owned(),
        title: Some("Chest".to_owned()),
        target: None,
        attrs: Vec::new(),
    });
    state.apply(&Frame::ClearContainer { id: id.to_owned() });
    for (thing, noun, text) in things {
        inside(state, id, thing, noun, text);
    }
}

/// At the pool, asked for its returns: the chest `6` handed back holding
/// `things`, and emptied as far as the bags allowed.
fn a_chest_back(
    seller: &mut Seller,
    state: &mut GameState,
    things: &[(&str, &str, &str)],
    replies: &[Reply],
) {
    assert_eq!(seller.next(state, &rooms), Step::Walk(POOL));
    assert_eq!(seller.next(state, &rooms), Step::AskReturn("88".to_owned()));
    hand(state, true, Some(("6", "chest", "iron chest")));
    listed(state, "6", things);
    seller.outcome(&[returned("6", "chest", "iron chest")], &[], state);
    assert_eq!(seller.next(state, &rooms), Step::EmptyBox("6".to_owned()));
    seller.outcome(&[], replies, state);
}

/// eloot reads the bags for its shops after the boxes (`Sell.sell`,
/// `eloot.lic:7820-7835`): an emerald a returned box held is sold this round,
/// though no gem was carried when it began.
#[test]
fn the_shops_follow_what_the_returns_brought() {
    let mut state = setup(&[], &[]);
    with_worker(&mut state);
    let mut seller = Seller::new(gems_and_pool(), &state, HOME).expect("a round");
    assert_eq!(seller.shops(), [Shop::Pool], "nothing to sell yet");
    a_chest_back(&mut seller, &mut state, &[], &[]);
    // The emerald went into the gem sack as the box was emptied.
    inside(&mut state, "901", "42", "emerald", "uncut emerald");
    assert_eq!(seller.next(&state, &rooms), Step::Trash("6".to_owned()));
    hand(&mut state, true, None);
    seller.outcome(&[], &[], &state);
    assert_eq!(
        seller.next(&state, &rooms),
        Step::AskReturn("88".to_owned())
    );
    seller.outcome(&[], &[Reply::NoneReady], &state);
    assert_eq!(seller.next(&state, &rooms), Step::Walk(GEMSHOP));
}

/// A returned box still holding what no bag would take: the visit stops,
/// the round sells what it can of the box itself, comes back, empties it
/// with the room the sale made and goes on with the returns
/// (`pool_direct_sell_recovery`, `eloot.lic:5455-5491`). A stow to free a
/// hand on the way puts away the other hand's thing, never the box.
#[test]
fn a_box_no_bag_would_empty_is_held_while_the_round_sells_and_come_back_to() {
    let mut state = setup(&[], &[]);
    with_worker(&mut state);
    hand(&mut state, false, Some(("12", "shield", "small shield")));
    let mut seller = Seller::new(gems_and_pool(), &state, HOME).expect("a round");
    let emerald = ("42", "emerald", "uncut emerald");
    a_chest_back(&mut seller, &mut state, &[emerald], &[Reply::ThingsLeft]);
    assert_eq!(seller.next(&state, &rooms), Step::Walk(GEMSHOP));
    assert_eq!(
        seller.next(&state, &rooms),
        Step::Stow {
            item: "12".to_owned(),
            bag: "902".to_owned()
        },
        "the shield goes, the chest stays"
    );
    hand(&mut state, false, None);
    seller.outcome(&[], &[], &state);
    assert_eq!(seller.next(&state, &rooms), Step::Fetch("42".to_owned()));
    hand(&mut state, false, Some(emerald));
    seller.outcome(&[], &[], &state);
    assert_eq!(seller.next(&state, &rooms), Step::Sell("42".to_owned()));
    hand(&mut state, false, None);
    listed(&mut state, "6", &[]);
    seller.outcome(&[sold(900)], &[], &state);
    assert_eq!(seller.next(&state, &rooms), Step::Walk(POOL), "back for it");
    assert_eq!(
        seller.next(&state, &rooms),
        Step::EmptyBox("6".to_owned()),
        "the chest first"
    );
    seller.outcome(&[], &[], &state);
    assert_eq!(seller.next(&state, &rooms), Step::Trash("6".to_owned()));
    hand(&mut state, true, None);
    seller.outcome(&[], &[], &state);
    assert_eq!(
        seller.next(&state, &rooms),
        Step::AskReturn("88".to_owned()),
        "and the returns go on"
    );
    assert_eq!(seller.stuck(), None);
}

/// A gold ingot no bag would take is sold from the box at the gem shop,
/// though the profile sells no valuables (`handle_ingot`,
/// `eloot.lic:7188-7203`).
#[test]
fn an_ingot_no_bag_takes_is_sold_from_the_box() {
    let mut state = setup(&[], &[]);
    with_worker(&mut state);
    let mut seller = Seller::new(gems_and_pool(), &state, HOME).expect("a round");
    a_chest_back(
        &mut seller,
        &mut state,
        &[("43", "ingot", "bright gold ingot")],
        &[Reply::ThingsLeft],
    );
    assert_eq!(seller.next(&state, &rooms), Step::Walk(GEMSHOP));
    assert_eq!(seller.next(&state, &rooms), Step::Fetch("43".to_owned()));
    hand(
        &mut state,
        false,
        Some(("43", "ingot", "bright gold ingot")),
    );
    seller.outcome(&[], &[], &state);
    assert_eq!(seller.next(&state, &rooms), Step::Sell("43".to_owned()));
}

/// Set aside once and still not emptied: the box stays in hand, and the
/// round says so, as eloot pauses for the player (`pool_sell_recovery`,
/// `eloot.lic:5516-5526`).
#[test]
fn a_box_set_aside_twice_stays_in_hand() {
    let mut state = setup(&[], &[]);
    with_worker(&mut state);
    let mut seller = Seller::new(gems_and_pool(), &state, HOME).expect("a round");
    let wand = ("44", "wand", "twisted wand");
    a_chest_back(&mut seller, &mut state, &[wand], &[Reply::ThingsLeft]);
    assert_eq!(
        seller.next(&state, &rooms),
        Step::Walk(POOL),
        "nothing in it the round sells: straight back"
    );
    assert_eq!(seller.next(&state, &rooms), Step::EmptyBox("6".to_owned()));
    seller.outcome(&[], &[Reply::ThingsLeft], &state);
    assert_eq!(seller.next(&state, &rooms), Step::Walk(HOME));
    assert_eq!(seller.stuck(), Some("6"));
}

/// What the hands held when the round began is never for sale: the sword
/// stowed to free a hand at the gem shop is fetched back at the end, not
/// sold at the pawnshop with the poignard (the crate review of 2026-10-01,
/// BE-E-6).
#[test]
fn a_weapon_stowed_to_free_a_hand_is_not_sold() {
    let mut state = setup(
        &[("1", "pearl", "black pearl")],
        &[("5", "poignard", "steel poignard")],
    );
    hand(
        &mut state,
        true,
        Some(("11", "broadsword", "steel broadsword")),
    );
    hand(&mut state, false, Some(("12", "shield", "small shield")));
    let mut seller = Seller::new(town(), &state, HOME).expect("a round");
    assert_eq!(seller.shops(), [Shop::Gemshop, Shop::Pawnshop]);
    assert_eq!(seller.next(&state, &nearest), Step::Walk(GEMSHOP));
    // The sack sells whole; first a hand is freed for it: the sword goes
    // into the backpack, a bag the round sells from.
    assert_eq!(
        seller.next(&state, &nearest),
        Step::Stow {
            item: "11".to_owned(),
            bag: "902".to_owned()
        }
    );
    hand(&mut state, true, None);
    inside(&mut state, "902", "11", "broadsword", "steel broadsword");
    seller.outcome(&[], &[], &state);
    assert_eq!(seller.next(&state, &nearest), Step::Fetch("901".to_owned()));
    hand(&mut state, true, Some(("901", "sack", "sack")));
    seller.outcome(&[], &[], &state);
    assert_eq!(
        seller.next(&state, &nearest),
        Step::SellSack("901".to_owned())
    );
    seller.outcome(&[sold(800)], &[Reply::SackInspected], &state);
    assert_eq!(seller.next(&state, &nearest), Step::Wear("901".to_owned()));
    hand(&mut state, true, None);
    state.apply(&Frame::ClearContainer {
        id: "901".to_owned(),
    });
    seller.outcome(&[], &[], &state);
    assert_eq!(seller.next(&state, &nearest), Step::Walk(PAWNSHOP));
    // The poignard sells; the sword is not a lot.
    assert_eq!(seller.next(&state, &nearest), Step::Fetch("5".to_owned()));
    hand(&mut state, true, Some(("5", "poignard", "steel poignard")));
    seller.outcome(&[], &[], &state);
    assert_eq!(seller.next(&state, &nearest), Step::Analyze("5".to_owned()));
    seller.outcome(&[], &[], &state);
    assert_eq!(
        seller.next(&state, &nearest),
        Step::Appraise("5".to_owned())
    );
    seller.outcome(&[appraised(1_200)], &[], &state);
    assert_eq!(seller.next(&state, &nearest), Step::Sell("5".to_owned()));
    hand(&mut state, true, None);
    seller.outcome(&[sold(1_200)], &[], &state);
    assert_eq!(
        seller.next(&state, &nearest),
        Step::Fetch("11".to_owned()),
        "the sword, fetched back"
    );
    hand(
        &mut state,
        true,
        Some(("11", "broadsword", "steel broadsword")),
    );
    seller.outcome(&[], &[], &state);
    assert_eq!(
        seller.next(&state, &nearest),
        Step::Walk(HOME),
        "fetched back, not appraised for sale"
    );
}
