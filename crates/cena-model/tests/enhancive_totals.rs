//! The enhancive classifier, against both captured report forms.
//!
//! M3 step 7. The fixtures are `enhancive_totals.xml` and
//! `enhancive_details.xml`, cut from a `--psm` capture the author ran on
//! 2026-09-19.
//!
//! # Why both forms
//!
//! They are not the same report with more words. MEASURED (`plan/15` §2b.4):
//! the details form attributes each bonus to the item granting it, and it
//! **inverts the martial line** from `name: +N ranks` to `+N Ranks name:
//! <item>`. Lich's `EnhanciveMartialSkill` matches only the first, so a
//! `TOTALS DETAILS` parse drops every martial enhancive. Testing one form
//! would have reproduced that.

use cena_model::{EnhanciveLine, EnhanciveTotals, Resource, Section, SkillKind, StatKind};
use cena_protocol::Parser;
use cena_protocol::frame::Frame;

/// The reassembled lines of a fixture.
fn lines_of(fixture: &str) -> Vec<String> {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../cena-protocol/tests/fixtures")
        .join(fixture);
    let bytes = std::fs::read(&path).unwrap_or_default();
    let mut parser = Parser::new();
    let mut frames = parser.push_bytes(&bytes);
    frames.extend(parser.push_bytes(b"\n"));

    let mut lines = Vec::new();
    let mut current = String::new();
    for frame in &frames {
        if let Frame::Text(text) = frame {
            current.push_str(&text.content);
            if text.ends_line {
                lines.push(std::mem::take(&mut current));
            }
        }
    }
    if !current.is_empty() {
        lines.push(current);
    }
    lines
}

fn read(fixture: &str) -> EnhanciveTotals {
    let lines = lines_of(fixture);
    let refs: Vec<&str> = lines.iter().map(String::as_str).collect();
    EnhanciveTotals::read(refs).unwrap_or_default()
}

/// All six sections appear, in the order the WIRE prints them.
///
/// **Not the order `parser.rb` declares.** Lich lists Martial before Spells;
/// the wire prints Spells first. A reader that assumed declaration order would
/// mis-section every martial row, which is the whole reason the section is a
/// parameter to the classifier rather than something it remembers.
#[test]
fn the_wire_order_is_not_lichs_declaration_order() {
    let order: Vec<Section> = lines_of("enhancive_totals.xml")
        .iter()
        .filter_map(|l| Section::classify(l))
        .collect();
    assert_eq!(
        order,
        vec![
            Section::Stats,
            Section::Skills,
            Section::Resources,
            Section::Spells,
            Section::Martial,
            Section::Statistics,
        ]
    );
}

/// Every section of the short form is read.
#[test]
fn the_short_form_reads_every_section() {
    let totals = read("enhancive_totals.xml");
    for section in Section::ALL {
        assert!(
            totals.saw_section(section),
            "{section:?} missing from the short form"
        );
    }

    assert_eq!(totals.stat(StatKind::Wisdom).map(|b| b.value), Some(15));
    assert_eq!(totals.stat(StatKind::Wisdom).map(|b| b.cap), Some(40));
    assert_eq!(
        totals
            .skill_bonus(SkillKind::TwoWeaponCombat)
            .map(|b| b.value),
        Some(10)
    );
    assert_eq!(
        totals.resource(Resource::MaxStamina).map(|b| b.value),
        Some(6)
    );
    assert_eq!(totals.spells(), &[215, 506, 515, 1109]);
    assert_eq!(totals.martial("Coup de Grace"), Some(2));
    assert_eq!(totals.statistic("Enhancive Items"), Some(6));
    assert_eq!(totals.statistic("Total Enhancive Amount"), Some(63));
}

/// **The details form yields the same facts, which Lich cannot manage.**
///
/// Its martial line is `+2 Ranks Coup de Grace: <item>`, which
/// `EnhanciveMartialSkill` (`parser.rb:123`) cannot match -- it requires
/// `name: +N ranks`. So Lich parsing a `TOTALS DETAILS` report silently loses
/// every martial enhancive. This asserts we do not.
#[test]
fn the_details_form_yields_the_same_facts() {
    let short = read("enhancive_totals.xml");
    let details = read("enhancive_details.xml");

    assert_eq!(details.stat(StatKind::Wisdom), short.stat(StatKind::Wisdom));
    assert_eq!(
        details.skill_bonus(SkillKind::SpiritualLoreSummoning),
        short.skill_bonus(SkillKind::SpiritualLoreSummoning)
    );
    assert_eq!(
        details.resource(Resource::MaxStamina),
        short.resource(Resource::MaxStamina)
    );
    assert_eq!(details.spells(), short.spells());
    assert_eq!(
        details.martial("Coup de Grace"),
        Some(2),
        "the inverted martial line is the one Lich drops"
    );
    assert_eq!(
        details.statistic("Total Enhancive Amount"),
        short.statistic("Total Enhancive Amount")
    );
}

