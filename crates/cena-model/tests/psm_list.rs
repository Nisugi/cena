//! The PSM classifier, against a real `cman list` table.
//!
//! M3 step 6. The fixture is `psm_list.xml`, cut from
//! `GSIV-Nisugi/2025/03/xml/2025-03-20_06-02-48.xml:24722-24762` and scrubbed.
//!
//! # What this fixture has that no other does
//!
//! **Bold spans that wrap across rows.** The wire emits
//! `<popBold/><pushBold/>` mid-line, so one span closes and the next opens
//! inside a single row. A consumer resetting bold at `ends_line` would
//! mis-attribute every row after the first, and no hand-written table would
//! have reproduced that.

use cena_model::{PsmCategory, PsmLine, PsmSet};
use cena_protocol::Parser;
use cena_protocol::frame::Frame;

/// The reassembled rows of the fixture, each with whether it arrived bolded.
fn psm_rows() -> Vec<(String, bool)> {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../cena-protocol/tests/fixtures/psm_list.xml");
    let bytes = std::fs::read(&path).unwrap_or_default();
    let mut parser = Parser::new();
    let mut frames = parser.push_bytes(&bytes);
    frames.extend(parser.push_bytes(b"\n"));

    let mut lines = Vec::new();
    let mut current = String::new();
    let mut bolded = false;
    for frame in &frames {
        if let Frame::Text(text) = frame {
            current.push_str(&text.content);
            if text.style.bold_depth > 0 && !text.content.trim().is_empty() {
                bolded = true;
            }
            if text.ends_line {
                lines.push((std::mem::take(&mut current), bolded));
                bolded = false;
            }
        }
    }
    if !current.is_empty() {
        lines.push((current, bolded));
    }
    lines
}

/// Bold marks KNOWN, not maxed.
///
/// **The measurement that shapes the module.** Every bolded row has ranks > 0;
/// every unbolded row is exactly `0/n`. `Hamstring 4/5` is bolded and not
/// maxed, which is what rules out "bold = maxed" -- a reading that would have
/// looked right against the four `5/5` rows alone.
#[test]
fn bold_means_known_not_maxed() {
    let mut bolded_unmaxed = 0;
    for (line, bolded) in psm_rows() {
        let Some(row) = PsmLine::classify_with_bold(&line, bolded) else {
            continue;
        };
        if bolded {
            assert!(
                row.ranks.is_known(),
                "bolded row must have ranks > 0: {line}"
            );
            if !row.ranks.is_maxed() {
                bolded_unmaxed += 1;
            }
        } else {
            assert_eq!(row.ranks.ranks, 0, "unbolded row must be 0/n: {line}");
        }
    }
    assert!(
        bolded_unmaxed >= 1,
        "the capture must contain a bolded, un-maxed row, or this test proves nothing"
    );
}

/// The two known-signals never disagree in the capture.
///
/// Bold and the `x/y` fraction are independent readings of the same fact.
/// Asserting they agree is what makes `PsmRanks::disagrees` meaningful: if a
/// future table breaks this, the port surfaces it rather than silently
/// preferring one.
#[test]
fn bold_and_the_fraction_agree() {
    for (line, bolded) in psm_rows() {
        let Some(row) = PsmLine::classify_with_bold(&line, bolded) else {
            continue;
        };
        assert!(!row.ranks.disagrees(), "signals disagree on: {line}");
    }
}

/// The capture's 27 rows all classify, and the furniture does not.
///
/// **27, MEASURED** -- `grep -oE "[0-9]+/[0-9]+" | wc -l`. This first said 29,
/// counted by eye off a probe's output that included the header and the rule.
/// The classifier was right and the assertion was wrong, which is `plan/05`
/// §-2's rule arriving in its most ordinary form: a number restated from
/// memory rather than measured.
#[test]
fn the_table_yields_every_row_and_no_furniture() {
    let rows: Vec<PsmLine> = psm_rows()
        .iter()
        .filter_map(|(l, b)| PsmLine::classify_with_bold(l, *b))
        .collect();
    assert_eq!(rows.len(), 27, "the capture has 27 maneuvers");

    // Spot-check the ends, so an off-by-one in the scan is caught.
    assert_eq!(
        rows.first().map(|r| r.mnemonic.as_str()),
        Some("acrobatsleap")
    );
    assert_eq!(
        rows.first().map(|r| r.display_name.as_str()),
        Some("Acrobat's Leap")
    );
}

