//! Attack initiations, pinned against real game messaging.
//!
//! Ported from `spec/lib/gemstone/combat/attack_defs_spec.rb`, whose cases are
//! *"lines lifted from GSIV session logs, XML links intact where the game
//! sends them"*. Each line is fed through the real parser, so the link and
//! bold facts the classifier reads are the parser's, not a struct literal's.

use cena_model::state::chunks::ChunkLine;
use cena_model::{AttackLine, GameState, Outcome, OutcomeKind, TargetKind, UcsLine};
use cena_model::{FlareLine, PositionTier};
use cena_protocol::Parser;

/// One wire line, through the parser, as the chunk would hold it.
fn line_of(wire: &str) -> ChunkLine {
    let mut parser = Parser::new();
    let mut state = GameState::default();
    for frame in parser.push_bytes(format!("{wire}\n").as_bytes()) {
        state.apply(&frame);
    }
    state
        .open_chunk()
        .lines()
        .first()
        .cloned()
        .unwrap_or_default()
}

/// The spec's `bolded(id, noun, name)`: a creature link as the game wraps it.
fn bolded(id: i64, noun: &str, name: &str) -> String {
    format!("<pushBold/><a exist=\"{id}\" noun=\"{noun}\">{name}</a><popBold/>")
}

fn target_id(attack: &AttackLine) -> Option<i64> {
    match &attack.target {
        TargetKind::Creature(a) => a.id,
        _ => None,
    }
}

#[test]
fn the_summoned_briar_dragging_its_victim_to_the_ground() {
    let line = line_of(&format!(
        "The lashing emerald briar lashes out violently at {}, dragging it to the ground!",
        bolded(452_443_346, "warg", "a niveous giant warg")
    ));
    let attack = AttackLine::classify(&line).expect("an attack");
    assert_eq!(attack.name, "tangleweed");
    assert_eq!(target_id(&attack), Some(452_443_346));
}

#[test]
fn the_briar_to_the_floor_variant_from_2026_01_logs() {
    let line = line_of(&format!(
        "The lashing emerald briar lashes out violently at {}, dragging it to the floor!",
        bolded(452_443_346, "warg", "a niveous giant warg")
    ));
    let attack = AttackLine::classify(&line).expect("an attack");
    assert_eq!(attack.name, "tangleweed");
    assert_eq!(target_id(&attack), Some(452_443_346));
}

#[test]
fn the_briar_entangle_variants() {
    for (id, tail) in [
        (
            452_440_152,
            "wraps itself around its body and entangles it on the ground.",
        ),
        (
            121_654_846,
            "wraps itself around her body and entangles her on the floor.",
        ),
    ] {
        let line = line_of(&format!(
            "The lashing emerald briar lashes out at {}, {tail}",
            bolded(id, "mastodon", "a heavily armored battle mastodon")
        ));
        let attack = AttackLine::classify(&line).expect("an attack");
        assert_eq!(attack.name, "tangleweed");
        assert_eq!(target_id(&attack), Some(id));
    }
}

/// Environmental / self-inflicted damage: no attacker, no `you` capture, yet
/// the damage is ours to take (real-feed 2026-09-07).
#[test]
fn a_frigid_wind_cold_tick_is_inbound_damage_to_us() {
    let line = line_of("The burn of the cold tears precious warmth from your flesh.");
    let attack = AttackLine::classify(&line).expect("an attack");
    assert_eq!(attack.name, "frigid_wind");
    assert!(attack.inbound);
    assert_eq!(attack.target, TargetKind::None);
    assert_eq!(
        attack.attacker.as_ref().map(|a| a.name.as_str()),
        Some("environment"),
        "the world did it -- distinguishable from our own gear in reports"
    );
}

#[test]
fn a_nearby_player_taking_the_cold_tick_is_a_foreign_target_not_inbound() {
    let line = line_of("Onkel shivers as the cold settles into his flesh.");
    let attack = AttackLine::classify(&line).expect("an attack");
    assert_eq!(attack.name, "frigid_wind");
    assert!(!attack.inbound);
    assert_eq!(attack.target, TargetKind::Foreign("Onkel".to_owned()));
}

#[test]
fn environmental_tick_lines_pass_the_attackerless_gate() {
    use cena_model::state::combat::attack::attackerless_line;
    assert!(attackerless_line(&line_of(
        "Bitter cold leaches warmth from your skin."
    )));
    assert!(!attackerless_line(&line_of("You feel more refreshed.")));
}

#[test]
fn the_thorn_bow_recoil_is_inbound_damage_to_us() {
    let line = line_of(
        "As a darkened ruic longbow etched with thorns leaves your left hand, the thorns embedded in your skin painfully rip away, vines quickly retreating.  A single vine thwaps your left hand as it returns to the longbow.",
    );
    let attack = AttackLine::classify(&line).expect("an attack");
    assert_eq!(attack.name, "thorn_recoil");
    assert!(attack.inbound);
    assert_eq!(
        attack.attacker.as_ref().map(|a| a.name.as_str()),
        Some("self"),
        "our own gear did it"
    );
}