/// Both martial line shapes classify to the same fact.
#[test]
fn both_martial_forms_agree() {
    let short = EnhanciveLine::classify("  Coup de Grace: +2 ranks", Section::Martial);
    let details = EnhanciveLine::classify(
        "  +2 Ranks Coup de Grace: a pallid jade green dragonfly tattoo",
        Section::Martial,
    );
    let want = Some(EnhanciveLine::Martial {
        name: "Coup de Grace".to_owned(),
        ranks: 2,
    });
    assert_eq!(short, want);
    assert_eq!(details, want, "the details form must not be dropped");
}

/// The details form's item-attribution lines are not bonuses.
///
/// `+10: a veniom-bound witchwood badge` carries no bonus of its own -- it
/// explains the one above it. Classifying it as a bonus would double-count
/// every enhancive in the report.
#[test]
fn item_attribution_lines_are_not_bonuses() {
    for (line, section) in [
        ("    +10: a veniom-bound witchwood badge", Section::Stats),
        ("    +5: a gilded locus", Section::Skills),
        ("    +6: a gilded locus", Section::Resources),
    ] {
        assert_eq!(
            EnhanciveLine::classify(line, section),
            None,
            "{line:?} is an attribution, not a bonus"
        );
    }
}

/// A skill's display name survives spaces and hyphens.
///
/// `Spiritual Lore - Blessings Bonus: 1/50` -- the split is on the ` Bonus: `
/// keyword, not on whitespace, because the name contains both.
#[test]
fn a_hyphenated_skill_name_classifies() {
    let line = "  Spiritual Lore - Blessings Bonus:  1/50";
    let Some(EnhanciveLine::SkillBonus { kind, bonus }) =
        EnhanciveLine::classify(line, Section::Skills)
    else {
        panic!("should classify as a skill bonus");
    };
    assert_eq!(kind, SkillKind::SpiritualLoreBlessings);
    assert_eq!(bonus.value, 1);
    assert_eq!(bonus.cap, 50);
}

/// Bonus and Ranks are different facts about the same skill.
#[test]
fn skill_bonus_and_ranks_do_not_collide() {
    let bonus = EnhanciveLine::classify("  Ambush Bonus: 10/50", Section::Skills);
    let ranks = EnhanciveLine::classify("  Ambush Ranks: 3/50", Section::Skills);
    assert!(matches!(bonus, Some(EnhanciveLine::SkillBonus { .. })));
    assert!(matches!(ranks, Some(EnhanciveLine::SkillRanks { .. })));

    let mut totals = EnhanciveTotals::default();
    if let (Some(b), Some(r)) = (bonus, ranks) {
        totals.apply(&b);
        totals.apply(&r);
    }
    assert_eq!(
        totals.skill_bonus(SkillKind::Ambush).map(|b| b.value),
        Some(10)
    );
    assert_eq!(
        totals.skill_ranks(SkillKind::Ambush).map(|b| b.value),
        Some(3)
    );
}

/// A line is classified against the section it arrived in, not guessed at.
///
/// The same text means different things under different headers, and this is
/// what `plan/12` §3a's "the consumer owns what spans lines" buys: the
/// classifier cannot mis-section a row because it is never asked to decide.
#[test]
fn the_section_decides_the_meaning() {
    let line = "  Max Stamina: 6/300";
    assert!(matches!(
        EnhanciveLine::classify(line, Section::Resources),
        Some(EnhanciveLine::Resource { .. })
    ));
    assert_eq!(
        EnhanciveLine::classify(line, Section::Stats),
        None,
        "a resource line is not a stat, even though both are `name: n/m`"
    );
}

/// Headers are recognised, and only real ones.
#[test]
fn headers_classify_and_prose_does_not() {
    assert_eq!(Section::classify("Stats:"), Some(Section::Stats));
    assert_eq!(
        Section::classify("Martial Knowledge Skills:"),
        Some(Section::Martial)
    );
    // The trailer, and a player talking.
    for line in [
        "For more details, see INVENTORY ENHANCIVE TOTALS DETAILS.",
        "Stats",
        "My Stats:",
        "",
    ] {
        assert_eq!(
            Section::classify(line),
            None,
            "should not be a header: {line}"
        );
    }
}

