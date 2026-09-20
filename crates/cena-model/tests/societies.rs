//! Society ability tables, checked against the wiki rather than against Lich.
//!
//! # Why the wiki is the oracle here
//!
//! `crit_parity.rs` digests a table whose only source is Lich. These tables
//! have a second source: `order_of_voln.rb:10` cites the `GemStone` wiki's Favor
//! page for its cost table, and that page -- along with the Council and Sunfist
//! pages -- is in `reference/wiki_clean/`. A cost transcribed from one source
//! and tested against the same source proves only that the transcription did
//! not change; the checks below were written after diffing the two, and they
//! assert the values the *wiki* states.
//!
//! What the diff found, before any of this was written:
//!
//! | Checked | Values | Disagreements |
//! |---|---|---|
//! | Voln favor table | 98 levels | 0 |
//! | Voln cost modifiers | 22 factors | **1** |
//! | Sunfist sigils | 20 x 3 | 0 |
//! | Council signs | 20 x 4 | 0 |
//!
//! The one disagreement is `retribution_has_both_costs` below.

use cena_model::Society;
use cena_model::state::societies::{
    Ability, AbilityKind, Cost, CostTiming, Target, col, sunfist, voln,
};

/// Every society's table is complete and in rank order.
///
/// A gap or a duplicate would make `known_at` silently wrong for one rank, and
/// nothing else here would notice.
#[test]
fn every_table_is_complete_and_ordered() {
    for society in Society::ALL {
        let abilities = society.abilities();
        assert_eq!(
            abilities.len(),
            usize::from(society.max_rank()),
            "{society} should grant one ability per rank"
        );
        for (index, ability) in abilities.iter().enumerate() {
            let expected = u8::try_from(index + 1).expect("rank fits in u8");
            assert_eq!(
                ability.rank, expected,
                "{} is out of order",
                ability.long_name
            );
            assert_eq!(
                ability.society, society,
                "{} is in the wrong table",
                ability.long_name
            );
            assert!(!ability.short_name.is_empty());
            assert!(!ability.long_name.is_empty());
        }
    }
}

/// **The favor table matches the wiki at the values the wiki measured.**
///
/// `reference/wiki_clean/Favor.txt`, `==Symbol Use Favor Cost==`. The wiki says
/// levels 3-42 and 100 are verified in game and the rest interpolated, so the
/// spot checks below are chosen from the measured band plus the endpoint.
#[test]
fn the_favor_table_matches_the_wiki() {
    let at = |level: usize| voln::BASE_FAVOR_COST_BY_LEVEL[level];
    assert_eq!(at(3), Some(13), "the lowest level the Order admits");
    assert_eq!(at(10), Some(100));
    assert_eq!(at(20), Some(286));
    assert_eq!(at(42), Some(774), "the last game-verified level");
    assert_eq!(at(100), Some(2174), "also game-verified");

    // Below level 3 there is no cost, rather than a cost of zero.
    assert_eq!(at(0), None);
    assert_eq!(at(2), None, "the Order does not admit level 2");
    assert!(
        voln::BASE_FAVOR_COST_BY_LEVEL
            .iter()
            .skip(voln::MIN_LEVEL)
            .all(Option::is_some),
        "every level from 3 to 100 has a cost"
    );
}

/// **The table is monotonic**, which no single spot check can show.
///
/// A transposed pair would keep every value present and every spot check
/// passing. Favor cost rises with level without exception, so a dip is a
/// transcription error.
#[test]
fn favor_cost_never_decreases_with_level() {
    let mut previous = 0;
    for level in voln::MIN_LEVEL..=100 {
        let cost = voln::BASE_FAVOR_COST_BY_LEVEL[level].expect("levels 3..=100 have costs");
        assert!(
            cost > previous,
            "level {level} costs {cost}, which is not more than level {} at {previous}",
            level - 1
        );
        previous = cost;
    }
}

