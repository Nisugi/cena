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

/// With boxes phased, each is looked at before it is given: a phased one,
/// *shifting*, is dropped, comes back to the hand whole, perhaps by another
/// id, and is given by that one (`box_unphase`, `eloot.lic:2986-2996`).
#[test]
fn a_phased_box_is_unphased_before_the_worker_takes_it() {
    let mut state = setup(&[], &[("5", "coffer", "iron coffer")]);
    with_worker(&mut state);
    let town = Town {
        phase_boxes: true,
        ..pool_town()
    };
    let mut seller = Seller::new(town, &state, HOME).expect("a round");
    seller.next(&state, &at_pool);
    assert_eq!(seller.next(&state, &at_pool), Step::Fetch("5".to_owned()));
    hand(&mut state, true, Some(("5", "coffer", "iron coffer")));
    seller.outcome(&[], &[], &state);
    assert_eq!(seller.next(&state, &at_pool), Step::LookAt("5".to_owned()));
    seller.outcome(&[], &[Reply::Shifting], &state);
    assert_eq!(seller.next(&state, &at_pool), Step::Drop("5".to_owned()));
    hand(&mut state, true, Some(("7", "coffer", "iron coffer")));
    seller.outcome(&[], &[], &state);
    assert_eq!(
        seller.next(&state, &at_pool),
        Step::Tip {
            to: "88".to_owned(),
            amount: 300,
            percent: false,
            confirm: false,
        },
        "the box whole, by its new id, and looked at once"
    );
}

#[test]
fn a_whole_box_is_looked_at_once_and_given() {
    let mut state = setup(&[], &[("5", "coffer", "iron coffer")]);
    with_worker(&mut state);
    let town = Town {
        phase_boxes: true,
        ..pool_town()
    };
    let mut seller = Seller::new(town, &state, HOME).expect("a round");
    seller.next(&state, &at_pool);
    seller.next(&state, &at_pool);
    hand(&mut state, true, Some(("5", "coffer", "iron coffer")));
    seller.outcome(&[], &[], &state);
    assert_eq!(seller.next(&state, &at_pool), Step::LookAt("5".to_owned()));
    seller.outcome(&[], &[], &state);
    assert!(matches!(
        seller.next(&state, &at_pool),
        Step::Tip { confirm: false, .. }
    ));
    assert_eq!(
        cena_behavior::town::classify("You see a shifting iron coffer."),
        Some(Reply::Shifting)
    );
}

#[test]
fn no_worker_in_the_room_passes_the_pool_by() {
    let state = setup(&[], &[("5", "coffer", "iron coffer")]);
    let mut seller = Seller::new(pool_town(), &state, HOME).expect("a round");
    assert_eq!(seller.next(&state, &at_pool), Step::Walk(POOL));
    assert_eq!(seller.next(&state, &at_pool), Step::Walk(HOME));
}

fn tip(to: &str, confirm: bool) -> Step {
    Step::Tip {
        to: to.to_owned(),
        amount: 300,
        percent: false,
        confirm,
    }
}

/// The pool's worker is the one the map names for the room (`find_worker`,
/// `eloot.lic:3204`), though no word of eloot's fits it and another NPC's
/// does; named and not there, there is no worker.
#[test]
fn the_worker_is_the_one_the_map_names() {
    let mut state = setup(&[], &[("5", "coffer", "iron coffer")]);
    with_npcs(
        &mut state,
        &[
            ("70", "woman", "slender aelotoi woman"),
            ("71", "scoundrel", "grimy halfling scoundrel"),
        ],
    );
    for (named, to) in [(Some("grimy halfling scoundrel"), "71"), (None, "70")] {
        let mut state = state.clone();
        let mut seller = Seller::new(pool_town(), &state, HOME).expect("a round");
        seller.worker_here(named);
        seller.next(&state, &at_pool);
        assert_eq!(seller.next(&state, &at_pool), Step::Fetch("5".to_owned()));
        hand(&mut state, true, Some(("5", "coffer", "iron coffer")));
        seller.outcome(&[], &[], &state);
        assert_eq!(seller.next(&state, &at_pool), tip(to, false), "{named:?}");
    }
    let mut seller = Seller::new(pool_town(), &state, HOME).expect("a round");
    seller.worker_here(Some("scruffy human worker"));
    assert_eq!(seller.next(&state, &at_pool), Step::Walk(POOL));
    assert_eq!(seller.next(&state, &at_pool), Step::Walk(HOME));
}

