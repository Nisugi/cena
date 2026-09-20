//! The combat definition tables: they load, whole, in Lich's order.
//!
//! The counts are the extractor's, printed when the TSVs were cut:
//!
//! ```text
//! $ ruby crates/cena-model/tools/extract_combat_defs.rb \
//!       C:/Gemstone/lich-5/lib/gemstone/combat/defs crates/cena-model/data
//! combat_attacks.tsv: 376 rows
//! combat_results.tsv: 222 rows
//! combat_effects.tsv: 356 rows
//! total: 954 rows
//!   attack 362  outcome 178  flare 153  status 144  damage 34  spell_loss 18
//!   assault 16  sequence 15  resolution 9  attack_class 6  ucs 6 ...
//! patterns kept verbatim with markup=1 (hand-ported in Rust): 8
//! ```
//!
//! A regeneration that changes them changes these numbers, and that is the
//! point: the change gets looked at.

use cena_model::state::combat::defs::{HAND_PORTED, Role, defs};
use cena_model::state::combat::status::StatusName;
use cena_model::{AmbushKind, AssaultName, OutcomeKind, ResolutionKind, SequenceName};

/// Every table loads with the counts the extractor reported.
#[test]
fn every_family_loads_with_its_count() {
    let table = defs();
    let expected: &[(&str, usize)] = &[
        ("attack", 362),
        ("outcome", 178),
        ("flare", 153),
        ("status", 144),
        ("damage", 34),
        ("spell_loss", 18),
        ("assault", 16),
        ("sequence", 15),
        ("resolution", 9),
        ("attack_class", 6),
        ("ucs", 6),
        ("ambush_prefix", 5),
        ("ucs_tier", 3),
        ("coup_kill", 1),
        ("reaction_prefix", 1),
        ("redirect_prefix", 1),
        ("crit_rider", 1),
        ("flare_weapon_link", 1),
    ];
    for (family, count) in expected {
        assert_eq!(
            table.family(family).len(),
            *count,
            "{family}: the extractor wrote {count} rows"
        );
    }
    let total: usize = table.families().map(|f| table.family(f).len()).sum();
    assert_eq!(total, 954, "954 rows across the three files");
}

/// **Every pattern compiles under this crate's `regex`.**
///
/// `inventory/11` §2b measured exactly one def needing lookbehind
/// (`statuses.rb:118`), and `HAND_PORTED` re-expresses it as a positive
/// pattern plus a veto. A non-empty list here is a Lich update that added a
/// feature Rust lacks; the row is kept (`regex: None`) rather than dropped,
/// so the shortfall is this assertion and not a silent gap.
#[test]
fn every_pattern_compiles() {
    let failures = defs().compile_failures();
    assert!(
        failures.is_empty(),
        "{} patterns did not compile:\n{}",
        failures.len(),
        failures
            .iter()
            .map(|(f, n, p, e)| format!("  {f}/{n}: {p}\n    {e}"))
            .collect::<Vec<_>>()
            .join("\n")
    );
}

/// **Every tag-reading pattern has a hand-port, and every port lands.**
///
/// The extractor flagged eight; `HAND_PORTED` must name each by
/// `(family, order)`, plus the one lookbehind. An unported one is a pattern
/// silently missing from its family -- the shape of loss Rule 2.2 forbids.
/// A port at a wrong order silently replaces the WRONG row, which is why the
/// ported row's original text is checked too: it must have been a row the
/// extractor could not make plain.
#[test]
fn every_markup_pattern_is_hand_ported() {
    let unported = defs().unported();
    assert!(
        unported.is_empty(),
        "flagged rows with no entry in HAND_PORTED: {unported:?}"
    );
    assert_eq!(HAND_PORTED.len(), 9, "eight tag-readers and one lookbehind");
    for port in HAND_PORTED {
        let row = defs()
            .family(port.family)
            .iter()
            .find(|d| d.order == port.order)
            .unwrap_or_else(|| {
                panic!(
                    "HAND_PORTED names {}/{}, not a row",
                    port.family, port.order
                )
            });
        assert_eq!(row.pattern, port.pattern, "the port was substituted");
        assert_eq!(
            row.veto.is_some(),
            port.veto.is_some(),
            "{}/{}: the veto compiled",
            port.family,
            port.order
        );
    }
    let vetoed: Vec<&str> = defs()
        .families()
        .flat_map(|f| defs().family(f))
        .filter(|d| d.veto.is_some())
        .map(|d| d.name.as_str())
        .collect();
    assert_eq!(vetoed, ["kneeling"], "exactly one row carries a veto");
}