#[test]
fn the_classic_and_dark_ewave_messaging() {
    for phrase in [
        "churning ethereal waves",
        "formless black waves",
        "formless black sphere",
    ] {
        let line = line_of(&format!(
            "{} is buffeted by the {phrase} and is knocked to the ground.",
            bolded(98_732_276, "shield-maiden", "A brawny gigas shield-maiden")
        ));
        let attack = AttackLine::classify(&line).expect(phrase);
        assert_eq!(attack.name, "ewave", "{phrase}");
    }
}

#[test]
fn bane_302_living_target_messaging() {
    let line = line_of(&format!(
        "A sickly, violet haze encompasses {}.",
        bolded(452_450_877, "mastodon", "a heavily armored battle mastodon")
    ));
    let attack = AttackLine::classify(&line).expect("an attack");
    assert_eq!(attack.name, "bane");
    assert_eq!(target_id(&attack), Some(452_450_877));
}

/// 335 Divine Wrath: four of its ~35 deity forms, including pronoun tails
/// that name the target once and then say `her`.
#[test]
fn divine_wrath_335_forms() {
    let cases = [
        format!(
            "A shadowy figure briefly materializes behind {}, and a silent scream courses over a tattooed gigas berserker's visage.",
            bolded(452_450_877, "berserker", "a tattooed gigas berserker")
        ),
        format!(
            "A shadowy black rose touches {} and wraps immediately about it, struggling to force its long, barbed thorns into it.",
            bolded(452_450_877, "berserker", "a tattooed gigas berserker")
        ),
        format!(
            "As {} comes too close to a tendril of black mist, the mist suddenly expands into a large black cloud, which rapidly surrounds her, obliterating her from view.",
            bolded(452_450_877, "conjurer", "a gaudy phantasmic conjurer")
        ),
        format!(
            "A shadowy black rose touches {} and wraps immediately about it, struggling to force its long, barbed thorns into the tattooed gigas berserker.",
            bolded(452_450_877, "berserker", "a tattooed gigas berserker")
        ),
    ];
    for wire in &cases {
        let attack = AttackLine::classify(&line_of(wire)).expect(wire);
        assert_eq!(attack.name, "divine_wrath", "{wire}");
        assert_eq!(target_id(&attack), Some(452_450_877), "{wire}");
    }
}

/// *"Bloodstained light" fires identically for ANY caster's spell*, so it must
/// not be parsed as one of our attacks.
#[test]
fn ambient_spell_messaging_with_no_caster_is_not_claimed() {
    let line = line_of(&format!(
        "Bloodstained light spills down from the heavens in an undulating deluge, bathing {}'s form in a cascade of transcendent power!",
        bolded(416_226_445, "skald", "a grim gigas skald")
    ));
    assert!(AttackLine::classify(&line).is_none());
}

// Inbound attacks (creature -> us). The only creature link on such a line is
// the ATTACKER; before this, Lich's line-scan fallback installed it as its
// own target and applied the damage it dealt US to IT -- 268 self-attributed
// attacks across the log archive.

#[test]
fn a_swing_at_us_never_resolves_the_attacker_as_its_own_target() {
    let line = line_of(&format!(
        "{} swings a dagger at you!",
        bolded(31_038_708, "champion", "A muscular tattooed champion")
    ));
    let attack = AttackLine::classify(&line).expect("an attack");
    assert!(attack.inbound);
    assert_eq!(attack.target, TargetKind::None);
    assert_eq!(
        attack.attacker.as_ref().and_then(|a| a.id),
        Some(31_038_708)
    );
}

#[test]
fn a_natural_weapon_attack_against_us_is_inbound() {
    let line = line_of(&format!(
        "{} claws at you!",
        bolded(22_764_224, "grahnk", "A burly grahnk")
    ));
    let attack = AttackLine::classify(&line).expect("an attack");
    assert!(attack.inbound);
    assert_eq!(attack.target, TargetKind::None);
}

/// Defs that name us in the pattern LITERAL, not a capture: an attacker
/// capture and no target capture, which also fell through to the line scan.
#[test]
fn an_inbound_def_that_names_us_in_its_literal_is_inbound() {
    let line = line_of(&format!(
        "{} springs from the shadows and strikes at you!",
        bolded(555, "thing", "A shadowy thing")
    ));
    let attack = AttackLine::classify(&line).expect("an attack");
    assert!(attack.inbound);
    assert_eq!(attack.target, TargetKind::None);
}

