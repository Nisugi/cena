//! Damage, roll and spell-loss lines, pinned against real messaging.
//!
//! Ported from `damage_defs_spec.rb` and `spell_loss_defs_spec.rb`, plus the
//! roll grammars against the real blob in `inventory/11` §3c.

use cena_model::state::chunks::ChunkLine;
use cena_model::{DamageLine, GameState, Resolution, ResolutionKind, SpellLoss};
use cena_protocol::Parser;

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

fn bolded(id: i64, noun: &str, name: &str) -> String {
    format!("<pushBold/><a exist=\"{id}\" noun=\"{noun}\">{name}</a><popBold/>")
}

// --- damage ------------------------------------------------------------------

#[test]
fn the_plain_endroll_damage_line() {
    let d =
        DamageLine::classify(&line_of("   ... and hit for 50 points of damage!")).expect("damage");
    assert_eq!(d.amount, 50);
    assert_eq!(d.list, "basic");
}

/// Chromatic Circle's element flavour line, generalised over the element:
/// the per-element list missed "arcs", so lightning 502 kills undercounted.
#[test]
fn every_environmental_verb_variant() {
    for wire in [
        "The crackling lightning quickly arcs around the gnarp, causing 35 points of damage!",
        "The whirlwind quickly swirls around a kobold, causing 21 points of damage!",
        "The shifting stones quickly orbit a rolton, causing 9 points of damage!",
        "The ice shards quickly orbit the gnarp, causing 12 points of damage!",
    ] {
        let d = DamageLine::classify(&line_of(wire)).unwrap_or_else(|| panic!("{wire}"));
        assert!(d.amount > 0, "{wire}");
    }
}

#[test]
fn a_non_damage_line_is_not_damage() {
    assert!(DamageLine::classify(&line_of("A kobold arrives.")).is_none());
}

/// The maneuver form has no `points of`, and its gate substring is ` hits!`.
#[test]
fn the_maneuver_hits_form() {
    let d = DamageLine::classify(&line_of("   ... 6 damage!")).expect("damage");
    assert_eq!(d.amount, 6);
}

// --- resolutions -------------------------------------------------------------

/// The AS/DS line from `attack.txt`, folded into the recorder's shape.
#[test]
fn an_as_ds_roll_folds_into_the_recorder_shape() {
    let r = Resolution::classify(&line_of(
        "  AS: +351 vs DS: +299 with AvD: +22 + d100 roll: +88 = +162",
    ))
    .expect("a roll");
    assert_eq!(r.kind, ResolutionKind::AsDs);
    assert_eq!(r.attacker_stat, Some(351));
    assert_eq!(r.defender_stat, Some(299));
    assert_eq!(r.modifier, Some(22));
    assert_eq!(r.roll, Some(88));
    assert_eq!(r.result, Some(162));
    assert_eq!(
        r.margin(),
        Some(74),
        "result - roll: the deterministic part"
    );
    assert!(!r.kind.precedes_its_line(), "AS/DS follows its attack line");
}

/// The real SMR from `inventory/11` §3c, the one that PRECEDES its attack.
#[test]
fn an_smr_roll_from_the_real_blob() {
    let r = Resolution::classify(&line_of("[SMR result: 191 (Open d100: 13, Bonus: 56)]"))
        .expect("a roll");
    assert_eq!(r.kind, ResolutionKind::Smr);
    assert_eq!(r.roll, Some(13));
    assert_eq!(r.bonus, Some(56));
    assert_eq!(r.result, Some(191));
    assert_eq!(r.attacker_stat, None);
    assert!(
        r.kind.precedes_its_line(),
        "maneuver rolls precede their line"
    );
}

#[test]
fn a_cs_td_roll_with_a_trailing_penalty() {
    let r = Resolution::classify(&line_of("  CS: 384 - TD: 419 + CvA: 13 + d100: 59 == 37"))
        .expect("a roll");
    assert_eq!(r.kind, ResolutionKind::CsTd);
    assert_eq!(r.attacker_stat, Some(384));
    assert_eq!(r.defender_stat, Some(419));
    assert_eq!(r.modifier, Some(13));
    assert_eq!(r.result, Some(37));
}

#[test]
fn a_uaf_udf_roll_keeps_its_fractional_total() {
    let r = Resolution::classify(&line_of(
        "  UAF: 681 vs UDF: 575 = 1.18 * MM: 110 + d100: 32 = 130",
    ))
    .expect("a roll");
    assert_eq!(r.kind, ResolutionKind::UafUdf);
    assert_eq!(r.attacker_stat, Some(681));
    assert_eq!(r.defender_stat, Some(575));
    assert_eq!(r.modifier, Some(110));
    assert_eq!(r.total, Some(1.18));
    assert_eq!(r.result, Some(130));
}