/// A full pool sends the box back to its bag and collects what is ready;
/// a box that came back made room, and the box is given again
/// (`handle_full_pool`, `eloot.lic:7420-7427`).
#[test]
fn a_full_pool_takes_the_box_again_once_a_return_made_room() {
    let mut state = setup(&[], &[("5", "coffer", "iron coffer")]);
    with_worker(&mut state);
    let mut seller = Seller::new(pool_town(), &state, HOME).expect("a round");
    seller.next(&state, &at_pool);
    seller.next(&state, &at_pool);
    hand(&mut state, true, Some(("5", "coffer", "iron coffer")));
    seller.outcome(&[], &[], &state);
    assert_eq!(seller.next(&state, &at_pool), tip("88", false));
    seller.outcome(&[], &[Reply::PoolFull], &state);
    seller.next(&state, &at_pool);
    hand(&mut state, true, None);
    seller.outcome(&[], &[], &state);
    assert_eq!(
        seller.next(&state, &at_pool),
        Step::AskReturn("88".to_owned())
    );
    hand(&mut state, true, Some(("6", "chest", "iron chest")));
    seller.outcome(&[returned("6", "chest", "iron chest")], &[], &state);
    assert_eq!(
        seller.next(&state, &at_pool),
        Step::EmptyBox("6".to_owned())
    );
    seller.outcome(&[], &[], &state);
    assert_eq!(seller.next(&state, &at_pool), Step::Trash("6".to_owned()));
    hand(&mut state, true, None);
    seller.outcome(&[], &[], &state);
    assert_eq!(
        seller.next(&state, &at_pool),
        Step::AskReturn("88".to_owned())
    );
    seller.outcome(&[], &[Reply::NoneReady], &state);
    assert_eq!(
        seller.next(&state, &at_pool),
        Step::Fetch("5".to_owned()),
        "room made: the box the full pool sent back is given again"
    );
}

/// Full, and nothing came back to make room: no more drop-offs.
#[test]
fn a_full_pool_that_gave_nothing_back_takes_nothing_more() {
    let mut state = setup(&[], &[("5", "coffer", "iron coffer")]);
    with_worker(&mut state);
    let mut seller = Seller::new(pool_town(), &state, HOME).expect("a round");
    seller.next(&state, &at_pool);
    seller.next(&state, &at_pool);
    hand(&mut state, true, Some(("5", "coffer", "iron coffer")));
    seller.outcome(&[], &[], &state);
    seller.next(&state, &at_pool);
    seller.outcome(&[], &[Reply::PoolFull], &state);
    seller.next(&state, &at_pool);
    hand(&mut state, true, None);
    seller.outcome(&[], &[], &state);
    assert_eq!(
        seller.next(&state, &at_pool),
        Step::AskReturn("88".to_owned())
    );
    seller.outcome(&[], &[Reply::NoneReady], &state);
    assert_eq!(seller.next(&state, &at_pool), Step::Walk(HOME));
}

