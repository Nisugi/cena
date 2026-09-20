//! Weapon, armor and shield tables, spot-checked against the Ruby source.
//!
//! The oracle is `reference/lich-5/lib/gemstone/armaments/`, read with Ruby
//! rather than by eye:
//!
//! ```text
//! broadsword DF:  [nil, 0.45, 0.3, 0.25, 0.225, 0.2]
//! broadsword AvD: asg1=36, asg20=18;  RT 5/4;  15 names
//! asg20:          full_plate, weight 75, CvA -13, hindrance_max 96
//! ```
//!
//! # Why spot checks rather than a full parity test
//!
//! `crit_parity.rs` digests the whole crit table because its 2,394 entries
//! are one shape. These are three shapes with positional arrays, and a digest
//! would assert "the bytes did not change" rather than "the values mean what
//! they should". The checks below are chosen to fail on the mistakes the
//! extractor could plausibly make: an off-by-one in a positional array, a
//! collapsed `nil`, a dropped table.

use cena_model::state::armaments::{
    ArmamentKind, Coverage, alias_count, armor, armors, resolve, shield, shields, weapon, weapons,
    weapons_named,
};

/// **The same weapon can appear in two categories with different stats.**
///
/// MEASURED: eleven do. A bastard sword is `edged` at DF 0.45 one-handed and
/// `two_handed` at 0.55; a handaxe is `edged` and `thrown`; a cestus is
/// `brawling` and `unarmed`. Keying the table by id alone silently kept 85 of
/// 96 rows, which `every_table_loads` caught.
#[test]
fn a_weapon_can_belong_to_two_categories() {
    let one_handed = weapon("edged", "bastard_sword").and_then(|w| w.damage_factor_vs(1));
    let two_handed = weapon("two_handed", "bastard_sword").and_then(|w| w.damage_factor_vs(1));
    assert_eq!(one_handed, Some(0.45), "swung one-handed");
    assert_eq!(two_handed, Some(0.55), "and harder with both hands");

    let both: Vec<&str> = weapons_named("bastard_sword")
        .map(|w| w.category.as_str())
        .collect();
    assert_eq!(both, ["edged", "two_handed"]);
}

/// The tables load, with the counts the extractor reported.
///
/// A dropped table is the failure this catches: `shield_stats.rb` was loaded
/// and never written by the first version of the extractor, and nothing else
/// here would have noticed.
#[test]
fn every_table_loads() {
    assert_eq!(weapons().count(), 96, "96 weapons");
    assert_eq!(
        armors().count(),
        18,
        "20 sub-group rows, two of which are the declared-and-empty ag_1 \
         asg_3 and asg_4"
    );
    assert_eq!(shields().count(), 4, "small, medium, large, tower");
    assert_eq!(alias_count(), 706);
}

/// A weapon's scalars match the Ruby.
#[test]
fn a_weapon_carries_its_stats() {
    let Some(sword) = weapon("edged", "broadsword") else {
        panic!("broadsword must be in the table");
    };
    assert_eq!(sword.category, "edged");
    assert_eq!(sword.base_name, "broadsword");
    assert_eq!(sword.base_rt, Some(5));
    assert_eq!(sword.min_rt, Some(4));
}

/// **Damage factor is positional, and index 0 is unused.**
///
/// `[nil, 0.45, 0.3, 0.25, 0.225, 0.2]` -- an off-by-one here would report a
/// weapon's cloth DF against leather, which is the kind of error that looks
/// plausible in every individual reading.
#[test]
fn damage_factor_is_indexed_by_armor_group() {
    let Some(sword) = weapon("edged", "broadsword") else {
        panic!("broadsword");
    };
    assert_eq!(sword.damage_factor_vs(1), Some(0.45), "vs cloth");
    assert_eq!(sword.damage_factor_vs(2), Some(0.3), "vs leather");
    assert_eq!(sword.damage_factor_vs(5), Some(0.2), "vs plate");
    assert_eq!(
        sword.damage_factor.first().copied().flatten(),
        None,
        "index 0 is unused and must stay None, not 0.0"
    );
}

/// **`AvD` is indexed by armor SUB-group, 1-based.**
#[test]
fn avd_is_indexed_by_armor_sub_group() {
    let Some(sword) = weapon("edged", "broadsword") else {
        panic!("broadsword");
    };
    assert_eq!(sword.avd_vs(1), Some(36));
    assert_eq!(sword.avd_vs(20), Some(18));
    assert_eq!(sword.avd_vs(0), None, "there is no sub-group 0");
    assert_eq!(sword.avd_vs(21), None, "nor 21");
}

/// An armor sub-group's stats match the Ruby.
#[test]
fn armor_carries_its_stats() {
    let Some(plate) = armor("full_plate") else {
        panic!("full_plate must be in the table");
    };
    assert_eq!(plate.kind, "plate");
    assert_eq!(plate.armor_group, 5);
    assert_eq!(plate.armor_sub_group, 20);
    assert_eq!(plate.base_weight, Some(75));
    assert_eq!(plate.normal_cva, Some(-13));
    assert_eq!(plate.hindrance_max, Some(96));
}

