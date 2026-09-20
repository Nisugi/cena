//! The bestiary, spot-checked against the Ruby source.
//!
//! The oracle is `reference/lich-5/lib/gemstone/creatures/`, read with Ruby
//! rather than by eye or from the TSV this port generated:
//!
//! ```text
//! albino_tomb_spider: level=8 hp=83 noun="spider" family="Arachnid"
//!   melee=56..147  udf=87..145  wiz_td=nil  mjs_td=60..66
//!   sleepable=true limbs=nil blood=true
//!   skin="multi-faceted tomb spider eye"
//!   areas=[{name: "The Graveyard", uids: [2162113..2162122]}]
//!   deaths=4
//! spider-noun creatures: 5   levels: min=1 max=120
//! ```
//!
//! # Why spot checks rather than a full parity digest
//!
//! `crit_parity.rs` digests its table because 2,394 entries are one shape.
//! These are 627 rows across four tables with optional everything, and a
//! digest would assert "the bytes did not change" rather than "the values
//! mean what they should". The checks below are chosen to fail on the
//! mistakes the extractor could plausibly make: a tri-state collapsed to
//! false, a range flattened to a scalar, a column index off by one, a table
//! silently not joined.

use cena_model::state::creature::{
    MessageKind, Stat, by_name, by_noun, creature, creatures, in_room, unparsed_attack_strengths,
};

/// Every table loads, with the counts the extractor reported.
#[test]
fn every_table_loads() {
    assert_eq!(creatures().count(), 627, "627 creature templates");
    // **604 templates name an area; only 584 give a room UID.** The other 20
    // are wiki presence info with no measured rooms, which
    // `_creature_template.rb` calls out explicitly: *"Empty uids = wiki
    // presence info without measured room data."* Only spans are loaded, so
    // 584 is the right number here -- an earlier draft of this test asserted
    // 604 and was wrong about which quantity it was counting.
    assert_eq!(
        creatures().filter(|c| !c.areas.is_empty()).count(),
        584,
        "584 templates record a room UID span"
    );
    assert_eq!(
        creatures().map(|c| c.areas.len()).sum::<usize>(),
        1394,
        "1,394 room UID spans"
    );
    assert_eq!(
        creatures().map(|c| c.attacks.len()).sum::<usize>(),
        1603,
        "1,603 physical attacks"
    );
    assert_eq!(
        creatures()
            .flat_map(|c| MessageKind::ALL.map(|k| c.messages_of(k).len()))
            .sum::<usize>(),
        3862,
        "3,862 death/flee/arrival/decay lines"
    );
}

/// One creature's scalars match the Ruby.
#[test]
fn a_creature_carries_its_stats() {
    let Some(spider) = creature("albino_tomb_spider") else {
        panic!("albino_tomb_spider must be in the bestiary");
    };
    assert_eq!(spider.name, "albino tomb spider");
    assert_eq!(spider.noun.as_deref(), Some("spider"));
    assert_eq!(spider.level, Some(8));
    assert_eq!(spider.max_hp, Some(Stat::Exact(83)));
    assert_eq!(spider.family.as_deref(), Some("Arachnid"));
    assert_eq!(spider.size.as_deref(), Some("medium"));
    assert_eq!(spider.asg.as_deref(), Some("1N"));
    assert_eq!(
        spider.treasure.skin.as_deref(),
        Some("multi-faceted tomb spider eye")
    );
    assert!(spider.treasure.coins, "it drops coins");
    assert!(!spider.treasure.is_empty());
    assert!(!spider.boss);
}

/// **A defense is usually a RANGE, and the range is kept.**
///
/// MEASURED: `melee` is a range 511 times, a scalar 37 times, absent 79 times.
/// Flattening to a midpoint would report a precision the bestiary never
/// claimed, and flattening to the low end would under-report every creature's
/// defense.
#[test]
fn a_defense_keeps_its_range() {
    let Some(spider) = creature("albino_tomb_spider") else {
        panic!("albino_tomb_spider");
    };
    assert_eq!(spider.melee_ds, Some(Stat::Range(56, 147)));
    assert_eq!(spider.udf, Some(Stat::Range(87, 145)));
    assert!(spider.melee_ds.is_some_and(Stat::is_range));

    // The accessors read the span rather than a single number.
    assert_eq!(spider.melee_ds.map(Stat::low), Some(56));
    assert_eq!(spider.melee_ds.map(Stat::high), Some(147));

    // A scalar is not a range, and says so.
    assert_eq!(spider.max_hp.map(Stat::is_range), Some(false));
    assert_eq!(spider.max_hp.map(Stat::low), Some(83));
    assert_eq!(spider.max_hp.map(Stat::high), Some(83));
}