/// *You need to lighten your load first*: the bank, then back to ask
/// again; refused again straight after, the returns are given up
/// (`pool_return`, `eloot.lic:7443-7452`).
#[test]
fn lighten_your_load_sends_the_round_to_the_bank_and_back() {
    let mut state = setup(&[], &[]);
    with_worker(&mut state);
    let town = Town {
        always_check_pool: true,
        keep_silver: 500,
        ..pool_town()
    };
    let mut seller = Seller::new(town, &state, HOME).expect("a round");
    let ask = Step::AskReturn("88".to_owned());
    assert_eq!(seller.next(&state, &pool_and_bank), Step::Walk(POOL));
    assert_eq!(seller.next(&state, &pool_and_bank), ask);
    seller.outcome(&[], &[Reply::Lighten], &state);
    assert_eq!(seller.next(&state, &pool_and_bank), Step::Walk(BANK));
    assert_eq!(seller.next(&state, &pool_and_bank), Step::DepositAll);
    seller.outcome(&[], &[], &state);
    assert_eq!(seller.next(&state, &pool_and_bank), Step::Withdraw(500));
    seller.outcome(&[], &[], &state);
    assert_eq!(seller.next(&state, &pool_and_bank), Step::Walk(POOL));
    assert_eq!(seller.next(&state, &pool_and_bank), ask, "asked again");
    seller.outcome(&[], &[Reply::Lighten], &state);
    assert_eq!(
        seller.next(&state, &pool_and_bank),
        Step::Walk(HOME),
        "refused again straight after the bank: the returns are given up"
    );
    // eloot's own phrases (`pool_return`, `eloot.lic:7438`).
    let classify = cena_behavior::town::classify;
    assert_eq!(
        classify("You need to lighten your load first"),
        Some(Reply::Lighten)
    );
    assert_eq!(
        classify("We don't have any boxes ready for you"),
        Some(Reply::NoneReady)
    );
}

/// A returned box whose coins would not all fit: the bank, then the box
/// again for the rest (`box_loot`, `eloot.lic:5109-5115`).
#[test]
fn coins_that_will_not_fit_send_the_round_to_the_bank_and_the_box_is_emptied_again() {
    let mut state = setup(&[], &[]);
    with_worker(&mut state);
    let town = Town {
        always_check_pool: true,
        ..pool_town()
    };
    let mut seller = Seller::new(town, &state, HOME).expect("a round");
    seller.next(&state, &pool_and_bank);
    seller.next(&state, &pool_and_bank);
    hand(&mut state, true, Some(("6", "chest", "iron chest")));
    seller.outcome(&[returned("6", "chest", "iron chest")], &[], &state);
    let empty = Step::EmptyBox("6".to_owned());
    assert_eq!(seller.next(&state, &pool_and_bank), empty);
    seller.outcome(&[], &[Reply::CoinsLeft], &state);
    assert_eq!(seller.next(&state, &pool_and_bank), Step::Walk(BANK));
    assert_eq!(seller.next(&state, &pool_and_bank), Step::DepositAll);
    seller.outcome(&[], &[], &state);
    assert_eq!(
        seller.next(&state, &pool_and_bank),
        Step::Walk(POOL),
        "nothing kept in hand: no withdrawal"
    );
    assert_eq!(seller.next(&state, &pool_and_bank), empty, "the rest now");
    seller.outcome(&[], &[], &state);
    assert_eq!(
        seller.next(&state, &pool_and_bank),
        Step::Trash("6".to_owned())
    );
}

/// A plinite the worker hands back is plucked, and what came of it put in
/// the default bag, as eloot frees both hands after (`box_loot`,
/// `eloot.lic:5138-5140`); a sword already in the other hand stays. Before,
/// a plinite ended the returns.
#[test]
fn a_returned_plinite_is_plucked_and_put_away() {
    let mut state = setup(&[], &[]);
    with_worker(&mut state);
    hand(&mut state, false, Some(("11", "sword", "broadsword")));
    let town = Town {
        always_check_pool: true,
        ..pool_town()
    };
    let mut seller = Seller::new(town, &state, HOME).expect("a round");
    seller.next(&state, &at_pool);
    seller.next(&state, &at_pool);
    hand(&mut state, true, Some(("9", "plinite", "glowing plinite")));
    seller.outcome(&[returned("9", "plinite", "glowing plinite")], &[], &state);
    assert_eq!(seller.next(&state, &at_pool), Step::Pluck("9".to_owned()));
    seller.outcome(&[], &[], &state);
    assert_eq!(
        seller.next(&state, &at_pool),
        Step::Stow {
            item: "9".to_owned(),
            bag: "902".to_owned()
        }
    );
    hand(&mut state, true, None);
    seller.outcome(&[], &[], &state);
    assert_eq!(
        seller.next(&state, &at_pool),
        Step::AskReturn("88".to_owned()),
        "the sword stays, and the returns go on"
    );
}