#[test]
fn our_own_outbound_attack_still_resolves_a_creature_target() {
    let line = line_of(&format!(
        "You swing a kelyn-edged slim short sword at {}!",
        bolded(4242, "orc", "a greater orc")
    ));
    let attack = AttackLine::classify(&line).expect("an attack");
    assert!(!attack.inbound);
    assert_eq!(target_id(&attack), Some(4242));
    // The generic swing def captures no weapon; Lich reads it separately
    // (`parse_swing_weapon`) to claim pre-flares by weapon.
    assert_eq!(attack.weapon, None, "the def itself names no weapon");
    // Lich's pattern consumes the article: `(?:an? |your |some )?` precedes
    // the capture, so "a kelyn-edged..." reads as "kelyn-edged...".
    assert_eq!(
        cena_model::state::combat::attack::swing_weapon(&line).as_deref(),
        Some("kelyn-edged slim short sword")
    );
}

#[test]
fn the_swing_weapon_reader_handles_aim_fire_and_articles() {
    use cena_model::state::combat::attack::swing_weapon;
    let cases = [
        (
            format!(
                "You take aim and fire a faewood arrow at {}!",
                bolded(1, "kobold", "a kobold")
            ),
            "faewood arrow",
        ),
        (
            format!(
                "You swing your perfect mithril war-hammer at {}!",
                bolded(1, "kobold", "a kobold")
            ),
            "perfect mithril war-hammer",
        ),
    ];
    for (wire, weapon) in &cases {
        assert_eq!(
            swing_weapon(&line_of(wire)).as_deref(),
            Some(*weapon),
            "{wire}"
        );
    }
    assert_eq!(swing_weapon(&line_of("A kobold arrives.")), None);
}

#[test]
fn a_creature_attacking_another_creature_resolves_both() {
    let line = line_of(&format!(
        "{} swings a club at {}!",
        bolded(7, "ogre", "An ogre"),
        bolded(200, "guard", "a guard")
    ));
    let attack = AttackLine::classify(&line).expect("an attack");
    assert!(!attack.inbound);
    assert_eq!(target_id(&attack), Some(200));
    assert_eq!(attack.attacker.as_ref().and_then(|a| a.id), Some(7));
}

/// `:tremors` has an attacker capture and no target capture, so the line scan
/// returned the attacker. Nothing can attack itself.
#[test]
fn a_self_referential_target_on_an_untargeted_aoe_is_dropped() {
    let line = line_of(&format!(
        "{} slams a gigantic foot down, sending tremors rippling outward from the point of impact!",
        bolded(22_219_124, "mastodon", "A heavily armored battle mastodon")
    ));
    let attack = AttackLine::classify(&line).expect("an attack");
    assert_eq!(target_id(&attack), None);
}

#[test]
fn positioning_strike_inbound_form_is_an_attack_on_us() {
    let line = line_of(&format!(
        "{} positions <a exist=\"98732276\" noun=\"shield-maiden\">herself</a> to attack you.",
        bolded(98_732_276, "shield-maiden", "A brawny gigas shield-maiden")
    ));
    let attack = AttackLine::classify(&line).expect("an attack");
    assert_eq!(attack.name, "positioning_strike");
    assert!(attack.inbound);
}

#[test]
fn positioning_strike_third_party_form_is_a_foreign_target() {
    let line = line_of(&format!(
        "{} positions <a exist=\"98732276\" noun=\"berserker\">himself</a> to attack Dicate.",
        bolded(98_732_276, "berserker", "A tattooed gigas berserker")
    ));
    let attack = AttackLine::classify(&line).expect("an attack");
    assert_eq!(attack.name, "positioning_strike");
    assert!(
        matches!(attack.target, TargetKind::Foreign(_)),
        "{:?}",
        attack.target
    );
}

// UCS inbound positioning -- `parser_inbound_spec`'s cases for the hand-ported
// tag-reading patterns.

#[test]
fn the_creature_tier_against_us_line_parses() {
    let line = line_of(&format!(
        "{} has decent positioning against you.",
        bolded(452_443_346, "brawler", "The triton brawler")
    ));
    let Some(UcsLine::PositionInbound { tier, attacker }) = UcsLine::classify(&line) else {
        panic!("a position_inbound line");
    };
    assert_eq!(tier, PositionTier::Decent);
    assert_eq!(tier.ordinal(), 1);
    assert_eq!(attacker.and_then(|a| a.id), Some(452_443_346));
}

#[test]
fn our_outbound_positioning_is_not_confused_with_theirs() {
    let line = line_of(&format!(
        "You have good positioning against {}.",
        bolded(452_443_346, "kobold", "a kobold")
    ));
    let Some(UcsLine::Position { tier, target }) = UcsLine::classify(&line) else {
        panic!("a position line");
    };
    assert_eq!(tier, PositionTier::Good);
    assert_eq!(tier.ordinal(), 2);
    assert_eq!(target.and_then(|a| a.id), Some(452_443_346));
}