/// **Target defense is per profession, and a missing one is absent.**
///
/// `wiz_td` is `nil` for this creature and `mjs_td` is `60..66`. A caster
/// reading their own circle must not get another's, and must not get a zero
/// for one nobody recorded.
#[test]
fn target_defense_is_per_profession() {
    let Some(spider) = creature("albino_tomb_spider") else {
        panic!("albino_tomb_spider");
    };
    assert_eq!(
        spider.target_defense.get("mjs_td"),
        Some(&Stat::Range(60, 66))
    );
    assert_eq!(spider.target_defense.get("bar_td"), Some(&Stat::Exact(24)));
    assert_eq!(
        spider.target_defense.get("wiz_td"),
        None,
        "nil in the source is absent here, not zero"
    );
    assert_eq!(spider.target_defense.get("not_a_profession"), None);
}

/// **The tri-state is preserved, and `None` means nobody measured it.**
///
/// `_creature_template.rb`: *"true/false/nil if unknown"*. MEASURED across the
/// 627: `sleepable` is unknown for **342**, `limbs` for 298, `blood` for 161.
/// Collapsing unknown to false would tell a sorcerer that 342 creatures resist
/// Sleep when the truth is that nobody has tried.
#[test]
fn an_unmeasured_flag_is_none_not_false() {
    let Some(spider) = creature("albino_tomb_spider") else {
        panic!("albino_tomb_spider");
    };
    assert_eq!(spider.sleepable, Some(true), "measured, and yes");
    assert_eq!(spider.blood, Some(true));
    assert_eq!(spider.bones, Some(false), "measured, and no");
    assert_eq!(spider.limbs, None, "NOT measured -- must not read as false");

    // The distribution, which is what makes the distinction load-bearing.
    let unknown = creatures().filter(|c| c.sleepable.is_none()).count();
    assert_eq!(unknown, 342, "sleepable is unknown for 342 of 627");
    let known_false = creatures().filter(|c| c.sleepable == Some(false)).count();
    assert_eq!(known_false, 158, "and measured-false for only 158");
    assert_eq!(creatures().filter(|c| c.limbs.is_none()).count(), 298);
    assert_eq!(creatures().filter(|c| c.blood.is_none()).count(), 161);
}

/// Areas load as room UID spans, and answer containment.
#[test]
fn a_creature_knows_where_it_lives() {
    let Some(spider) = creature("albino_tomb_spider") else {
        panic!("albino_tomb_spider");
    };
    assert_eq!(spider.areas.len(), 1);
    assert_eq!(spider.areas[0].name, "The Graveyard");
    assert_eq!(spider.areas[0].uid_low, 2_162_113);
    assert_eq!(spider.areas[0].uid_high, 2_162_122);

    assert!(spider.found_at(2_162_113), "the first room of the span");
    assert!(spider.found_at(2_162_118), "the middle");
    assert!(spider.found_at(2_162_122), "the last, inclusive");
    assert!(!spider.found_at(2_162_112), "one below");
    assert!(!spider.found_at(2_162_123), "one above");
}

/// The room index finds it, and finds only creatures that live there.
#[test]
fn a_room_lists_its_creatures() {
    let here = in_room(2_162_118);
    assert!(
        here.iter().any(|c| c.id == "albino_tomb_spider"),
        "the spider hunts in The Graveyard"
    );
    assert!(
        here.iter().all(|c| c.found_at(2_162_118)),
        "every answer must actually be found there"
    );
    assert!(
        in_room(1).is_empty(),
        "a room no template mentions has no creatures"
    );
}

/// Attacks load with their attack strength.
#[test]
fn attacks_carry_their_strength() {
    let Some(spider) = creature("albino_tomb_spider") else {
        panic!("albino_tomb_spider");
    };
    let claw = spider
        .attacks
        .iter()
        .find(|a| a.name == "Claw")
        .expect("the spider claws");
    assert_eq!(claw.attack_strength, Some(Stat::Range(106, 116)));
    assert_eq!(claw.attack_strength_raw, None, "this one parsed");

    // The worst case across every attack, which is what a defensive check wants.
    assert_eq!(spider.max_attack_strength(), Some(116));
}

