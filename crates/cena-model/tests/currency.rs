//! Silver, notes and the event currencies.
//!
//! # The balances are the author's
//!
//! `;infomon show` for Nisugi, 2026-09-20:
//!
//! ```text
//! currency.silver_total : 81
//! currency.tickets : 17
//! currency.bloodscrip : 1024
//! currency.ethereal_scrip : 7117
//! currency.soul_shards : 9963
//! currency.dust : 7
//! ```
//!
//! Six at once, which is why each currency is a named field: a character
//! accumulates balances from every event they have attended and they do not
//! convert.

use cena_model::GameState;
use cena_model::state::character::currency::Currency;
use cena_protocol::Parser;

fn state_after(lines: &[&str]) -> GameState {
    let mut parser = Parser::new();
    let mut state = GameState::default();
    for line in lines {
        for frame in parser.parse_line(line) {
            state.apply(&frame);
        }
    }
    // A chunk is read when a prompt closes it.
    for frame in parser.parse_line("<prompt time=\"1\">&gt;</prompt>") {
        state.apply(&frame);
    }
    state
}

#[test]
fn the_authors_balances_all_arrive() {
    let currency = state_after(&[
        "You are carrying a total of 81 silver.",
        "  General - 17 tickets.",
        "  Duskruin Arena - 1,024 bloodscrip.",
        "  Reim - 7,117 ethereal scrip.",
        "  Ebon Gate - 9,963 soul shards.",
        "You are carrying 7 Dust in your reserves.",
    ])
    .character
    .currency;

    assert_eq!(currency.silver_total, Some(81));
    assert_eq!(currency.tickets, Some(17));
    assert_eq!(currency.bloodscrip, Some(1_024));
    assert_eq!(currency.ethereal_scrip, Some(7_117));
    assert_eq!(currency.soul_shards, Some(9_963));
    assert_eq!(currency.dust, Some(7));
}

#[test]
fn a_currency_never_earned_is_unknown_rather_than_zero() {
    // §5.2. The author's store holds six balances and says nothing about
    // raikhen or aevit, which is different from holding zero of them: a
    // display should omit a currency the player has never earned rather than
    // showing a row of zeroes.
    let currency = state_after(&["  General - 17 tickets."]).character.currency;
    assert_eq!(currency.tickets, Some(17));
    assert_eq!(currency.raikhen, None, "absent, not zero");
    assert_eq!(currency.aevit, None);
    assert_eq!(currency.blackscrip, None);
}

mod silver {
    use super::state_after;

    #[test]
    fn the_word_forms_are_numbers() {
        // The game says "no silver" and "but one silver", not 0 and 1. Lich
        // maps both (`parser.rb:546-555`); reading them as numbers would store
        // NOTHING for a character who is broke, which reads the same as never
        // having asked.
        assert_eq!(
            state_after(&["You have no silver with you."])
                .character
                .currency
                .silver,
            Some(0)
        );
        assert_eq!(
            state_after(&["You have but one silver with you."])
                .character
                .currency
                .silver,
            Some(1)
        );
    }

    #[test]
    fn a_plain_amount_is_read() {
        assert_eq!(
            state_after(&["You have 12,345 silver with you."])
                .character
                .currency
                .silver,
            Some(12_345)
        );
    }

    #[test]
    fn on_your_person_a_container_and_the_total_are_three_facts() {
        // Three different questions, and Lich keeps three keys
        // (`parser.rb:556-566`). A consumer asking "can I afford this" wants
        // the total; one asking "what is in my purse" wants the first.
        let currency = state_after(&[
            "You have 20 silver with you.",
            "You are carrying 61 silver stored within your black leather backpack.",
            "You are carrying a total of 81 silver.",
        ])
        .character
        .currency;
        assert_eq!(currency.silver, Some(20));
        assert_eq!(currency.silver_container, Some(61));
        assert_eq!(currency.silver_total, Some(81), "the author's real total");
    }

