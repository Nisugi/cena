//! The closed vocabularies round-trip, and their sets are complete.
//!
//! M3 step 2. Each enum in `state/character/vocabulary.rs` is the alternation of
//! a Lich regex transcribed by hand, so the failure mode worth testing is a
//! **transcription error** -- a missing variant, a typo in a wire spelling, or an
//! `ALL` that has drifted from the variant list.
//!
//! # What makes each of these able to fail
//!
//! A test that only checked `parse(x.as_str()) == Some(x)` would pass on a set
//! missing half its variants: every variant present round-trips fine. So each
//! enum is also asserted against **the literal alternation from the regex**,
//! which is the thing `ALL` can drift from.

use cena_model::{AccountType, Che, DeathsSting, PsmCategory, ResourceType, Society, Warcry};

/// Assert `ALL` is exactly `expected`, in order, and that every member
/// round-trips through `as_str`/`parse`.
///
/// No `unwrap`/`expect`/`panic!` in a helper: the workspace denies all three and
/// `clippy.toml`'s allowance covers `#[test]` functions only. Failures surface as
/// `assert_eq!` on the collected vectors.
fn check<T>(all: &[T], expected: &[&str], parse: fn(&str) -> Option<T>, what: &str)
where
    T: Copy + std::fmt::Debug + PartialEq,
{
    let spellings: Vec<String> = all.iter().map(|v| format!("{v:?}")).collect();
    assert_eq!(
        all.len(),
        expected.len(),
        "{what}: ALL has {} entries, the wire's alternation has {}. A missing \
         variant is invisible to a round-trip test, which is why this counts. \
         Got {spellings:?}",
        all.len(),
        expected.len()
    );
    for (value, want) in all.iter().zip(expected) {
        assert_eq!(
            parse(want),
            Some(*value),
            "{what}: the wire spelling {want:?} must parse to {value:?}"
        );
    }
    for (i, value) in all.iter().enumerate() {
        assert_eq!(
            parse(expected[i]),
            Some(*value),
            "{what}: {value:?} must round-trip"
        );
    }
    assert_eq!(
        parse("definitely not a member of this set"),
        None,
        "{what}: an unknown value is None, not a silent default"
    );
}

#[test]
fn deaths_sting_matches_the_wire_alternation() {
    // `parser.rb:19`, the TotalExp regex:
    //   Death's Sting: (?<deaths_sting>None|Light|Moderate|Sharp|Harsh|Piercing|Crushing)
    check(
        &DeathsSting::ALL,
        &[
            "None", "Light", "Moderate", "Sharp", "Harsh", "Piercing", "Crushing",
        ],
        DeathsSting::parse,
        "DeathsSting",
    );
}

#[test]
fn deaths_sting_orders_by_severity() {
    // `Ord` is derived, so the variant ORDER is the severity order and a
    // behavior can compare rather than look up. If someone alphabetises the
    // variants this fails, which is the point.
    assert!(
        DeathsSting::None < DeathsSting::Light,
        "no sting is better than a light one"
    );
    assert!(
        DeathsSting::Crushing > DeathsSting::Piercing,
        "crushing is the worst; if the variants get reordered, this is the test \
         that says so"
    );
    let mut sorted = DeathsSting::ALL;
    sorted.sort_unstable();
    assert_eq!(
        sorted,
        DeathsSting::ALL,
        "ALL is already in severity order, so sorting must not move anything"
    );
}

#[test]
fn society_matches_the_wire_alternation() {
    // `parser.rb:38`: the (?<society>Order of Voln|Council of Light|Guardians of Sunfist)
    check(
        &Society::ALL,
        &["Order of Voln", "Council of Light", "Guardians of Sunfist"],
        Society::parse,
        "Society",
    );
}

#[test]
fn a_master_rank_differs_by_society() {
    // The wire does NOT send a Master's rank -- `parser.rb:38`'s (?<rank>) group
    // is absent on a Master line -- so this number comes from knowledge of the
    // game, transcribed from `parser.rb:400-407`.
    assert_eq!(
        Society::OrderOfVoln.max_rank(),
        26,
        "Voln has 26 steps, which is why this is not one shared constant"
    );
    assert_eq!(Society::CouncilOfLight.max_rank(), 20);
    assert_eq!(Society::GuardiansOfSunfist.max_rank(), 20);
}

#[test]
fn resource_types_match_the_wire_alternation() {
    // `parser.rb:51-52`, shared by the Resource and Suffused regexes.
    check(
        &ResourceType::ALL,
        &[
            "Essence",
            "Necrotic Energy",
            "Lore Knowledge",
            "Motes of Tranquility",
            "Devotion",
            "Nature's Grace",
            "Grit",
            "Luck Inspiration",
            "Guile",
            "Vitality",
        ],
        ResourceType::parse,
        "ResourceType",
    );
}