/// The wiki's own worked example, reproduced.
///
/// `Favor.txt`: *"To calculate the Symbol of Transcendence favor cost for a
/// level 40 character, multiply the level 40 Symbol of Return cost (728 favor)
/// by the transcendence factor (0.60) ... 728 x 0.60 = 437 favor"*.
///
/// 728 * 0.60 is 436.8, so this also pins the rounding: Ruby's `.ceil`, never
/// in the member's favour.
#[test]
fn the_wikis_worked_example_comes_out_right() {
    let transcendence = voln::symbol("transcendence").expect("rank 12");
    assert_eq!(voln::favor_cost(&transcendence.cost, 40), Some(437));
}

/// Symbol of Return costs exactly the base, by definition.
#[test]
fn symbol_of_return_is_the_base_cost() {
    let ret = voln::symbol("return").expect("rank 25");
    for level in [3, 40, 100] {
        assert_eq!(
            voln::favor_cost(&ret.cost, level).map(|c| u16::try_from(c).expect("fits")),
            voln::BASE_FAVOR_COST_BY_LEVEL[level],
            "at level {level}"
        );
    }
}

/// **Symbol of Retribution has two costs, and Lich models one.**
///
/// The wiki lists `Retribution (attack version)` at 0.04 and
/// `Retribution (self-cast version)` at 0.30. `order_of_voln.rb:182` stores
/// only 0.04 and writes the other in a comment, so Lich reports a self-cast
/// Retribution as costing an eighth of its real price.
///
/// This is the one place the two sources disagree, and it is ported as a fix.
#[test]
fn retribution_has_both_costs() {
    let retribution = voln::symbol("retribution").expect("rank 15");
    assert_eq!(
        voln::favor_cost(&retribution.cost, 100),
        Some(87),
        "attack version: 2174 * 0.04"
    );
    assert_eq!(
        voln::alternate_favor_cost(&retribution.cost, 100),
        Some(653),
        "self-cast: 2174 * 0.30, which Lich does not model"
    );

    let Cost::Favor { alternate, .. } = retribution.cost else {
        panic!("a favor cost");
    };
    assert_eq!(alternate.expect("an alternate").reason, "self-cast");
}

/// Symbol of Blessing's alternate, which Lich *does* model.
///
/// Asserted beside Retribution because it is the structure Retribution needed
/// and did not get -- if this ever stops working the two should fail together.
#[test]
fn blessing_is_cheaper_on_non_magical_gear() {
    let blessing = voln::symbol("blessing").expect("rank 2");
    assert_eq!(voln::favor_cost(&blessing.cost, 100), Some(435), "0.20");
    assert_eq!(
        voln::alternate_favor_cost(&blessing.cost, 100),
        Some(87),
        "0.04 on non-magical gear"
    );
    let Cost::Favor { alternate, .. } = blessing.cost else {
        panic!("a favor cost");
    };
    assert_eq!(alternate.expect("an alternate").reason, "non-magical");
}

/// Every Voln cost modifier the wiki's factor table lists.
///
/// The whole table rather than a sample: these are the numbers a caller spends
/// favor on, and 22 assertions is cheaper than one wrong one.
#[test]
fn every_voln_cost_modifier_matches_the_wiki() {
    // (short name, wiki factor) from `Favor.txt`'s "Other Symbol Costs".
    let wiki: &[(&str, f64)] = &[
        ("courage", 0.10),
        ("protection", 0.10),
        ("blessing", 0.20),
        ("sleep", 0.20),
        ("diminishment", 0.30),
        ("submission", 0.30),
        ("holiness", 0.30),
        ("mana", 0.30),
        ("sight", 0.30),
        ("disruption", 0.30),
        ("turning", 0.30),
        ("recall", 0.40),
        ("restoration", 0.40),
        ("need", 0.40),
        ("supremacy", 0.50),
        ("renewal", 0.50),
        ("transcendence", 0.60),
        ("preservation", 0.60),
        ("dreams", 0.60),
        ("return", 1.00),
        ("retribution", 0.04),
    ];
    for (name, factor) in wiki {
        let symbol = voln::symbol(name).unwrap_or_else(|| panic!("{name} is a symbol"));
        let Cost::Favor { modifier, .. } = symbol.cost else {
            panic!("{name} should cost favor, got {:?}", symbol.cost);
        };
        assert!(
            (modifier - factor).abs() < f64::EPSILON,
            "{name}: wiki says {factor}, table says {modifier}"
        );
    }
}

