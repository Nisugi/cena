//! The selling round's other shops (`plan/31` Stage 4b): the Chronomage,
//! the collectibles counter, the furrier's bundles and the bank.

mod town_support;

use town_support::*;

#[test]
fn a_gold_ring_goes_to_the_chronomage_and_a_collectible_to_its_counter() {
    let mut state = setup(
        &[],
        &[
            ("3", "ring", "braided gold ring"),
            ("4", "glass", "piece of cloudy glass"),
        ],
    );
    with_clerk(&mut state);
    let mut seller = Seller::new(town_4b(), &state, HOME).expect("a round");
    assert_eq!(seller.shops(), [Shop::Chronomage, Shop::Collectibles]);
    assert_eq!(seller.next(&state, &every), Step::Walk(CHRONOMAGE));
    assert_eq!(seller.next(&state, &every), Step::Fetch("3".to_owned()));
    hand(&mut state, true, Some(("3", "ring", "braided gold ring")));
    seller.outcome(&[], &[], &state);
    assert_eq!(
        seller.next(&state, &every),
        Step::Give {
            item: "3".to_owned(),
            to: "77".to_owned()
        }
    );
    hand(&mut state, true, None);
    seller.outcome(&[], &[], &state);
    assert_eq!(
        seller.next(&state, &every),
        Step::Walk(COUNTER),
        "the second tag the counter goes by"
    );
    assert_eq!(seller.next(&state, &every), Step::Fetch("4".to_owned()));
    hand(
        &mut state,
        true,
        Some(("4", "glass", "piece of cloudy glass")),
    );
    seller.outcome(&[], &[], &state);
    assert_eq!(seller.next(&state, &every), Step::Deposit("4".to_owned()));
    // Still in hand: the counter would not take it; back to its bag.
    seller.outcome(&[], &[], &state);
    assert_eq!(
        seller.next(&state, &every),
        Step::Stow {
            item: "4".to_owned(),
            bag: "902".to_owned()
        }
    );
    hand(&mut state, true, None);
    seller.outcome(&[], &[], &state);
    assert_eq!(
        seller.next(&state, &every),
        Step::Walk(HOME),
        "nothing sold: no bank"
    );
}

#[test]
fn a_bundle_comes_apart_a_skin_at_a_time_then_the_bank_takes_the_silver() {
    let mut state = setup(&[], &[("6", "claws", "bundle of bear claws")]);
    let town = Town {
        keep_silver: 500,
        ..town_4b()
    };
    let mut seller = Seller::new(town, &state, HOME).expect("a round");
    assert_eq!(seller.shops(), [Shop::Furrier]);
    assert_eq!(seller.next(&state, &every), Step::Walk(FURRIER));
    // A bag of skins, none excluded, sells whole first.
    assert_eq!(seller.next(&state, &every), Step::Fetch("902".to_owned()));
    hand(&mut state, true, Some(("902", "backpack", "backpack")));
    seller.outcome(&[], &[], &state);
    assert_eq!(
        seller.next(&state, &every),
        Step::SellSack("902".to_owned())
    );
    // The furrier would not take the bag whole: back on, and item by item.
    seller.outcome(&[LootFact::Worthless { item: None }], &[], &state);
    assert_eq!(seller.next(&state, &every), Step::Wear("902".to_owned()));
    hand(&mut state, true, None);
    seller.outcome(&[], &[], &state);
    assert_eq!(seller.next(&state, &every), Step::Fetch("6".to_owned()));
    hand(
        &mut state,
        true,
        Some(("6", "claws", "bundle of bear claws")),
    );
    seller.outcome(&[], &[], &state);
    assert_eq!(seller.next(&state, &every), Step::Unbundle);
    hand(&mut state, false, Some(("8", "claw", "bear claw")));
    seller.outcome(&[], &[], &state);
    assert_eq!(seller.next(&state, &every), Step::Sell("8".to_owned()));
    hand(&mut state, false, None);
    seller.outcome(&[sold(40)], &[], &state);
    assert_eq!(seller.next(&state, &every), Step::Unbundle);
    // The last two: one skin in each hand, the bundle gone.
    hand(&mut state, true, Some(("9", "claw", "bear claw")));
    hand(&mut state, false, Some(("10", "claw", "bear claw")));
    seller.outcome(&[], &[Reply::LastTwo], &state);
    assert_eq!(seller.next(&state, &every), Step::Sell("9".to_owned()));
    hand(&mut state, true, None);
    seller.outcome(&[sold(40)], &[], &state);
    assert_eq!(seller.next(&state, &every), Step::Sell("10".to_owned()));
    hand(&mut state, false, None);
    state.apply(&Frame::ClearContainer {
        id: "902".to_owned(),
    });
    seller.outcome(&[sold(40)], &[], &state);
    // Silver earned: the bank, the deposit, the keeper silver back out.
    assert_eq!(seller.next(&state, &every), Step::Walk(BANK));
    assert_eq!(seller.next(&state, &every), Step::DepositAll);
    seller.outcome(&[LootFact::Deposited(120)], &[], &state);
    assert_eq!(seller.next(&state, &every), Step::Withdraw(500));
    seller.outcome(&[LootFact::Withdrew(500)], &[], &state);
    assert_eq!(seller.next(&state, &every), Step::Walk(HOME));
    assert_eq!(seller.next(&state, &every), Step::Done);
}

#[test]
fn over_eighty_percent_encumbered_the_bank_comes_first() {
    let mut state = setup(&[("1", "pearl", "black pearl")], &[]);
    state.character.encumbrance_percent = Some(85);
    let mut seller = Seller::new(town_4b(), &state, HOME).expect("a round");
    assert_eq!(seller.next(&state, &every), Step::Walk(BANK));
    assert_eq!(seller.next(&state, &every), Step::DepositAll);
    seller.outcome(&[], &[], &state);
    assert_eq!(
        seller.next(&state, &every),
        Step::Walk(GEMSHOP),
        "banked once for the weight, then the shop"
    );
}

#[test]
fn a_note_in_the_default_bag_is_a_round_of_its_own() {
    let state = setup(&[], &[("7", "note", "promissory note")]);
    let mut seller = Seller::new(town_4b(), &state, HOME).expect("a round");
    assert!(seller.shops().is_empty());
    assert_eq!(seller.next(&state, &every), Step::Walk(BANK));
    assert_eq!(seller.next(&state, &every), Step::DepositAll);
}

#[test]
fn gold_rings_by_eloot_s_names() {
    use cena_behavior::town::is_gold_ring;
    assert!(is_gold_ring("gold ring"));
    assert!(is_gold_ring("dirt-caked gold ring"));
    assert!(!is_gold_ring("gold-inlaid ring"));
    assert!(!is_gold_ring("etched gold ring"));
}
