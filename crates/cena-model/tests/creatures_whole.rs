//! The bestiary carries **everything a template says** (2026-09-24).
//!
//! > **AUTHOR, 2026-09-24:** *"The reason it was all there was multiple
//! > reasons, one of which is a comprehensive beastiary, the other is the
//! > messages have uses just because they haven't been made apparent yet."*
//!
//! The first port kept 4 of the 13 message kinds and a subset of the rest.
//! Every value below was read from `reference/lich-5/lib/gemstone/creatures/`
//! with Ruby, not from the TSVs this port generated, so a test here fails if
//! the extractor or the loader loses something the source has.

use cena_model::state::creature::{AttackCategory, MessageKind, Stat, creature, creatures};
use cena_model::state::creature_message::classify;

/// How many lines of one kind the whole bestiary carries.
fn lines_of(kind: MessageKind) -> usize {
    creatures().map(|c| c.messages_of(kind).len()).sum()
}

/// Every message kind, with its count MEASURED over the 627 templates.
#[test]
fn every_message_kind_is_carried() {
    for (kind, lines) in [
        (MessageKind::Death, 1094),
        (MessageKind::Flee, 1067),
        (MessageKind::Arrival, 991),
        (MessageKind::Decay, 710),
        (MessageKind::Description, 559),
        (MessageKind::Search, 101),
        (MessageKind::SpellPrep, 281),
        (MessageKind::Stand, 68),
        (MessageKind::StunBreak, 113),
        (MessageKind::Ambient, 7),
        (MessageKind::Attack, 1972),
        (MessageKind::Trigger, 162),
    ] {
        assert_eq!(lines_of(kind), lines, "{kind:?}");
    }
}

/// **`kswole.lic`'s case: the casting preparation, before the cast lands.**
///
/// `kswole.lic` keeps 134 hand-written regexes of these (`:104-296`). The
/// bestiary has 281 lines across 169 creatures, and `{target}` is a wildcard
/// here, so `arctic_titan`'s `gestures at {target}!` matches a real line.
#[test]
fn a_casting_preparation_is_recognised_before_the_cast() {
    let titan = creature("arctic_titan").expect("in the bestiary");
    let found = classify(
        titan,
        "An arctic titan gestures at you!",
        &[MessageKind::SpellPrep],
    )
    .expect("the prep line matches");
    assert_eq!(found.kind, MessageKind::SpellPrep);
    assert_eq!(
        creatures()
            .filter(|c| !c.messages_of(MessageKind::SpellPrep).is_empty())
            .count(),
        169
    );
}

/// **A trigger names what is coming**: the cause, before the symptom.
///
/// `gaudy_phantasmic_conjurer`'s `bind` is two lines -- its shout, then the
/// effect -- and either one says `bind`.
#[test]
fn a_trigger_names_the_effect_it_warns_of() {
    let conjurer = creature("gaudy_phantasmic_conjurer").expect("in the bestiary");
    for line in [
        "A gaudy phantasmic conjurer shouts out a single mystical syllable, thrusting its ghostly hands at you!",
        "An unseen force entangles you, restricting your movement!",
    ] {
        let found = classify(conjurer, line, &[MessageKind::Trigger]).expect("a bind line matches");
        assert_eq!(found.key.as_deref(), Some("bind"), "{line}");
    }
}

/// An attack line says which attack it was.
#[test]
fn an_attack_line_names_its_attack() {
    let bear = creature("agresh_bear").expect("in the bestiary");
    for (line, attack) in [
        ("An Agresh bear claws at you!", "claw"),
        ("An Agresh bear tries to bite you!", "bite"),
    ] {
        let found = classify(bear, line, &[MessageKind::Attack]).expect("matches");
        assert_eq!(
            (found.kind, found.key.as_deref()),
            (MessageKind::Attack, Some(attack))
        );
    }
    let titan = creature("arctic_titan").expect("in the bestiary");
    assert_eq!(
        classify(
            titan,
            "An arctic titan shakes off the stun!",
            &[MessageKind::StunBreak]
        )
        .map(|m| m.kind),
        Some(MessageKind::StunBreak)
    );
}

/// **Line breaks survive.** 26 descriptions and 13 tips are paragraphs or
/// bulleted lists, and 7 attack and trigger messages are two game lines.
/// Flattened to spaces, which is what the first port did, the bullets ran
/// together.
#[test]
fn a_line_break_in_the_source_is_kept() {
    let multi_line = creatures()
        .flat_map(|c| MessageKind::ALL.map(|k| c.messages_of(k)))
        .flatten()
        .filter(|m| m.text.contains('\n'))
        .count();
    assert_eq!(multi_line, 33);

    let golem = creature("behemothic_gorefrost_golem").expect("in the bestiary");
    let tip = golem.tips("wizard").first().expect("a wizard tip");
    assert!(tip.starts_with("* Golems are great targets for Mana Leech (516)"));
    assert!(tip.contains("\n* Open with Hand of Tonis"), "{tip}");
}