/// **Six attack strengths are malformed IN THE SOURCE and are kept.**
///
/// `"566 to"`, `"(lunge) 245-276"`, `"390 UAF"`, `""`. Data-entry damage in
/// Lich's bestiary rather than a shape worth modelling -- so the text is kept
/// verbatim, the number is `None`, and the count is reportable rather than
/// silent (Rule 2.2).
#[test]
fn a_malformed_attack_strength_is_kept_not_dropped() {
    assert_eq!(
        unparsed_attack_strengths(),
        6,
        "more means the source changed or the parse regressed"
    );

    let Some(vampire) = creature("ashen_patrician_vampire") else {
        panic!("ashen_patrician_vampire");
    };
    let rapier = vampire
        .attacks
        .iter()
        .find(|a| a.name == "Rapier")
        .expect("it has a rapier");
    assert_eq!(rapier.attack_strength, None, "`566 to` is not a number");
    assert_eq!(
        rapier.attack_strength_raw.as_deref(),
        Some("566 to"),
        "but the text survives, so the damage is visible rather than invented"
    );
}

/// Death messages load.
#[test]
fn a_creature_knows_what_it_says_when_it_dies() {
    let Some(spider) = creature("albino_tomb_spider") else {
        panic!("albino_tomb_spider");
    };
    let deaths = spider.messages_of(MessageKind::Death);
    assert_eq!(deaths.len(), 4, "four death lines in the source");
    assert!(
        deaths
            .iter()
            .any(|line| line == "The tomb spider collapses to the ground and dies."),
        "got {deaths:?}"
    );

    let flees = spider.messages_of(MessageKind::Flee);
    assert_eq!(flees.len(), 2);
    assert!(
        flees.iter().all(|line| line.contains("{direction}")),
        "flee lines carry a direction placeholder: {flees:?}"
    );

    assert!(
        spider.messages_of(MessageKind::Arrival).is_empty(),
        "and a kind with no lines is empty rather than missing"
    );
}

/// **A noun is not a unique key**, so every match is returned.
///
/// MEASURED: five templates use the noun `spider`, at different levels. A
/// caller told there is one would attack with the wrong expectations -- the
/// same reasoning `resolve_noun` and the armament aliases record.
#[test]
fn a_noun_returns_every_creature_that_uses_it() {
    let spiders = by_noun("spider");
    assert_eq!(spiders.len(), 5, "five creatures answer to `spider`");
    assert!(spiders.iter().any(|c| c.id == "albino_tomb_spider"));

    let levels: Vec<Option<i32>> = spiders.iter().map(|c| c.level).collect();
    assert!(
        levels.iter().flatten().copied().max() != levels.iter().flatten().copied().min(),
        "and they are not the same creature: {levels:?}"
    );
}

/// Lookup by display name, case-insensitively.
#[test]
fn a_creature_is_found_by_name() {
    assert_eq!(
        by_name("albino tomb spider").first().map(|c| c.id.as_str()),
        Some("albino_tomb_spider")
    );
    assert_eq!(
        by_name("ALBINO TOMB SPIDER").first().map(|c| c.id.as_str()),
        Some("albino_tomb_spider"),
        "case does not matter"
    );
    assert!(by_name("an entirely fictional beast").is_empty());
    assert!(creature("no_such_creature").is_none());
}

/// No row is half-parsed.
///
/// A creature that loaded with empty strings would satisfy every spot check
/// above by being absent from them.
#[test]
fn no_creature_row_is_half_parsed() {
    for c in creatures() {
        assert!(!c.id.is_empty(), "a creature with no id");
        assert!(!c.name.is_empty(), "{} has no name", c.id);
        // Levels are the field a hunting consumer filters on, and 95% have one.
        // Asserted as a floor rather than per-row, since 30 genuinely lack it.
        assert!(
            c.level.is_none_or(|level| (1..=200).contains(&level)),
            "{} has an implausible level: {:?}",
            c.id,
            c.level
        );
        for area in &c.areas {
            assert!(
                area.uid_low <= area.uid_high,
                "{} has an inverted UID span",
                c.id
            );
        }
    }
    assert_eq!(
        creatures().filter(|c| c.level.is_some()).count(),
        597,
        "597 of 627 record a level"
    );
}
