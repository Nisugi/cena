//! The locksmith pool (`plan/31` Stage 4c): boxes dropped off with the tip,
//! returned, emptied and trashed.

mod town_support;

use town_support::*;

#[test]
fn a_box_goes_to_the_pool_with_its_tip_then_comes_back_emptied_and_trashed() {
    let mut state = setup(&[], &[("5", "coffer", "iron coffer")]);
    with_worker(&mut state);
    let mut seller = Seller::new(pool_town(), &state, HOME).expect("a round");
    assert_eq!(seller.shops(), [Shop::Pool]);
    assert_eq!(seller.next(&state, &at_pool), Step::Walk(POOL));
    assert_eq!(seller.next(&state, &at_pool), Step::Fetch("5".to_owned()));
    hand(&mut state, true, Some(("5", "coffer", "iron coffer")));
    seller.outcome(&[], &[], &state);
    let tip = |confirm| Step::Tip {
        to: "88".to_owned(),
        amount: 300,
        percent: false,
        confirm,
    };
    assert_eq!(seller.next(&state, &at_pool), tip(false));
    let quote = LootFact::PoolQuoted {
        item: a_box(),
        tip: 300,
        fee: 50,
    };
    seller.outcome(&[quote], &[], &state);
    assert_eq!(
        seller.next(&state, &at_pool),
        tip(true),
        "the quote confirmed"
    );
    hand(&mut state, true, None);
    let dropped = LootFact::PoolDropped {
        noun: "coffer".to_owned(),
        tip: 300,
        fee: 50,
    };
    seller.outcome(&[dropped], &[], &state);
    assert_eq!(
        seller.next(&state, &at_pool),
        Step::AskReturn("88".to_owned())
    );
    // A box the pool finished earlier comes back.
    hand(&mut state, true, Some(("6", "chest", "iron chest")));
    let back = LootFact::BoxReturned {
        item: cena_session::containers::ItemRef {
            id: "6".to_owned(),
            noun: "chest".to_owned(),
            text: "iron chest".to_owned(),
        },
    };
    seller.outcome(&[back], &[], &state);
    assert_eq!(
        seller.next(&state, &at_pool),
        Step::EmptyBox("6".to_owned())
    );
    seller.outcome(&[], &[], &state);
    assert_eq!(seller.next(&state, &at_pool), Step::Trash("6".to_owned()));
    seller.outcome(&[], &[Reply::NoTrash], &state);
    assert_eq!(
        seller.next(&state, &at_pool),
        Step::Drop("6".to_owned()),
        "no receptacle here: dropped"
    );
    hand(&mut state, true, None);
    seller.outcome(&[], &[], &state);
    assert_eq!(
        seller.next(&state, &at_pool),
        Step::AskReturn("88".to_owned())
    );
    seller.outcome(&[], &[Reply::NoneReady], &state);
    assert_eq!(seller.next(&state, &at_pool), Step::Walk(HOME));
}

#[test]
fn a_full_pool_sends_the_box_back_and_a_locked_return_is_kept() {
    let mut state = setup(&[], &[("5", "coffer", "iron coffer")]);
    with_worker(&mut state);
    let mut seller = Seller::new(pool_town(), &state, HOME).expect("a round");
    seller.next(&state, &at_pool);
    seller.next(&state, &at_pool);
    hand(&mut state, true, Some(("5", "coffer", "iron coffer")));
    seller.outcome(&[], &[], &state);
    seller.next(&state, &at_pool);
    seller.outcome(&[], &[Reply::PoolFull], &state);
    assert_eq!(
        seller.next(&state, &at_pool),
        Step::Stow {
            item: "5".to_owned(),
            bag: "902".to_owned()
        }
    );
    hand(&mut state, true, None);
    seller.outcome(&[], &[], &state);
    assert_eq!(
        seller.next(&state, &at_pool),
        Step::AskReturn("88".to_owned()),
        "full: straight to the returns"
    );
    hand(&mut state, true, Some(("6", "chest", "iron chest")));
    seller.outcome(&[], &[], &state);
    assert_eq!(
        seller.next(&state, &at_pool),
        Step::EmptyBox("6".to_owned())
    );
    seller.outcome(&[], &[Reply::BoxLocked], &state);
    assert_eq!(
        seller.next(&state, &at_pool),
        Step::Stow {
            item: "6".to_owned(),
            bag: "902".to_owned()
        },
        "a box that would not open goes back in the bag"
    );
}

#[test]
fn no_worker_in_the_room_passes_the_pool_by() {
    let state = setup(&[], &[("5", "coffer", "iron coffer")]);
    let mut seller = Seller::new(pool_town(), &state, HOME).expect("a round");
    assert_eq!(seller.next(&state, &at_pool), Step::Walk(POOL));
    assert_eq!(seller.next(&state, &at_pool), Step::Walk(HOME));
}