/// A multi-word display name is not mistaken for the mnemonic.
///
/// `Acrobat's Leap`, `Unarmed Specialist`, `Side by Side` -- the name has an
/// unknown word count, which is why the classifier scans for the `x/y`
/// fraction and works backwards rather than counting from the left.
#[test]
fn multi_word_names_keep_their_mnemonic() {
    let line = "  Side by Side         sidebyside      0/5   Passive";
    let Some(row) = PsmLine::classify(line) else {
        panic!("should classify");
    };
    assert_eq!(row.mnemonic, "sidebyside");
    assert_eq!(row.display_name, "Side by Side");
    assert_eq!(row.ranks.ranks, 0);
    assert_eq!(row.ranks.max, 5);
}

/// The optional Category column is read when present and empty when not.
#[test]
fn the_guild_category_is_read_when_present() {
    let with = "  Disarm Weapon        disarm          5/5   Setup          Warrior Guild";
    let Some(row) = PsmLine::classify(with) else {
        panic!("should classify");
    };
    assert_eq!(row.kind, "Setup");
    assert_eq!(row.category, "Warrior Guild");

    let without = "  Dirtkick             dirtkick        0/5   Setup";
    let Some(row) = PsmLine::classify(without) else {
        panic!("should classify");
    };
    assert_eq!(row.kind, "Setup");
    assert_eq!(row.category, "");
}

/// A line with a fraction but too few fields is not a row.
///
/// Found by mutation: deleting the `fraction_at < 2` guard left the suite
/// green. A row needs a display name AND a mnemonic before the fraction, so a
/// fraction in the first two fields cannot be one.
///
/// These are not hypothetical. `Available Combat Maneuvers Points: 20` sits
/// directly above this very blob in the source log, and players write
/// fractions in chat constantly. Without the guard, `he 5/5 whatever` becomes
/// a maneuver named "he" -- the `state.rs:306-309` rule that a player can say
/// anything.
#[test]
fn a_fraction_with_too_few_fields_is_not_a_row() {
    for line in ["5/5", "Ashryn 3/5", "  0/5   Passive"] {
        assert_eq!(PsmLine::classify(line), None, "should not classify: {line}");
    }
}

/// The header and the rule are not rows.
#[test]
fn table_furniture_is_not_classified() {
    for line in [
        "  Skill                Mnemonic        Ranks Type           Category        Subcategory",
        "  -------------------------------------------------------------------------------------",
        "The output listed above was generated based on the following filters:",
        "  Availability: profession",
        "",
    ] {
        assert_eq!(PsmLine::classify(line), None, "should not classify: {line}");
    }
}

/// Both header forms are recognised, including the one Lich drops.
///
/// `parser.rb:29`'s `PSMStart` matches only `are available:`, so `CMAN INFO`
/// output is silently discarded by Lich. Reading both is the fix.
#[test]
fn both_header_forms_are_recognised() {
    assert_eq!(
        cena_model::state::character::psm::classify_header(
            "Ashryn, the following Combat Maneuvers are available:"
        ),
        Some(PsmCategory::CombatManeuver)
    );
    assert_eq!(
        cena_model::state::character::psm::classify_header(
            "Ashryn, your Combat Maneuvers are as follows:"
        ),
        Some(PsmCategory::CombatManeuver),
        "the `as follows:` form is the one Lich drops"
    );
    assert_eq!(
        cena_model::state::character::psm::classify_header("Ashryn says, \"hello\""),
        None
    );
}