#[test]
fn warcries_match_the_wire_alternation() {
    // `parser.rb:43`.
    check(
        &Warcry::ALL,
        &[
            "Bertrandt's Bellow",
            "Yertie's Yowlp",
            "Gerrelle's Growl",
            "Seanette's Shout",
            "Carn's Cry",
            "Horland's Holler",
        ],
        Warcry::parse,
        "Warcry",
    );
}

#[test]
fn a_warcry_knows_both_of_the_names_lich_stores_it_under() {
    // **The bug this type makes unrepresentable.**
    //
    // Lich writes `warcry.<second word>` when one is learned (`parser.rb:379`,
    // `match[:name].split(' ')[1]`) and zeroes `warcry.bertrandts_bellow` when
    // the character is not a warrior (`parser.rb:369-374`). The two key spaces
    // never meet, so a learned war cry can never be un-learned.
    //
    // One variant, two spellings, no drift.
    assert_eq!(Warcry::BertrandtsBellow.short_name(), "bellow");
    assert_eq!(Warcry::BertrandtsBellow.as_str(), "Bertrandt's Bellow");

    // Every short name is the lowercased second word of the full name --
    // asserted rather than assumed, because that is the derivation Lich used.
    for cry in Warcry::ALL {
        let second = cry.as_str().split(' ').nth(1).unwrap_or_default();
        assert_eq!(
            cry.short_name(),
            second.to_lowercase(),
            "{cry:?}: short_name must be the second word of {:?}",
            cry.as_str()
        );
    }

    // And the short names are distinct, or two cries would share a key.
    let mut shorts: Vec<&str> = Warcry::ALL.iter().map(|c| c.short_name()).collect();
    shorts.sort_unstable();
    let before = shorts.len();
    shorts.dedup();
    assert_eq!(
        before,
        shorts.len(),
        "short names must be unique: {shorts:?}"
    );
}

#[test]
fn psm_categories_match_the_wire_alternation() {
    // The KEY prefix, which is what `Infomon.get("cman.x")` uses.
    //
    // **FIVE, not six.** This listed `ascension` between `armor` and `cman`,
    // because `infomon/cli.rb`'s sync issues six `<x> list all` commands
    // together and I read that grouping as the game's taxonomy. It is Lich's
    // sync convenience. See `a_psm_categorys_heading_is_not_its_key` for the
    // author's correction and the three ways the wire agrees with it.
    check(
        &PsmCategory::ALL,
        &["armor", "cman", "feat", "shield", "weapon"],
        PsmCategory::parse,
        "PsmCategory",
    );
}

#[test]
fn a_psm_categorys_heading_is_not_its_key() {
    // **The asymmetry worth a test.** "Combat Maneuvers" is stored under
    // `cman`, and Lich bridges the two with a regex `case`
    // (`parser.rb:205-220`). Here they are two methods on one value, so they
    // cannot drift -- but only if both are exercised.
    assert_eq!(PsmCategory::CombatManeuver.as_str(), "cman");
    assert_eq!(PsmCategory::CombatManeuver.heading(), "Combat Maneuvers");

    // Every heading from `parser.rb:29`'s PSMStart alternation resolves.
    for (heading, want) in [
        ("Armor Specializations", PsmCategory::Armor),
        ("Combat Maneuvers", PsmCategory::CombatManeuver),
        ("Feats", PsmCategory::Feat),
        ("Shield Specializations", PsmCategory::Shield),
        ("Weapon Techniques", PsmCategory::Weapon),
    ] {
        assert_eq!(
            PsmCategory::parse_heading(heading),
            Some(want),
            "the `list all` header {heading:?} must resolve to {want:?}"
        );
    }
    assert_eq!(
        PsmCategory::parse_heading("cman"),
        None,
        "a key prefix is not a heading; the two namespaces stay separate"
    );

    // **Ascension is NOT a PSM**, and this asserts the exclusion rather than
    // leaving it as a variant nobody wrote.
    //
    // > **AUTHOR, 2026-09-19:** *"there's a few .. cman, shield, weapon,
    // > armor, feat are psms"*
    //
    // This enum had six variants, taken from `infomon/cli.rb`'s sync list
    // where six `<x> list all` commands sit together. That grouping is Lich's
    // sync convenience, not the game's taxonomy, and the wire agrees with the
    // author: ascension uses the `as follows:` header, has no
    // `Subcategory: all` terminator, and its rows are SKILL names
    // (`agility`, `edgedweapons`, `slblessings`) rather than maneuver
    // mnemonics. See `state/character/psm.rs`'s `AscensionTable`.
    assert_eq!(PsmCategory::ALL.len(), 5);
    assert_eq!(PsmCategory::parse("ascension"), None);
    assert_eq!(PsmCategory::parse_heading("Ascension Abilities"), None);
}