/// The free symbols cost nothing, and nothing else does.
#[test]
fn only_the_free_symbols_are_free() {
    let free: Vec<&str> = voln::SYMBOLS
        .iter()
        .filter(|s| s.cost == Cost::Free)
        .map(|s| s.short_name)
        .collect();
    assert_eq!(
        free,
        ["recognition", "thought", "strike", "smite", "seeking"],
        "five free symbols; `thought` is the retired one"
    );
}

/// A free ability has no favor cost at any level.
#[test]
fn a_free_symbol_costs_nothing_at_every_level() {
    let smite = voln::symbol("smite").expect("rank 21");
    for level in [3, 50, 100] {
        assert_eq!(
            voln::favor_cost(&smite.cost, level),
            None,
            "at level {level}"
        );
    }
}

/// An out-of-range level has no answer, rather than a wrong one.
///
/// A level above 100 or below 3 is outside the measured table, and guessing
/// would give a confident wrong number -- the same rule `plan/12` §5.2 applies
/// to game state.
#[test]
fn a_level_outside_the_table_has_no_cost() {
    let holiness = voln::symbol("holiness").expect("rank 9");
    assert_eq!(voln::favor_cost(&holiness.cost, 2), None, "below level 3");
    assert_eq!(
        voln::favor_cost(&holiness.cost, 101),
        None,
        "above the table"
    );
    assert_eq!(voln::favor_cost(&holiness.cost, 0), None);
}

/// **Council sign costs and timings match the wiki, all twenty.**
///
/// `reference/wiki_clean/Council of Light.txt:71-90`, which states rank, mana,
/// spirit and "Cost Paid" for every sign.
#[test]
fn every_council_sign_matches_the_wiki() {
    // (short name, rank, spirit, mana, timing) from the wiki table.
    let wiki: &[(&str, u8, u8, u8, Option<CostTiming>)] = &[
        ("recognition", 1, 0, 0, None),
        ("signal", 2, 0, 0, None),
        ("warding", 3, 0, 1, Some(CostTiming::Invoked)),
        ("striking", 4, 0, 1, Some(CostTiming::Invoked)),
        ("clotting", 5, 0, 1, Some(CostTiming::Invoked)),
        ("thought", 6, 0, 1, Some(CostTiming::Invoked)),
        ("defending", 7, 0, 2, Some(CostTiming::Invoked)),
        ("smiting", 8, 0, 2, Some(CostTiming::Invoked)),
        ("staunching", 9, 0, 1, Some(CostTiming::Invoked)),
        ("deflection", 10, 0, 3, Some(CostTiming::Invoked)),
        ("hypnosis", 11, 1, 0, Some(CostTiming::Invoked)),
        ("swords", 12, 1, 0, Some(CostTiming::Dissipates)),
        ("shields", 13, 1, 0, Some(CostTiming::Dissipates)),
        ("dissipation", 14, 1, 0, Some(CostTiming::Dissipates)),
        ("healing", 15, 2, 0, Some(CostTiming::Invoked)),
        ("madness", 16, 3, 0, Some(CostTiming::Dissipates)),
        ("possession", 17, 4, 0, Some(CostTiming::Invoked)),
        ("wracking", 18, 5, 0, Some(CostTiming::Invoked)),
        ("darkness", 19, 6, 0, Some(CostTiming::Invoked)),
        ("hopelessness", 20, 0, 0, None),
    ];
    assert_eq!(wiki.len(), col::SIGNS.len());
    for (name, rank, spirit, mana, timing) in wiki {
        let found = col::sign(name).unwrap_or_else(|| panic!("{name} is a sign"));
        assert_eq!(found.rank, *rank, "{name} rank");
        let expected = if *spirit == 0 && *mana == 0 {
            Cost::Free
        } else {
            Cost::SpiritMana {
                spirit: *spirit,
                mana: *mana,
                paid_when: *timing,
            }
        };
        assert_eq!(found.cost, expected, "{name} cost");
    }
}