/// The capture's own header classifies, so the form is under test from wire
/// bytes rather than from a string I typed.
#[test]
fn the_captures_header_classifies() {
    let found = psm_rows().iter().any(|(l, _)| {
        cena_model::state::character::psm::classify_header(l) == Some(PsmCategory::CombatManeuver)
    });
    assert!(found, "the fixture's header must classify");
}

/// The `LIST` terminator is recognised; the three leading spaces matter.
#[test]
fn the_table_terminator_is_recognised() {
    assert!(cena_model::state::character::psm::is_table_end(
        "   Subcategory: all"
    ));
    // A player saying it, without the column indent.
    assert!(!cena_model::state::character::psm::is_table_end(
        "Subcategory: all"
    ));
}

/// `replace_category` clears only its own category.
///
/// Guard-before-clear: assert both tables are populated first, or the test
/// passes without the scoping existing.
#[test]
fn replacing_one_category_leaves_the_others() {
    let mut set = PsmSet::default();
    let cman: Vec<PsmLine> = psm_rows()
        .iter()
        .filter_map(|(l, b)| PsmLine::classify_with_bold(l, *b))
        .collect();
    set.replace_category(PsmCategory::CombatManeuver, &cman);
    set.replace_category(
        PsmCategory::Feat,
        &[PsmLine {
            mnemonic: "wps".to_owned(),
            display_name: "Weighting".to_owned(),
            ranks: cena_model::PsmRanks {
                ranks: 1,
                max: 5,
                known_by_bold: true,
            },
            kind: "Passive".to_owned(),
            category: String::new(),
        }],
    );

    assert_eq!(set.len(PsmCategory::CombatManeuver), 27, "guard");
    assert_eq!(set.len(PsmCategory::Feat), 1, "guard");

    set.replace_category(PsmCategory::CombatManeuver, &[]);

    assert_eq!(set.len(PsmCategory::CombatManeuver), 0);
    assert_eq!(
        set.len(PsmCategory::Feat),
        1,
        "replacing cman must not touch feat"
    );
}

/// An unknown mnemonic is stored, not dropped.
///
/// The open-set requirement, and the defect `inventory/10` §8 records against
/// Lich's enhancive parser: unrecognised names are silently discarded there.
#[test]
fn an_unknown_mnemonic_is_kept() {
    let line = "  Future Maneuver      futuremnv       2/5   Attack";
    let Some(row) = PsmLine::classify(line) else {
        panic!("an unknown mnemonic must still classify");
    };
    assert_eq!(row.mnemonic, "futuremnv");

    let mut set = PsmSet::default();
    set.replace_category(PsmCategory::CombatManeuver, &[row]);
    assert!(set.get(PsmCategory::CombatManeuver, "futuremnv").is_some());
}

/// "Never read" and "not in the table" are different answers.
///
/// This is MO-3's distinction, built in from the start rather than retrofitted.
#[test]
fn an_unread_table_differs_from_an_absent_mnemonic() {
    let mut set = PsmSet::default();
    assert!(!set.has_table(PsmCategory::Shield), "guard: nothing read");
    assert_eq!(set.get(PsmCategory::Shield, "bulwark"), None);

    set.replace_category(PsmCategory::Shield, &[]);

    assert!(set.has_table(PsmCategory::Shield), "the table was read");
    assert_eq!(
        set.get(PsmCategory::Shield, "bulwark"),
        None,
        "and bulwark is genuinely not in it"
    );
}

/// The five PSM categories, and Ascension is not one of them.
///
/// > **AUTHOR, 2026-09-19:** *"there's a few .. cman, shield, weapon, armor,
/// > feat are psms"*
///
/// This enum had six variants, taken from `infomon/cli.rb`'s sync list. That
/// grouping is Lich's sync convenience, not the game's taxonomy.
#[test]
fn there_are_five_psm_categories() {
    assert_eq!(PsmCategory::ALL.len(), 5);
    let names: Vec<&str> = PsmCategory::ALL.iter().map(|c| c.as_str()).collect();
    assert_eq!(names, vec!["armor", "cman", "feat", "shield", "weapon"]);
    assert_eq!(PsmCategory::parse("ascension"), None);
}