#[test]
fn a_fear_roll_is_its_own_grammar() {
    let r = Resolution::classify(&line_of(
        "  FS: 200 - FD: 150 + FvP: 10 + d100(L): 45 = 105",
    ))
    .expect("a roll");
    assert_eq!(r.kind, ResolutionKind::Fear);
    assert_eq!(r.attacker_stat, Some(200));
    assert_eq!(r.defender_stat, Some(150));
    assert_eq!(r.modifier, Some(10));
    assert_eq!(r.roll, Some(45));
}

#[test]
fn a_negative_smr_result_is_live() {
    // "Penalty term and negative results are both live (Rysk logs)".
    let r = Resolution::classify(&line_of("[SMR result: -12 (Open d100: 3, Penalty: 40)]"))
        .expect("a roll");
    assert_eq!(r.result, Some(-12));
    assert_eq!(r.penalty, Some(40));
}

// --- spell losses ------------------------------------------------------------

#[test]
fn empathic_focus_1109_off_a_creature_with_a_pronoun_link() {
    let line = line_of(&format!(
        "{} loses <a exist=\"452443346\" noun=\"psionicist\">its</a> focused look.",
        bolded(452_443_346, "psionicist", "An ethereal triton psionicist")
    ));
    let loss = SpellLoss::classify(&line).expect("a spell loss");
    assert_eq!(loss.spell, Some(1109));
    assert_eq!(loss.target.id, Some(452_443_346));
}

#[test]
fn empathic_focus_1109_off_a_player_in_plain_text() {
    let loss =
        SpellLoss::classify(&line_of("Nisugi loses his focused look.")).expect("a spell loss");
    assert_eq!(loss.spell, Some(1109));
    assert_eq!(loss.target.id, None);
    assert_eq!(loss.target.name, "Nisugi");
}

#[test]
fn strength_of_will_1119() {
    let line = line_of(&format!(
        "{} loses an aura of resolve.",
        bolded(98_732_276, "shield-maiden", "A brawny gigas shield-maiden")
    ));
    let loss = SpellLoss::classify(&line).expect("a spell loss");
    assert_eq!(loss.spell, Some(1119));
    assert_eq!(loss.spell_name, "Strength of Will");
    assert_eq!(loss.target.id, Some(98_732_276));
}

#[test]
fn intensity_1130() {
    let line = line_of(&format!(
        "{} loses an intense expression.",
        bolded(452_450_877, "wendigo", "A savage fork-tongued wendigo")
    ));
    let loss = SpellLoss::classify(&line).expect("a spell loss");
    assert_eq!(loss.spell, Some(1130));
    assert_eq!(loss.target.id, Some(452_450_877));
}

#[test]
fn foresight_1204_from_the_2020_ball_log() {
    let loss = SpellLoss::classify(&line_of(
        "Laehna takes a deep breath, blinking a couple of times before resuming a calm expression.",
    ))
    .expect("a spell loss");
    assert_eq!(loss.spell, Some(1204));
    assert_eq!(loss.target.name, "Laehna");
}

#[test]
fn spirit_warding_ii_107_in_both_observed_forms() {
    for wire in [
        "Deep blue motes swirl away from Laehna and fade.",
        "The deep blue glow leaves Tylanthriel.",
    ] {
        let loss = SpellLoss::classify(&line_of(wire)).unwrap_or_else(|| panic!("{wire}"));
        assert_eq!(loss.spell, Some(107), "{wire}");
    }
}

/// **The unpinned wear-off has no spell number, and says so.**
///
/// `spell_losses.rb:107`: the game's generic "appears somehow different"
/// line says a spell ended without saying which. Lich keeps it as
/// `spell: nil, spell_name: 'unknown'` rather than dropping the fact or
/// guessing a number; the extractor writes `unknown` and the classifier
/// reads `None`. Two rows, both kept -- a first cut of the loader dropped
/// them for having no name.
#[test]
fn the_generic_wear_off_has_no_spell_number() {
    for wire in [
        "A kobold appears somehow different.",
        "A kobold seems slightly different.",
    ] {
        let loss = SpellLoss::classify(&line_of(wire)).unwrap_or_else(|| panic!("{wire}"));
        assert_eq!(loss.spell, None, "{wire}: no number to pin it to");
        assert_eq!(loss.spell_name, "unknown");
        assert_eq!(loss.target.name, "A kobold");
    }
}