#[test]
fn account_types_accept_both_the_wire_and_the_canonical_spelling() {
    // `parser.rb:78-79` reads F2P|Standard|Premium|Platinum off the wire, and
    // `:579-580` renames on write. Both sides parse.
    for (wire, want) in [
        ("F2P", AccountType::Free),
        ("Standard", AccountType::Normal),
        ("Premium", AccountType::Premium),
        ("Platinum", AccountType::Platinum),
    ] {
        assert_eq!(
            AccountType::parse(wire),
            Some(want),
            "the wire spelling {wire:?} must parse"
        );
    }
    for tier in AccountType::ALL {
        assert_eq!(
            AccountType::parse(tier.as_str()),
            Some(tier),
            "{tier:?} must round-trip through its canonical name"
        );
    }
}

#[test]
fn platinum_does_not_collapse_into_premium() {
    // **The lossy conversion this type refuses to reproduce.**
    //
    // `parser.rb:579-580` maps Platinum -> Premium before storing, so
    // `account.type` cannot tell them apart. Platinum is a different game
    // instance with different mechanics; conflating them loses a fact a
    // behavior may need.
    assert_ne!(
        AccountType::parse("Platinum"),
        AccountType::parse("Premium"),
        "Platinum and Premium are different tiers and must stay distinguishable"
    );
    assert_eq!(AccountType::ALL.len(), 4, "four wire values, four variants");
}

#[test]
fn che_houses_match_the_wire_alternation() {
    // `parser.rb:82`, repeated verbatim at `:83` and `:84`. Seventeen.
    check(
        &Che::ALL,
        &[
            "Argent Aspis",
            "Rising Phoenix",
            "Paupers",
            "Arcane Masters",
            "Brigatta",
            "Twilight Hall",
            "Silvergate Inn",
            "Sovyn",
            "Sylvanfair",
            "Helden Hall",
            "White Haven",
            "Beacon Hall",
            "Rone Academy",
            "Willow Hall",
            "Moonstone Abbey",
            "Obsidian Tower",
            "Cairnfang Manor",
        ],
        Che::parse,
        "Che",
    );
}

#[test]
fn every_vocabulary_displays_as_its_wire_spelling() {
    // `Display` exists so a frontend can print these without a match. It must
    // agree with `as_str`, or the two drift and only one gets updated.
    for v in DeathsSting::ALL {
        assert_eq!(v.to_string(), v.as_str());
    }
    for v in Society::ALL {
        assert_eq!(v.to_string(), v.as_str());
    }
    for v in ResourceType::ALL {
        assert_eq!(v.to_string(), v.as_str());
    }
    for v in Warcry::ALL {
        assert_eq!(v.to_string(), v.as_str());
    }
    for v in PsmCategory::ALL {
        assert_eq!(v.to_string(), v.as_str());
    }
    for v in AccountType::ALL {
        assert_eq!(v.to_string(), v.as_str());
    }
    for v in Che::ALL {
        assert_eq!(v.to_string(), v.as_str());
    }
}

#[test]
fn no_vocabulary_has_a_duplicate_spelling() {
    // A copy-paste slip in a 17-arm `match` yields two variants with the same
    // string, and `parse` then silently answers the first. Every round-trip
    // test above still passes.
    fn distinct<T: Copy>(all: &[T], as_str: fn(T) -> &'static str, what: &str) {
        let mut seen: Vec<&str> = all.iter().map(|v| as_str(*v)).collect();
        seen.sort_unstable();
        let before = seen.len();
        seen.dedup();
        assert_eq!(
            before,
            seen.len(),
            "{what} has a duplicated wire spelling, so one variant is \
             unreachable through `parse`"
        );
    }
    distinct(&DeathsSting::ALL, DeathsSting::as_str, "DeathsSting");
    distinct(&Society::ALL, Society::as_str, "Society");
    distinct(&ResourceType::ALL, ResourceType::as_str, "ResourceType");
    distinct(&Warcry::ALL, Warcry::as_str, "Warcry");
    distinct(&PsmCategory::ALL, PsmCategory::as_str, "PsmCategory");
    distinct(&AccountType::ALL, AccountType::as_str, "AccountType");
    distinct(&Che::ALL, Che::as_str, "Che");
}