/// **Unknown treasure is `None`, not `false`.** The first port wrote an
/// unrecorded flag as `false`, telling a looter that 164 creatures carry no
/// boxes when nobody had looked.
#[test]
fn unrecorded_treasure_is_none_not_false() {
    let unknown = |f: fn(&cena_model::Treasure) -> Option<bool>| {
        creatures().filter(|c| f(&c.treasure).is_none()).count()
    };
    assert_eq!(unknown(|t| t.boxes), 164);
    assert_eq!(unknown(|t| t.coins), 71);
    assert_eq!(unknown(|t| t.gems), 121);
    assert_eq!(unknown(|t| t.magic_items), 176);

    // `skin: false` is "does not skin", and `skin: true` is "skins, into
    // something unrecorded" -- not skin NAMES, which is how they arrived.
    for id in [
        "behemothic_gorefrost_golem",
        "bloody_halfling_cannibal",
        "brawny_gigas_shield-maiden",
        "stunted_halfling_bloodspeaker",
        "tattooed_gigas_berserker",
    ] {
        let c = creature(id).expect("in the bestiary");
        assert_eq!(
            (c.treasure.skins, c.treasure.skin.as_deref()),
            (Some(false), None),
            "{id}"
        );
    }
    let warg = creature("niveous_giant_warg").expect("in the bestiary");
    assert_eq!(
        (warg.treasure.skins, warg.treasure.skin.as_deref()),
        (Some(true), None)
    );
    assert!(creatures().all(|c| !matches!(c.treasure.skin.as_deref(), Some("true" | "false"))));
}

/// All twelve target defenses, and the fields the first port shipped in
/// `creatures.tsv` and never read.
#[test]
fn every_target_defense_and_every_scalar_is_read() {
    let spider = creature("albino_tomb_spider").expect("in the bestiary");
    assert_eq!(spider.target_defense.get("mje_td"), Some(&Stat::Exact(24)));
    assert_eq!(spider.target_defense.get("mne_td"), Some(&Stat::Exact(24)));
    assert_eq!(
        spider.target_defense.get("mns_td"),
        Some(&Stat::Range(60, 66))
    );
    assert_eq!(spider.target_defense.get("mnm_td"), Some(&Stat::Exact(24)));
    assert_eq!(
        creatures()
            .filter(|c| c.target_defense.contains_key("mje_td"))
            .count(),
        491
    );

    assert_eq!(spider.height, Some(2));
    assert_eq!(spider.speed, Some(Stat::Exact(10)));
    assert_eq!(spider.bcs, Some(true));
    assert_eq!(spider.other_class, ["Living"]);
    assert!(
        spider
            .description()
            .is_some_and(|d| d.starts_with("Glowing an eerie, pale white, the tomb spider")),
        "{:?}",
        spider.description()
    );
}

/// Every attack category, with its count MEASURED over the 627 templates.
#[test]
fn every_attack_category_is_carried() {
    for (category, attacks) in [
        (AttackCategory::Physical, 1603),
        (AttackCategory::BoltSpell, 152),
        (AttackCategory::WardingSpell, 378),
        (AttackCategory::OffensiveSpell, 247),
        (AttackCategory::Maneuver, 641),
        (AttackCategory::SpecialAbility, 153),
    ] {
        let count = creatures()
            .flat_map(|c| &c.attacks)
            .filter(|a| a.category == category)
            .count();
        assert_eq!(count, attacks, "{category:?}");
    }

    // A warding spell carries a casting strength, not an attack strength.
    let occultist = creature("bony_tenthsworn_occultist").expect("in the bestiary");
    let burst = occultist
        .attacks
        .iter()
        .find(|a| a.name == "Blood Burst (701)")
        .expect("it casts Blood Burst");
    assert_eq!(
        (
            burst.category,
            burst.casting_strength,
            burst.attack_strength
        ),
        (
            AttackCategory::WardingSpell,
            Some(Stat::Range(274, 283)),
            None
        )
    );
}

/// The lists and the abilities.
#[test]
fn the_lists_and_the_abilities_are_carried() {
    let vortece = creature("dark_vortece").expect("in the bestiary");
    assert_eq!(vortece.immunities, ["magic"]);
    assert_eq!(creatures().map(|c| c.immunities.len()).sum::<usize>(), 18);
    assert_eq!(creatures().map(|c| c.equipment.len()).sum::<usize>(), 1068);

    let diviner = creature("branded_goliath_diviner").expect("in the bestiary");
    let impedance = diviner
        .abilities
        .iter()
        .find(|a| a.id == "mystic_impedance")
        .expect("it declares Mystic Impedance");
    assert_eq!(impedance.name, "Mystic Impedance (1708)");
    assert_eq!(
        (impedance.kind.as_deref(), impedance.target.as_deref()),
        (Some("debuff"), Some("opponent"))
    );
    assert_eq!(impedance.typical_duration_s, Some(30));
    assert_eq!(impedance.dispellable, None, "unrecorded");
    assert_eq!(
        impedance.effects,
        [(
            "blocks_spells_at_or_above".to_owned(),
            Some("15".to_owned())
        )]
    );
    // The ability's own note is the argument for carrying spell_prep.
    assert!(
        impedance
            .notes
            .as_deref()
            .is_some_and(|n| n.contains("spell_prep line")),
        "{:?}",
        impedance.notes
    );
}