/// `always_check_pool`: the pool asked for its returns every round, with no
/// box carried, or the pool not used for drop-offs at all
/// (`process_boxes`, `eloot.lic:7696`, `:7714`).
#[test]
fn always_check_pool_asks_for_returns_with_nothing_to_give() {
    let state = setup(&[], &[]);
    assert!(
        Seller::new(pool_town(), &state, HOME).is_none(),
        "no box, no switch: no round"
    );
    let mut state = setup(&[], &[("5", "coffer", "iron coffer")]);
    with_worker(&mut state);
    let town = Town {
        pool: false,
        always_check_pool: true,
        ..pool_town()
    };
    let mut seller = Seller::new(town, &state, HOME).expect("a round");
    assert_eq!(seller.shops(), [Shop::Pool]);
    assert_eq!(seller.next(&state, &at_pool), Step::Walk(POOL));
    assert_eq!(
        seller.next(&state, &at_pool),
        Step::AskReturn("88".to_owned()),
        "the coffer is not given: the profile does not use the pool for that"
    );
    seller.outcome(&[], &[Reply::NoneReady], &state);
    assert_eq!(seller.next(&state, &at_pool), Step::Walk(HOME));
}

/// `loot pool` keeps what was carried when it began, and banks the rest:
/// what the boxes held, and the tips evened out (`pool`,
/// `eloot.lic:7626-7648`). The returns alone (`loot pool return`) bank
/// nothing.
#[test]
fn loot_pool_banks_after_and_keeps_what_was_carried() {
    let mut state = setup(&[], &[("5", "coffer", "iron coffer")]);
    with_worker(&mut state);
    state.character.currency.silver = Some(1234);
    let both = cena_behavior::town::Round::Pool {
        drop: true,
        collect: true,
    };
    let mut seller = Seller::for_round(pool_town(), &state, HOME, both).expect("a round");
    seller.next(&state, &pool_and_bank);
    seller.next(&state, &pool_and_bank);
    hand(&mut state, true, Some(("5", "coffer", "iron coffer")));
    seller.outcome(&[], &[], &state);
    seller.next(&state, &pool_and_bank);
    hand(&mut state, true, None);
    let dropped = LootFact::PoolDropped {
        noun: "coffer".to_owned(),
        tip: 300,
        fee: 50,
    };
    seller.outcome(&[dropped], &[], &state);
    assert_eq!(
        seller.next(&state, &pool_and_bank),
        Step::AskReturn("88".to_owned())
    );
    seller.outcome(&[], &[Reply::NoneReady], &state);
    assert_eq!(seller.next(&state, &pool_and_bank), Step::Walk(BANK));
    assert_eq!(seller.next(&state, &pool_and_bank), Step::DepositAll);
    seller.outcome(&[], &[], &state);
    assert_eq!(seller.next(&state, &pool_and_bank), Step::Withdraw(1234));

    let mut state = setup(&[], &[]);
    with_worker(&mut state);
    let returns = cena_behavior::town::Round::Pool {
        drop: false,
        collect: true,
    };
    let mut seller = Seller::for_round(pool_town(), &state, HOME, returns).expect("a round");
    seller.next(&state, &pool_and_bank);
    seller.next(&state, &pool_and_bank);
    hand(&mut state, true, Some(("6", "chest", "iron chest")));
    seller.outcome(&[returned("6", "chest", "iron chest")], &[], &state);
    seller.next(&state, &pool_and_bank);
    let coins = LootFact::BoxOpened {
        item: cena_session::containers::ItemRef {
            id: "6".to_owned(),
            noun: "chest".to_owned(),
            text: "iron chest".to_owned(),
        },
        silvers: 900,
    };
    seller.outcome(&[coins], &[], &state);
    seller.next(&state, &pool_and_bank);
    hand(&mut state, true, None);
    seller.outcome(&[], &[], &state);
    seller.next(&state, &pool_and_bank);
    seller.outcome(&[], &[Reply::NoneReady], &state);
    assert_eq!(
        seller.next(&state, &pool_and_bank),
        Step::Walk(HOME),
        "the returns alone: no bank"
    );
}