/// **Crit divisor is derived from the armor group, not stored.**
///
/// `armor_stats.rb:352-357`. An earlier extractor wrote an empty column for
/// this, having guessed a field name the data does not have.
#[test]
fn the_crit_divisor_comes_from_the_armor_group() {
    let by_name = |name: &str| armor(name).and_then(cena_model::Armor::crit_divisor);
    assert_eq!(by_name("normal_clothing"), Some(5), "cloth");
    assert_eq!(by_name("full_plate"), Some(11), "plate");

    // Every armor resolves to one of the five divisors.
    for a in armors() {
        assert!(
            matches!(a.crit_divisor(), Some(5 | 6 | 7 | 9 | 11)),
            "{} has divisor {:?}",
            a.base_name,
            a.crit_divisor()
        );
    }
}

/// **Coverage buckets the sub-group into four ranges.**
#[test]
fn coverage_buckets_the_sub_group() {
    assert_eq!(
        armor("normal_clothing").and_then(cena_model::Armor::coverage),
        Some(Coverage::Torso),
        "asg 1"
    );
    assert_eq!(
        armor("full_plate").and_then(cena_model::Armor::coverage),
        Some(Coverage::TorsoArmsLegsAndHead),
        "asg 20"
    );
}

/// **An empty hindrance slot is `None`, not zero.**
///
/// Indices 8, 15 and 18 correspond to no live spell circle. Collapsing them
/// to zero would make the table claim a Savant hindrance nobody has measured
/// -- the same rule `plan/12` §5.2 applies to game state.
#[test]
fn an_absent_circle_is_none_not_zero() {
    let Some(cloth) = armor("normal_clothing") else {
        panic!("normal_clothing");
    };
    assert_eq!(
        cloth.hindrance_for_circle(1),
        Some(0),
        "Minor Spiritual: a real circle with no hindrance"
    );
    assert_eq!(
        cloth.hindrance_for_circle(8),
        None,
        "index 8 is not a circle, so it is unknown rather than zero"
    );
    assert_eq!(cloth.hindrances.len(), 20, "twenty positional slots");
}

/// **The aliases are the reason this table is worth porting.**
///
/// `a flyssa` on the wire is a broadsword. That mapping lives only here.
#[test]
fn an_alias_resolves_to_its_weapon() {
    assert_eq!(resolve(ArmamentKind::Weapon, "flyssa"), ["broadsword"]);
    assert_eq!(resolve(ArmamentKind::Weapon, "katzbalger"), ["broadsword"]);
    assert_eq!(
        resolve(ArmamentKind::Weapon, "broadsword"),
        ["broadsword"],
        "the canonical name is an alias of itself"
    );
}

/// Alias lookup is case-insensitive and trims.
#[test]
fn alias_lookup_is_forgiving_of_case() {
    assert_eq!(resolve(ArmamentKind::Weapon, "FLYSSA"), ["broadsword"]);
    assert_eq!(resolve(ArmamentKind::Weapon, "  flyssa  "), ["broadsword"]);
}

/// **An alias is not always unique, and every match is returned.**
///
/// MEASURED: `aketon` names both `double_leather` and `reinforced_leather`.
/// Returning one would attribute the wrong stats to a real item -- the same
/// reasoning `resolve_noun` records for room objects.
#[test]
fn an_ambiguous_alias_returns_every_match() {
    let matches = resolve(ArmamentKind::Armor, "aketon");
    assert!(
        matches.len() > 1,
        "aketon names more than one armor: {matches:?}"
    );
    assert!(matches.contains(&"double_leather".to_owned()));
    assert!(matches.contains(&"reinforced_leather".to_owned()));
}

/// The kind scopes the lookup.
///
/// A caller asking "what weapon is this" must not get an armor answer, even
/// where a name appears in both tables.
#[test]
fn the_kind_scopes_the_lookup() {
    assert!(
        resolve(ArmamentKind::Armor, "flyssa").is_empty(),
        "flyssa is a weapon, not armor"
    );
    assert!(resolve(ArmamentKind::Weapon, "an entirely fictional thing").is_empty());
}

/// Shields load with their modifiers.
#[test]
fn shields_carry_their_modifiers() {
    let Some(small) = shield("small_shield") else {
        panic!("small_shield must be in the table");
    };
    assert_eq!(small.base_name, "small shield");
    assert_eq!(small.size_modifier, Some(-0.15));
    assert_eq!(small.evade_modifier, Some(-0.22));
    assert_eq!(small.base_weight, Some(6));
}

/// Every weapon has a category and a non-empty name.
///
/// A row that parsed into a `Weapon` with empty strings would satisfy every
/// spot check above by being absent from them.
#[test]
fn no_weapon_row_is_half_parsed() {
    for w in weapons() {
        assert!(!w.id.is_empty(), "a weapon with no id");
        assert!(!w.category.is_empty(), "{} has no category", w.id);
        assert!(!w.base_name.is_empty(), "{} has no base name", w.id);
        assert!(
            !w.damage_factor.is_empty(),
            "{} has no damage factors at all",
            w.id
        );
    }
}