/// Order is preserved: each family's rows are 1..=n, ascending.
///
/// First-match-wins is only Lich's rule if the order is Lich's.
#[test]
fn every_family_is_in_lookup_order() {
    let table = defs();
    for family in table.families() {
        let orders: Vec<u32> = table.family(family).iter().map(|d| d.order).collect();
        let expected: Vec<u32> = (1..=orders.len())
            .filter_map(|i| u32::try_from(i).ok())
            .collect();
        assert_eq!(orders, expected, "{family} is not in lookup order");
    }
}

/// **Every closed vocabulary in the data parses into its enum.**
///
/// A Lich addition -- a new status, a new assault -- would otherwise classify
/// to `None` silently. This is where it goes red instead.
#[test]
fn every_named_kind_in_the_data_has_a_variant() {
    let table = defs();
    let check = |family: &str, parse: fn(&str) -> bool| {
        let unknown: Vec<&str> = table
            .family(family)
            .iter()
            .map(|d| d.name.as_str())
            .filter(|n| !parse(n))
            .collect();
        assert!(
            unknown.is_empty(),
            "{family} names with no variant: {unknown:?}"
        );
    };
    check("status", |n| StatusName::parse(n).is_some());
    check("resolution", |n| ResolutionKind::parse(n).is_some());
    check("outcome", |n| OutcomeKind::parse(n).is_some());
    check("assault", |n| AssaultName::parse(n).is_some());
    check("sequence", |n| SequenceName::parse(n).is_some());
    check("ambush_prefix", |n| AmbushKind::parse(n).is_some());
    check("reaction_prefix", |n| AmbushKind::parse(n).is_some());

    // And the enums hold nothing the data lacks.
    let statuses: std::collections::BTreeSet<&str> = table
        .family("status")
        .iter()
        .map(|d| d.name.as_str())
        .collect();
    for s in StatusName::ALL {
        // ...except the six only the `<crtrStatus>` feed and the crit tables
        // produce, which by construction no message def names.
        assert_eq!(
            statuses.contains(s.as_str()),
            !StatusName::FEED_ONLY.contains(&s),
            "{s:?}: a message status has rows in the data, a feed-only one has none"
        );
    }
}

/// The attack classification lists are what `attacks.rb:601-608` declares.
#[test]
fn the_attack_classes_match_lich() {
    let table = defs();
    let names = |role| table.attack_class(role).collect::<Vec<_>>();
    assert_eq!(names(Role::Environmental), ["frigid_wind"]);
    assert_eq!(names(Role::SelfInflicted), ["thorn_recoil"]);
    assert_eq!(
        names(Role::RoomTargeted),
        ["howl", "trumpet", "shrapnel_spray", "rift_tentacles"]
    );
}

/// **The damage gate is a correctness claim, and it holds.**
///
/// `damage.rb:104-106`: every damage pattern contains `damage` except the
/// maneuver `N hits!` form. The classifier skips the scan when neither
/// substring is present, so a pattern lacking both would be unreachable.
#[test]
fn every_damage_pattern_contains_a_gate_substring() {
    for d in defs().family("damage") {
        assert!(
            d.pattern.contains("damage") || d.pattern.contains(" hits!"),
            "damage/{}: {} contains neither gate substring",
            d.name,
            d.pattern
        );
    }
}

/// Every attack row records its assembly group.
#[test]
fn every_attack_has_a_group() {
    let groups: std::collections::BTreeMap<&str, usize> =
        defs()
            .family("attack")
            .iter()
            .fold(std::collections::BTreeMap::new(), |mut m, d| {
                *m.entry(d.extra("group").unwrap_or("")).or_default() += 1;
                m
            });
    assert!(
        !groups.contains_key(""),
        "attack rows with no group: {groups:?}"
    );
    assert_eq!(
        groups.get("environmental"),
        Some(&21),
        "the attackerless gate's group"
    );
    assert_eq!(groups.get("priority"), Some(&4));
}

/// Every flare row carries its three flags.
#[test]
fn every_flare_has_its_flags() {
    let table = defs();
    for d in table.family("flare") {
        for key in ["damaging", "aoe", "spawns"] {
            assert!(d.extra(key).is_some(), "flare/{}: no {key} flag", d.name);
        }
    }
    let spawning: Vec<&str> = table
        .family("flare")
        .iter()
        .filter(|d| d.flag("spawns"))
        .map(|d| d.name.as_str())
        .collect();
    assert!(spawning.contains(&"blink"), "blink spawns: {spawning:?}");
}