/// The pool's step coming to nothing five times over ends the visit, as
/// any other step of the round: a box that will not go back in its bag
/// does not hold the round at the pool.
#[test]
fn a_pool_step_that_comes_to_nothing_ends_the_visit() {
    let mut state = setup(&[], &[("5", "coffer", "iron coffer")]);
    with_worker(&mut state);
    let mut seller = Seller::new(pool_town(), &state, HOME).expect("a round");
    seller.next(&state, &at_pool);
    seller.next(&state, &at_pool);
    hand(&mut state, true, Some(("5", "coffer", "iron coffer")));
    seller.outcome(&[], &[], &state);
    seller.next(&state, &at_pool);
    seller.outcome(&[], &[Reply::PoolFull], &state);
    let back = Step::Stow {
        item: "5".to_owned(),
        bag: "902".to_owned(),
    };
    for _ in 0..5 {
        assert_eq!(seller.next(&state, &at_pool), back);
        seller.outcome(&[], &[], &state);
    }
    assert_eq!(seller.next(&state, &at_pool), Step::Walk(HOME));
}

/// A returned box whose contents were never listed is kept, not trashed
/// with whatever it holds (`box_loot`, `eloot.lic:5096`; the crate review of
/// 2026-10-01, BE-E-5).
#[test]
fn a_returned_box_never_looked_into_is_kept() {
    let mut state = setup(&[], &[]);
    with_worker(&mut state);
    let town = Town {
        always_check_pool: true,
        ..pool_town()
    };
    let mut seller = Seller::new(town, &state, HOME).expect("a round");
    seller.next(&state, &at_pool);
    seller.next(&state, &at_pool);
    hand(&mut state, true, Some(("6", "chest", "iron chest")));
    seller.outcome(&[returned("6", "chest", "iron chest")], &[], &state);
    assert_eq!(
        seller.next(&state, &at_pool),
        Step::EmptyBox("6".to_owned())
    );
    seller.outcome(&[], &[Reply::BoxUnseen], &state);
    assert_eq!(
        seller.next(&state, &at_pool),
        Step::Stow {
            item: "6".to_owned(),
            bag: "902".to_owned()
        }
    );
}

/// `loot pool` sells nothing, so a returned box no bag would empty stays in
/// hand at once, and the round says so (`stow_box_item`, `eloot.lic:5363`).
#[test]
fn loot_pool_keeps_a_box_no_bag_would_empty_in_hand() {
    let mut state = setup(&[], &[]);
    with_worker(&mut state);
    let returns = cena_behavior::town::Round::Pool {
        drop: false,
        collect: true,
    };
    let mut seller = Seller::for_round(pool_town(), &state, HOME, returns).expect("a round");
    seller.next(&state, &at_pool);
    seller.next(&state, &at_pool);
    hand(&mut state, true, Some(("6", "chest", "iron chest")));
    seller.outcome(&[returned("6", "chest", "iron chest")], &[], &state);
    seller.next(&state, &at_pool);
    seller.outcome(&[], &[Reply::ThingsLeft], &state);
    assert_eq!(seller.next(&state, &at_pool), Step::Walk(HOME));
    assert_eq!(seller.stuck(), Some("6"));
}

/// A reliquary back from the pool is kept, whatever the profile sells: eloot
/// never throws one out (`save_trash_box`, `eloot.lic:7781`).
#[test]
fn a_returned_reliquary_is_kept_whatever_the_profile_sells() {
    let mut state = setup(&[], &[]);
    with_worker(&mut state);
    let town = Town {
        always_check_pool: true,
        ..pool_town()
    };
    let mut seller = Seller::new(town, &state, HOME).expect("a round");
    seller.next(&state, &at_pool);
    seller.next(&state, &at_pool);
    let reliquary = ("6", "reliquary", "filigreed rolaren reliquary");
    hand(&mut state, true, Some(reliquary));
    seller.outcome(
        &[returned(reliquary.0, reliquary.1, reliquary.2)],
        &[],
        &state,
    );
    assert_eq!(
        seller.next(&state, &at_pool),
        Step::EmptyBox("6".to_owned())
    );
    seller.outcome(&[], &[], &state);
    assert_eq!(
        seller.next(&state, &at_pool),
        Step::Stow {
            item: "6".to_owned(),
            bag: "902".to_owned()
        }
    );
}
