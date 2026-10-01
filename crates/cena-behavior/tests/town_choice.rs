//! Selling by choice (`plan/61` step 6): `loot sell type|shop|item`,
//! eloot's `--type`, `--sellable` and `--sell` (`eloot.lic:6653-6813`). Each
//! narrows the round the profile would run, and none widens it.

mod town_support;

use cena_behavior::town::Choice;
use town_support::*;

fn owned(words: &[&str]) -> Vec<String> {
    words.iter().map(|word| (*word).to_owned()).collect()
}

/// `town()`, choosing.
fn choosing(choice: Choice) -> Town {
    Town { choice, ..town() }
}

/// A pearl in the gem sack and a poignard in the backpack: the gem shop and
/// the pawnshop, all told.
fn a_pearl_and_a_poignard() -> GameState {
    setup(
        &[("1", "pearl", "black pearl")],
        &[("5", "poignard", "steel poignard")],
    )
}

/// `type`: only the kinds named, by the object table's names (`custom_type`,
/// `eloot.lic:6744-6813`).
#[test]
fn a_kind_chosen_sells_only_that_kind() {
    let state = a_pearl_and_a_poignard();
    let all = Seller::new(town(), &state, HOME).expect("a round");
    assert_eq!(all.shops(), [Shop::Gemshop, Shop::Pawnshop]);
    let gems =
        Seller::new(choosing(Choice::Kinds(owned(&["gem"]))), &state, HOME).expect("a round");
    assert_eq!(gems.shops(), [Shop::Gemshop]);
    let weapons =
        Seller::new(choosing(Choice::Kinds(owned(&["weapon"]))), &state, HOME).expect("a round");
    assert_eq!(weapons.shops(), [Shop::Pawnshop]);
}

/// A kind the profile does not sell is not sold for being named, and there
/// is no round for it, a note in the bag or not (`custom_type`, `:6775`).
#[test]
fn a_kind_the_profile_does_not_sell_is_no_round() {
    let state = setup(
        &[],
        &[
            ("5", "wand", "twisted wand"),
            ("7", "note", "promissory note"),
        ],
    );
    assert!(Seller::new(choosing(Choice::Kinds(owned(&["wand"]))), &state, HOME).is_none());
    assert!(
        Seller::new(town(), &state, HOME).is_some(),
        "all of it: the note's bank"
    );
}

/// `type box` takes the boxes to the pool, as eloot's `process_boxes` runs
/// for it (`:6769`); another kind does not.
#[test]
fn type_box_goes_to_the_pool_and_another_kind_does_not() {
    let mut state = setup(&[], &[("5", "coffer", "iron coffer")]);
    with_worker(&mut state);
    let boxes = Town {
        choice: Choice::Kinds(owned(&["box"])),
        ..pool_town()
    };
    let seller = Seller::new(boxes, &state, HOME).expect("a round");
    assert_eq!(seller.shops(), [Shop::Pool]);
    let gems = Town {
        choice: Choice::Kinds(owned(&["gem"])),
        ..pool_town()
    };
    assert!(Seller::new(gems, &state, HOME).is_none());
}

/// `shop`: only the shops named of the round the bags call for, and no pool
/// (`custom_sellable`, `:6700-6742`).
#[test]
fn a_shop_chosen_is_the_only_shop_and_there_is_no_pool() {
    let state = a_pearl_and_a_poignard();
    let seller =
        Seller::new(choosing(Choice::Shops(vec![Shop::Pawnshop])), &state, HOME).expect("a round");
    assert_eq!(seller.shops(), [Shop::Pawnshop]);
    let mut state = setup(&[], &[("5", "coffer", "iron coffer")]);
    with_worker(&mut state);
    let at_the_gem_shop = Town {
        choice: Choice::Shops(vec![Shop::Gemshop]),
        ..pool_town()
    };
    assert!(
        Seller::new(at_the_gem_shop, &state, HOME).is_none(),
        "the boxes are not the gem shop's"
    );
}

/// `item`: only things whose names hold one named, in any case
/// (`custom_list`, `:6653-6698`); the gem sack is not sold whole for it, its
/// ruby not being chosen, and the ruby is never a lot.
#[test]
fn an_item_chosen_sells_alone_and_its_sack_is_not_sold_whole() {
    let mut state = setup(
        &[
            ("1", "emerald", "uncut emerald"),
            ("2", "ruby", "uncut ruby"),
        ],
        &[],
    );
    let mut seller =
        Seller::new(choosing(Choice::Names(owned(&["emerald"]))), &state, HOME).expect("a round");
    assert_eq!(seller.shops(), [Shop::Gemshop]);
    assert_eq!(seller.next(&state, &nearest), Step::Walk(GEMSHOP));
    assert_eq!(
        seller.next(&state, &nearest),
        Step::Fetch("1".to_owned()),
        "the emerald alone, not the sack"
    );
    hand(&mut state, true, Some(("1", "emerald", "uncut emerald")));
    seller.outcome(&[], &[], &state);
    assert_eq!(seller.next(&state, &nearest), Step::Sell("1".to_owned()));
    hand(&mut state, true, None);
    seller.outcome(&[sold(900)], &[], &state);
    assert_eq!(
        seller.next(&state, &nearest),
        Step::Walk(HOME),
        "the ruby stays; no bank on this map, so home"
    );
}

/// A kind chosen that is everything the gem shop takes of the sack: the
/// sack sells whole, as eloot's does for `--type gem` (`gemshop`, `:6970`).
#[test]
fn type_gem_still_sells_the_gem_sack_whole() {
    let mut state = setup(
        &[
            ("1", "emerald", "uncut emerald"),
            ("2", "ruby", "uncut ruby"),
        ],
        &[],
    );
    let mut seller =
        Seller::new(choosing(Choice::Kinds(owned(&["gem"]))), &state, HOME).expect("a round");
    assert_eq!(seller.next(&state, &nearest), Step::Walk(GEMSHOP));
    assert_eq!(
        seller.next(&state, &nearest),
        Step::Fetch("901".to_owned()),
        "the sack, to sell whole"
    );
    hand(&mut state, true, Some(("901", "sack", "sack")));
    seller.outcome(&[], &[], &state);
    assert_eq!(
        seller.next(&state, &nearest),
        Step::SellSack("901".to_owned())
    );
    seller.outcome(&[sold(1_800)], &[Reply::SackInspected], &state);
    assert_eq!(seller.next(&state, &nearest), Step::Wear("901".to_owned()));
}