/// **Sunfist sigil costs match the wiki at every value the wiki states.**
///
/// `reference/wiki_clean/Guardians of Sunfist.txt:131-149`. The wiki omits the
/// stamina column on nine rows and calls Intimidation's mana "Variable"; those
/// are the cells left out below, and the module docs record them.
#[test]
fn every_sunfist_sigil_matches_the_wiki() {
    // (short name, rank, stamina the wiki states, mana the wiki states)
    let wiki: &[(&str, u8, Option<u8>, Option<u8>)] = &[
        ("recognition", 1, None, None),
        ("location", 2, None, Some(0)),
        ("contact", 3, Some(0), Some(1)),
        ("resolve", 4, Some(5), Some(0)),
        ("minor bane", 5, None, Some(3)),
        ("bandages", 6, Some(10), Some(0)),
        ("defense", 7, None, Some(5)),
        ("offense", 8, None, Some(5)),
        ("distraction", 9, Some(10), Some(5)),
        ("minor protection", 10, None, Some(5)),
        ("focus", 11, None, Some(5)),
        ("intimidation", 12, None, None),
        ("mending", 13, Some(15), Some(10)),
        ("concentration", 14, Some(30), Some(0)),
        ("major bane", 15, None, Some(10)),
        ("determination", 16, Some(30), Some(0)),
        ("health", 17, Some(20), Some(10)),
        ("power", 18, Some(50), Some(0)),
        ("major protection", 19, Some(15), Some(10)),
        ("escape", 20, Some(75), Some(15)),
    ];
    assert_eq!(wiki.len(), sunfist::SIGILS.len());
    for (name, rank, wiki_stamina, wiki_mana) in wiki {
        let found = sunfist::sigil(name).unwrap_or_else(|| panic!("{name} is a sigil"));
        assert_eq!(found.rank, *rank, "{name} rank");
        let (stamina, mana) = match found.cost {
            Cost::Free => (0, 0),
            Cost::StaminaMana { stamina, mana } => (stamina, mana),
            other => panic!("{name} should cost stamina/mana, got {other:?}"),
        };
        if let Some(expected) = wiki_stamina {
            assert_eq!(stamina, *expected, "{name} stamina");
        }
        if let Some(expected) = wiki_mana {
            assert_eq!(mana, *expected, "{name} mana");
        }
    }
}

/// Lookup takes either name, in any case, with surrounding space.
#[test]
fn lookup_takes_either_name_in_any_case() {
    let by_short = voln::symbol("holiness");
    assert_eq!(by_short, voln::symbol("Symbol of Holiness"));
    assert_eq!(by_short, voln::symbol("HOLINESS"));
    assert_eq!(by_short, voln::symbol("  holiness  "));
    assert!(voln::symbol("").is_none());
}

/// **Lookup is scoped to one society.**
///
/// All three have a `recognition`, and they are three different abilities with
/// three different spell numbers. A caller asking Voln for `recognition` must
/// not get the Council's.
#[test]
fn each_society_has_its_own_recognition() {
    let voln = voln::symbol("recognition").expect("Voln rank 1");
    let council = col::sign("recognition").expect("Council rank 1");
    let guardians = sunfist::sigil("recognition").expect("Sunfist rank 1");

    assert_eq!(voln.spell_number, 9801);
    assert_eq!(council.spell_number, 9901);
    assert_eq!(guardians.spell_number, 9701);
    assert_eq!(voln.long_name, "Symbol of Recognition");
    assert_eq!(council.long_name, "Sign of Recognition");
    assert_eq!(guardians.long_name, "Sigil of Recognition");
}