#[test]
fn every_positioning_word_maps_to_an_ordinal_tier() {
    let tiers: Vec<(&str, u8)> = PositionTier::ALL
        .into_iter()
        .map(|t| (t.as_str(), t.ordinal()))
        .collect();
    assert_eq!(tiers, [("decent", 1), ("good", 2), ("excellent", 3)]);
}

/// 1106 Bone Shatter's result rider, across severity grades.
#[test]
fn the_bone_shatter_convulsions_rider_is_a_hit() {
    for grade in ["mild", "moderate", "severe"] {
        let line = line_of(&format!(
            "The gigas berserker shudders with {grade} convulsions as pearlescent ripples envelop his body."
        ));
        assert_eq!(
            Outcome::classify(&line).map(|o| o.kind),
            Some(OutcomeKind::Hit),
            "{grade}"
        );
    }
}

/// The dispel flux crit, across all three sphere nouns -- and not the
/// no-flux strip lines.
#[test]
fn dispel_flux_matches_all_three_sphere_nouns() {
    for noun in ["elemental aura", "hazy film", "murky veil"] {
        let line = line_of(&format!(
            "The {noun} around {} fluxes chaotically!",
            bolded(452_450_877, "taint", "a festering taint")
        ));
        assert_eq!(
            FlareLine::classify(&line).map(|f| f.name),
            Some("dispel_flux".to_owned()),
            "{noun}"
        );
    }
    for wire in [
        "The elemental aura around a festering taint wavers.",
        "A hazy film coats a festering taint.",
        "A murky veil surrounds an Ithzir seer.",
    ] {
        assert_ne!(
            FlareLine::classify(&line_of(wire)).map(|f| f.name),
            Some("dispel_flux".to_owned()),
            "unexpected claim of: {wire}"
        );
    }
}

// The three re-scanning bugs `state/chunks.rs` records, as guards on the
// link-position rules that replace the re-scan.

/// **The attacker is the LAST link in its capture.**
///
/// `parser.rb:196-214`: a flavour prefix can carry the attacker's own pronoun
/// link first -- *"Froth bubbling on `<his>` lips, `<a tattooed gigas
/// berserker>` swings..."* -- and taking the first recorded the attacker as
/// "his" (hunt log 2026-09-07). Same id either way; the NAME is what the bug
/// got wrong.
#[test]
fn the_attacker_is_the_last_link_in_its_capture() {
    let line = line_of(&format!(
        "Froth bubbling on <a exist=\"88\" noun=\"berserker\">his</a> lips, {} swings a club at you!",
        bolded(88, "berserker", "a tattooed gigas berserker")
    ));
    let attack = AttackLine::classify(&line).expect("an attack");
    assert!(attack.inbound);
    let attacker = attack.attacker.expect("an attacker");
    assert_eq!(attacker.id, Some(88));
    assert_eq!(attacker.name, "a tattooed gigas berserker", "not `his`");
}

/// **A possessive INSIDE the link still resolves the target.**
///
/// `parser.rb:198-205`, `OPEN_LINK_TAIL_PATTERN`: the game writes
/// `<a>grim gigas skald's</a> open wound`, so a `(?<target>.+?)'s` capture
/// ends at the apostrophe, inside the link, and Lich's first pass found no
/// link at all -- *"10 bleed ticks on named creatures, all unattributed"*
/// (Rysk logs 2026-09-11). A byte span that ends inside a run still overlaps
/// it, so here it is the same lookup.
#[test]
fn a_possessive_inside_the_link_still_resolves() {
    let line = line_of(&format!(
        "Blood weeps from {} open chest wound.",
        bolded(340_826_187, "skald", "grim gigas skald's")
    ));
    let attack = AttackLine::classify(&line).expect("an attack");
    assert_eq!(attack.name, "bleed");
    let TargetKind::Creature(target) = &attack.target else {
        panic!("{:?}", attack.target)
    };
    assert_eq!(target.id, Some(340_826_187));
    assert_eq!(
        target.name, "grim gigas skald",
        "the possessive is stripped from the name"
    );
}

/// **Only a BOLDED link is a line-scan target.**
///
/// `parser.rb:325-343`: *"Non-bolded links are equipment, objects, or other
/// non-combatants."* A flurry round line names our own blade as an unbolded
/// link and captures nothing; the line scan must not make the blade the
/// target.
#[test]
fn an_unbolded_equipment_link_is_not_a_target() {
    let line = line_of(
        "Flowing with deadly grace, you smoothly reverse the direction of your <a exist=\"5\" noun=\"blade\">blade</a> and slash again!",
    );
    let attack = AttackLine::classify(&line).expect("an attack");
    assert_eq!(attack.name, "flurry");
    assert_eq!(
        attack.target,
        TargetKind::None,
        "our own blade is equipment, not a target"
    );
}