    #[test]
    fn notes_are_not_coins() {
        assert_eq!(
            state_after(&["Total note value: 500,000"])
                .character
                .currency
                .notes,
            Some(500_000)
        );
    }
}

mod singulars {
    use super::state_after;

    #[test]
    fn one_ticket_is_not_one_tickets() {
        // The game says "1 ticket." The plural-only suffix would miss it and
        // store nothing, which reads as "never asked".
        assert_eq!(
            state_after(&["  General - 1 ticket."])
                .character
                .currency
                .tickets,
            Some(1)
        );
    }

    #[test]
    fn one_soul_shard_is_read() {
        assert_eq!(
            state_after(&["  Ebon Gate - 1 soul shard."])
                .character
                .currency
                .soul_shards,
            Some(1)
        );
    }

    #[test]
    fn one_gigas_fragment_is_read() {
        assert_eq!(
            state_after(&["You are carrying 1 gigas artifact fragment."])
                .character
                .currency
                .gigas_artifact_fragments,
            Some(1)
        );
    }
}

#[test]
fn voln_favor_can_be_negative() {
    // Lich captures `[-\d,]+` (`parser.rb:53`). An unsigned field would fail
    // to parse and store nothing -- the defect creature health, covert arts
    // and fame all shared this month.
    assert_eq!(
        state_after(&["Voln Favor: -1,500"])
            .character
            .currency
            .voln_favor,
        Some(-1_500)
    );
}

#[test]
fn redsteel_marks_arrive_in_two_shapes() {
    // One Lich pattern accepts both (`parser.rb:64`): a carried line and an
    // indented label in a report.
    assert_eq!(
        state_after(&["You are carrying 42 redsteel marks."])
            .character
            .currency
            .redsteel_marks,
        Some(42)
    );
    assert_eq!(
        state_after(&["    Redsteel Marks:           42"])
            .character
            .currency
            .redsteel_marks,
        Some(42)
    );
}

#[test]
fn an_unrelated_line_states_nothing() {
    let currency = state_after(&[
        "You are carrying a large sack.",
        "You have no idea what that means.",
    ])
    .character
    .currency;
    assert_eq!(currency, Currency::default());
}

mod persistence {
    use cena_model::state::character::snapshot::{CharacterSnapshot, Group};

    #[test]
    fn currency_is_its_own_group() {
        // Separately staleable: silver changes with every purchase while
        // skills change when you train.
        assert!(Group::ALL.contains(&Group::Currency));
        assert_eq!(Group::ALL.len(), 7, "the doc says seven");
    }

    #[test]
    fn it_round_trips_through_a_snapshot() {
        let mut character = cena_model::Character::default();
        character.currency.bloodscrip = Some(1_024);
        character.currency.silver_total = Some(81);

        let snapshot = CharacterSnapshot::of(
            "Nisugi",
            "GS",
            &character,
            std::collections::BTreeMap::new(),
        );
        let mut restored = cena_model::Character::default();
        assert!(snapshot.restore_into(&mut restored));
        assert_eq!(restored.currency.bloodscrip, Some(1_024));
        assert_eq!(restored.currency.silver_total, Some(81));
    }

    #[test]
    fn the_sync_asks_for_wealth_and_tickets() {
        assert_eq!(Group::Currency.sync_commands(), ["wealth", "tickets"]);
    }
}

#[test]
fn a_figure_that_is_not_wholly_a_number_is_not_read() {
    // Review finding: `number` kept every digit and dropped everything else,
    // so `12a3` read as 123 -- a balance the wire never stated. The shared
    // reader (`state/numbers.rs`) refuses a partial number, and `None` says
    // "not understood" where 123 would have said something false.
    let currency = state_after(&[
        "You are carrying a total of 12a3 silver.",
        "Voln Favor: -5 (9)",
    ])
    .character
    .currency;
    assert_eq!(currency.silver_total, None);
    assert_eq!(currency.voln_favor, None);
}