/// **Spell numbers are unique across all three societies.**
///
/// They share one numbering space in effect lists, so a collision would make a
/// consumer attribute an active effect to the wrong society.
#[test]
fn spell_numbers_do_not_collide() {
    let mut numbers: Vec<u16> = Society::ALL
        .into_iter()
        .flat_map(|s| s.abilities().iter().map(|a| a.spell_number))
        .collect();
    assert_eq!(numbers.len(), 66, "26 + 20 + 20");
    numbers.sort_unstable();
    let before = numbers.len();
    numbers.dedup();
    assert_eq!(numbers.len(), before, "a spell number is used twice");
}

/// The command each society builds, with and without a target.
#[test]
fn each_society_builds_its_own_command() {
    let holiness = voln::symbol("holiness").expect("rank 9");
    assert_eq!(holiness.command(Target::None), "symbol of holiness");
    assert_eq!(
        holiness.command(Target::Id(12345)),
        "symbol of holiness #12345"
    );
    assert_eq!(
        holiness.command(Target::Named("kobold")),
        "symbol of holiness kobold"
    );

    let striking = col::sign("striking").expect("rank 4");
    assert_eq!(striking.command(Target::None), "sign of striking");

    let contact = sunfist::sigil("contact").expect("rank 3");
    assert_eq!(contact.command(Target::None), "sigil of contact");
}

/// **Two abilities are invoked by their own verb.**
///
/// Kai's Smite is `smite`, not `symbol of smite`; Sign of Signal is `signal`.
/// `society.rb:148`'s `entry[:usage]`, and the only two entries that set it.
#[test]
fn a_verb_ability_ignores_the_prefix() {
    let smite = voln::symbol("smite").expect("rank 21");
    assert_eq!(smite.command(Target::None), "smite");
    assert_eq!(smite.command(Target::Id(7)), "smite #7");

    let signal = col::sign("signal").expect("rank 2");
    assert_eq!(signal.command(Target::None), "signal");

    let with_usage: Vec<&str> = Society::ALL
        .into_iter()
        .flat_map(Society::abilities)
        .filter(|a| a.usage.is_some())
        .map(|a| a.long_name)
        .collect();
    assert_eq!(with_usage, ["Kai's Smite", "Sign of Signal"]);
}

/// Knowing an ability is rank-gated, and a Master knows them all.
#[test]
fn rank_gates_what_a_member_knows() {
    let holiness = voln::symbol("holiness").expect("rank 9");
    assert!(!holiness.known_at(8), "one rank short");
    assert!(holiness.known_at(9), "exactly the rank");
    assert!(holiness.known_at(26));

    for society in Society::ALL {
        let master = society.max_rank();
        assert!(
            society.abilities().iter().all(|a| a.known_at(master)),
            "a {society} Master knows every ability"
        );
        assert!(
            society.abilities().iter().any(|a| !a.known_at(1)),
            "a rank 1 {society} member does not"
        );
    }
    assert_eq!(voln::MASTER_RANK, Society::OrderOfVoln.max_rank());
    assert_eq!(col::MASTER_RANK, Society::CouncilOfLight.max_rank());
    assert_eq!(sunfist::MASTER_RANK, Society::GuardiansOfSunfist.max_rank());
}

/// **The Council's spirit check is strict and its mana check is not.**
///
/// `council_of_light.rb:359` tests `total_spirit < Char.spirit`, `:364` tests
/// `mana_cost <= Char.mana`. Asserted directly because it looks like a typo:
/// the wiki's "Obvious Spirit Drain" section is the reason, and a reader who
/// "fixes" the `<` removes a margin the game enforces socially.
#[test]
fn the_spirit_check_is_strict_and_the_mana_check_is_not() {
    let healing = col::sign("healing").expect("2 spirit, invoked");
    assert!(!col::affordable(healing, 2, 0, 0), "2 spirit is not enough");
    assert!(col::affordable(healing, 3, 0, 0), "3 is");

    let deflection = col::sign("deflection").expect("3 mana, invoked");
    assert!(
        col::affordable(deflection, 0, 3, 0),
        "exactly 3 mana IS enough, unlike spirit"
    );
    assert!(!col::affordable(deflection, 0, 2, 0));
}