/// `Default` IS the zeroing Lich needs 191 explicit writes for.
///
/// `enhancive.rb:368`: *"Resets all enhancive values to 0/empty // Critical
/// because game output only shows non-zero values."* A fresh `EnhanciveTotals`
/// reports `None` everywhere, so a removed item's bonus cannot survive into the
/// next report.
#[test]
fn a_fresh_report_carries_nothing_from_the_last() {
    let first = read("enhancive_totals.xml");
    assert_eq!(
        first.stat(StatKind::Wisdom).map(|b| b.value),
        Some(15),
        "guard"
    );

    // The next report, from a character wearing nothing: one header, no rows.
    let second = EnhanciveTotals::read(["Stats:"]).unwrap_or_default();
    assert!(second.saw_section(Section::Stats), "the section was read");
    assert_eq!(
        second.stat(StatKind::Wisdom),
        None,
        "the previous report's bonus must not survive"
    );
}

/// A section header with no rows is a real answer.
///
/// The clique means absence is normal: a character with no enhancive skills has
/// no `Skills:` section at all. "Section absent" and "section present but
/// empty" are different facts, and `saw_section` is what tells them apart --
/// the MO-3 distinction again.
#[test]
fn an_empty_section_differs_from_an_absent_one() {
    let totals = EnhanciveTotals::read(["Stats:", "  Wisdom (WIS): 15/40", "Resources:"])
        .unwrap_or_default();
    assert!(totals.saw_section(Section::Stats));
    assert!(
        totals.saw_section(Section::Resources),
        "present but empty is still present"
    );
    assert!(
        !totals.saw_section(Section::Martial),
        "absent is a different answer"
    );
    assert_eq!(totals.resource(Resource::MaxMana), None);
}

/// A chunk with no section header is not a report.
///
/// `state.rs:306-309`'s rule: a player can say anything, including something
/// shaped like a stat line.
#[test]
fn prose_is_not_a_report() {
    assert_eq!(
        EnhanciveTotals::read(["Ashryn says, \"Wisdom (WIS): 15/40\""]),
        None
    );
    assert_eq!(EnhanciveTotals::read(["", "  +10: a badge"]), None);
}

/// Sections may arrive in any order, which is what "clique" means.
#[test]
fn sections_may_arrive_in_any_order() {
    let totals = EnhanciveTotals::read([
        "Statistics:",
        "  Enhancive Items: 2",
        "Stats:",
        "  Wisdom (WIS): 15/40",
    ])
    .unwrap_or_default();
    assert_eq!(totals.statistic("Enhancive Items"), Some(2));
    assert_eq!(totals.stat(StatKind::Wisdom).map(|b| b.value), Some(15));
}

/// An unrecognised statistic name is refused, not stored.
///
/// The three names are a closed vocabulary (`parser.rb:124`). Storing a fourth
/// would put a key in the map that no accessor names.
#[test]
fn an_unknown_statistic_is_refused() {
    assert_eq!(
        EnhanciveLine::classify("  Enhancive Sparkles: 4", Section::Statistics),
        None
    );
}

/// The report, folded by the model where the game sends it: nothing called
/// `EnhanciveTotals::read` outside this file before 2026-09-26, so a live
/// session never held its enhancives.
#[test]
fn the_report_fills_the_character_and_marks_it_taught() {
    use cena_model::state::character::snapshot::Group;
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../cena-protocol/tests/fixtures/enhancive_totals.xml");
    let bytes = std::fs::read(path).unwrap_or_default();
    let mut state = cena_model::GameState::default();
    let mut parser = Parser::new();
    for frame in parser.push_bytes(&bytes) {
        state.apply(&frame);
    }
    let held = &state.character.enhancives;
    assert_eq!(
        held.skill_bonus(SkillKind::TwoWeaponCombat)
            .map(|b| b.value),
        Some(10)
    );
    assert_eq!(held.spells(), &[215, 506, 515, 1109]);
    assert!(state.character.take_taught().contains(&Group::Enhancives));

    // Another chunk is no report: what is held stays.
    for frame in
        parser.push_bytes(b"You glance around.\n<prompt time=\"1789873290\">&gt;</prompt>\n")
    {
        state.apply(&frame);
    }
    assert_eq!(state.character.enhancives.spells(), &[215, 506, 515, 1109]);
}