/// **A dissipating sign counts the spirit already owed.**
///
/// The cost is taken when the effect expires, so stacking them commits spirit
/// that a naive check would let the member spend twice.
#[test]
fn a_dissipating_sign_counts_pending_spirit() {
    let swords = col::sign("swords").expect("1 spirit, dissipates");
    assert_eq!(
        swords.cost,
        Cost::SpiritMana {
            spirit: 1,
            mana: 0,
            paid_when: Some(CostTiming::Dissipates)
        }
    );
    // With nothing owed, 2 spirit affords a 1-spirit sign.
    assert!(col::affordable(swords, 2, 0, 0));
    // With 1 already owed by an active sign, it does not: 1 + 1 is not < 2.
    assert!(!col::affordable(swords, 2, 0, 1));
    assert!(col::affordable(swords, 3, 0, 1));

    // An invoked sign of the same cost is unaffected by pending spirit.
    let hypnosis = col::sign("hypnosis").expect("1 spirit, invoked");
    assert!(col::affordable(hypnosis, 2, 0, 1));
}

/// A free sign is affordable with nothing at all.
#[test]
fn a_free_sign_costs_nothing() {
    let recognition = col::sign("recognition").expect("rank 1");
    assert_eq!(recognition.cost, Cost::Free);
    assert!(
        !col::affordable(recognition, 0, 0, 0),
        "affordable() answers only for signs that have a spirit/mana cost"
    );
}

/// Sunfist affordability takes both currencies, inclusively.
#[test]
fn a_sigil_needs_both_its_currencies() {
    let mending = sunfist::sigil("mending").expect("15 stamina, 10 mana");
    assert!(mending.known_at(13));
    assert!(sunfist::affordable(mending, 15, 10), "exactly enough");
    assert!(!sunfist::affordable(mending, 14, 10), "stamina short");
    assert!(!sunfist::affordable(mending, 15, 9), "mana short");

    let recognition = sunfist::sigil("recognition").expect("free");
    assert!(sunfist::affordable(recognition, 0, 0));
}

/// Every ability has a kind, and each society uses more than one.
///
/// Lich declares `:type` on exactly one Sunfist sigil and leaves the other
/// nineteen `nil`; this asserts the gap is filled rather than reproduced.
#[test]
fn every_ability_has_a_kind() {
    for society in Society::ALL {
        let kinds: std::collections::BTreeSet<AbilityKind> =
            society.abilities().iter().map(|a| a.kind).collect();
        assert!(
            kinds.len() > 1,
            "{society} should use more than one kind, got {kinds:?}"
        );
    }
    let attacks: Vec<&str> = sunfist::SIGILS
        .iter()
        .filter(|s| s.kind == AbilityKind::Attack)
        .map(|s| s.short_name)
        .collect();
    assert_eq!(attacks, ["distraction", "intimidation"]);
    assert_eq!(AbilityKind::Attack.as_str(), "attack");
}

/// The table is reachable from the society, not only from its module.
#[test]
fn abilities_are_reachable_from_the_society() {
    let from_society: Vec<&Ability> = Society::OrderOfVoln.abilities().iter().collect();
    let from_module: Vec<&Ability> = voln::SYMBOLS.iter().collect();
    assert_eq!(from_society, from_module);
    assert_eq!(Society::OrderOfVoln.command_prefix(), "symbol of");
    assert_eq!(Society::CouncilOfLight.command_prefix(), "sign of");
    assert_eq!(Society::GuardiansOfSunfist.command_prefix(), "sigil of");
}
