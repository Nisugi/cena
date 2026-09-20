//! The stat classifier, against the real `info` blob.
//!
//! M3 step 3. The fixture is `character_info.xml`, cut from
//! `GSIV-Nisugi/2026/09/xml/2026-09-01_15-13-56.xml:28249-28266` and scrubbed
//! (`crates/cena-protocol/tests/FIXTURES.md`).
//!
//! # Why a fixture and not snippets
//!
//! `plan/18`'s own lesson: `pbarStance` shipped in `vitals` past 17 green tests
//! because every model test used hand-written strings. A hand-written stat line
//! would also have quietly had the wrong number of spaces, or no `<pushBold/>`,
//! and the enhancement rule would have been tested against fiction.

use cena_model::{StatKind, StatLine, StatValue};
use cena_protocol::Parser;
use cena_protocol::frame::Frame;

/// The reassembled lines of the `info` fixture, in order.
///
/// **This is the reassembly the consumer will own** (`plan/12` §3a): a frame
/// boundary is not a line boundary, so runs are joined until `ends_line`. Doing
/// it here, in the test, is what proves the classifier's input contract before
/// the consumer exists to provide it.
///
/// No `unwrap`/`expect`/`panic!`: the workspace denies all three and
/// `clippy.toml`'s allowance covers `#[test]` functions only.
fn info_lines() -> Vec<String> {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../cena-protocol/tests/fixtures/character_info.xml");
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

/// The same reassembly, but recording which spans were bold.
///
/// Returns `(line, bold_fragments)`. The bold fragments are the game's own
/// enhancement signal.
fn info_lines_with_bold() -> Vec<(String, Vec<String>)> {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../cena-protocol/tests/fixtures/character_info.xml");
    let bytes = std::fs::read(&path).unwrap_or_default();
    let mut parser = Parser::new();
    let mut frames = parser.push_bytes(&bytes);
    frames.extend(parser.push_bytes(b"\n"));

    let mut out = Vec::new();
    let mut line = String::new();
    let mut bold: Vec<String> = Vec::new();
    for frame in &frames {
        if let Frame::Text(text) = frame {
            line.push_str(&text.content);
            if text.style.bold_depth > 0 {
                bold.push(text.content.clone());
            }
            if text.ends_line {
                out.push((std::mem::take(&mut line), std::mem::take(&mut bold)));
            }
        }
    }
    out
}

#[test]
fn the_fixture_reassembles_into_whole_lines() {
    // Guard against vacuity: an unreadable fixture yields no lines and every
    // test below would pass over nothing.
    let lines = info_lines();
    assert!(
        lines.len() >= 12,
        "the info blob has a header, a Name line, ten stat lines and a Mana \
         line; got {} lines: {lines:#?}",
        lines.len()
    );
    assert!(
        lines.iter().any(|l| l.contains("Strength (STR)")),
        "the stat table must survive reassembly: {lines:#?}"
    );
}

#[test]
fn all_ten_stats_classify_from_the_real_blob() {
    // The completeness check. A classifier that handled nine would pass every
    // single-stat test.
    let found: Vec<StatKind> = info_lines()
        .iter()
        .filter_map(|l| StatLine::classify(l))
        .map(|s| s.kind)
        .collect();
    assert_eq!(
        found,
        StatKind::ALL.to_vec(),
        "all ten stats must classify, in the wire's print order"
    );
}

#[test]
fn a_two_column_line_has_no_normal_value() {
    // `info` prints Ascended and Enhanced. `normal` is `info full` only, and a
    // classifier that filled it from column 0 would report the ascended value
    // as the base -- silently wrong, and wrong in the direction that looks
    // plausible.
    let line = info_lines()
        .into_iter()
        .find(|l| l.contains("Strength (STR)"))
        .expect("the fixture carries a Strength line");
    let stat = StatLine::classify(&line).expect("it classifies");
    assert_eq!(stat.column_count, 2, "`info` sends two columns");
    assert_eq!(
        stat.normal(),
        None,
        "the base value is unknown until `info full` is run, and None says so"
    );
    assert_eq!(
        stat.ascended(),
        Some(StatValue {
            value: 115,
            bonus: 32
        })
    );
    assert_eq!(
        stat.enhanced(),
        Some(StatValue {
            value: 115,
            bonus: 32
        })
    );
}

#[test]
fn a_three_column_line_fills_normal_and_shifts_the_rest() {
    // `info full`. Lich's regex handles both with an optional FIRST group
    // (`parser.rb:14`), so the column meanings shift rather than extend: with
    // three columns the middle one is ascended, not the first.
    let stat =
        StatLine::classify("    Strength (STR):   110 (30)    ...  115 (32)    ...  120 (35)")
            .expect("the three-column form classifies");
    assert_eq!(stat.column_count, 3);
    assert_eq!(
        stat.normal(),
        Some(StatValue {
            value: 110,
            bonus: 30
        }),
        "with three columns the FIRST is the base value"
    );
    assert_eq!(
        stat.ascended(),
        Some(StatValue {
            value: 115,
            bonus: 32
        }),
        "and the middle is ascended -- the position that shifts"
    );
    assert_eq!(
        stat.enhanced(),
        Some(StatValue {
            value: 120,
            bonus: 35
        }),
        "enhanced is always last"
    );
}

#[test]
fn a_negative_bonus_survives() {
    // `parser.rb:14` captures `(?<bonus>-?[0-9]+)` and Lich's own fixture
    // asserts `Aura (AUR): 100 (-35)` -> -35
    // (`spec/lib/gemstone/infomon_spec.rb:91`). An unsigned field would wrap
    // this into 65,501.
    let stat = StatLine::classify("        Aura (AUR):   100 (-35)    ...  100 (-35)")
        .expect("a negative bonus classifies");
    assert_eq!(
        stat.enhanced(),
        Some(StatValue {
            value: 100,
            bonus: -35
        }),
        "a stat below the racial mean has a negative bonus"
    );
}

#[test]
fn the_bold_runs_are_the_enhancement_signal() {
    // **The measurement this whole step exists for.**
    //
    // The wire bolds an enhanced number. MEASURED through Cena's parser, the
    // Intuition line is FIVE frames, only the last with `ends_line`:
    //
    //   bold=0 ends=false "   Intuition (INT):    98 (24)    ...  "
    //   bold=1 ends=false "106"
    //   bold=0 ends=false " ("
    //   bold=1 ends=false "28"
    //   bold=0 ends=true  ")"
    //
    // So bold survives into the frames and needs no regex, which is `plan/12`
    // §3a's bargain: every fact the markup encodes reaches the classifier.
    let with_bold = info_lines_with_bold();

    let enhanced: Vec<(&str, &Vec<String>)> = with_bold
        .iter()
        .filter(|(_, bold)| !bold.is_empty())
        .map(|(line, bold)| (line.as_str(), bold))
        .collect();
    assert_eq!(
        enhanced.len(),
        2,
        "exactly two stats are enhanced in this capture -- Intuition and \
         Wisdom. If this is 0 the bold is being lost; if it is 10 the \
         reassembly is attributing it to every line. Got: {enhanced:#?}"
    );

    let intuition = enhanced
        .iter()
        .find(|(line, _)| line.contains("Intuition"))
        .expect("Intuition is one of the two");
    assert_eq!(
        intuition.1,
        &vec!["106".to_owned(), "28".to_owned()],
        "the bold spans are the enhanced value and its bonus, and nothing else"
    );

    // And the plain lines carry no bold at all, or the signal means nothing.
    let strength = with_bold
        .iter()
        .find(|(line, _)| line.contains("Strength"))
        .expect("Strength is in the fixture");
    assert!(
        strength.1.is_empty(),
        "an unenhanced stat must have no bold runs, or bold cannot distinguish \
         anything: {:?}",
        strength.1
    );
}

#[test]
fn an_enhanced_stat_still_classifies_across_its_five_frames() {
    // Reassembly must produce a line the classifier accepts. If the bold runs
    // were dropped instead of joined, this line would read
    // "Intuition (INT): 98 (24) ... ()" and fail to classify -- which is the
    // failure mode of treating one frame as one line.
    let line = info_lines()
        .into_iter()
        .find(|l| l.contains("Intuition"))
        .expect("the fixture carries an Intuition line");
    let stat = StatLine::classify(&line).expect("an enhanced line classifies");
    assert_eq!(
        stat.ascended(),
        Some(StatValue {
            value: 98,
            bonus: 24
        })
    );
    assert_eq!(
        stat.enhanced(),
        Some(StatValue {
            value: 106,
            bonus: 28
        }),
        "the enhanced column is higher than the ascended one, which is what \
         being enhanced means"
    );
}

#[test]
fn a_line_that_is_not_a_stat_line_classifies_as_none() {
    // Most of the blob is not a stat line, and a classifier that guessed would
    // write garbage into ten typed fields.
    for line in [
        "                Ascended (Bonus)  ...  Enhanced (Bonus)",
        "Name: Ashryn Race: Half-Elf  Profession: Ranger (shown as: Hero)",
        "Gender: Male    Age: 36    Expr: 43,904,921    Level:  100",
        "Mana:  413   Silver: 0",
        "",
        "You also see a rock.",
    ] {
        assert_eq!(
            StatLine::classify(line),
            None,
            "{line:?} is not a stat line"
        );
    }
}

#[test]
fn a_mismatched_name_and_code_is_refused() {
    // **A player can type anything.** `state.rs:306-309` records the rule for
    // the idle warning -- `trim() ==` rather than `contains`, because otherwise
    // "a player can say anything, and a `contains` would let one make the
    // supervisor stop reconnecting by typing it."
    //
    // Same hazard here: an echoed line could carry a plausible stat shape. The
    // long name and the three-letter code must agree, which a forgery has to
    // get right in two places.
    assert_eq!(
        StatLine::classify("    Strength (INT):   999 (99)    ...  999 (99)"),
        None,
        "the long name and the abbreviation must agree"
    );
    // And the genuine pairing still works, or the check is too strict.
    assert!(
        StatLine::classify("   Intuition (INT):    98 (24)    ...   98 (24)").is_some(),
        "a matching name and code classifies"
    );
}

#[test]
fn a_single_column_line_is_refused() {
    // The wire sends two columns or three. One is a shape this classifier does
    // not know, and filling `ascended` from it would be a guess.
    assert_eq!(
        StatLine::classify("    Strength (STR):   115 (32)"),
        None,
        "a one-column line is not a shape the wire sends"
    );
}

#[test]
fn a_malformed_column_refuses_the_whole_line() {
    // Half a parse is worse than none: it writes one real column and one
    // wrong one.
    for line in [
        "    Strength (STR):   115 (32)    ...  abc (32)",
        "    Strength (STR):   115 (32)    ...  115 32",
        "    Strength (STR):   115 (32)    ...  115 (",
    ] {
        assert_eq!(
            StatLine::classify(line),
            None,
            "{line:?} must be refused whole, not parsed in part"
        );
    }
}

#[test]
fn stat_kinds_round_trip_through_both_spellings() {
    // Two namespaces -- the Lich key (`strength`) and the wire code (`STR`) --
    // on one value, so they cannot drift.
    for kind in StatKind::ALL {
        assert_eq!(StatKind::parse(kind.as_str()), Some(kind));
        assert_eq!(StatKind::parse_abbrev(kind.abbrev()), Some(kind));
        // The wire prints the code upper-case; accept either case.
        assert_eq!(
            StatKind::parse_abbrev(&kind.abbrev().to_lowercase()),
            Some(kind)
        );
    }
    assert_eq!(StatKind::parse("charisma"), None, "there is no charisma");
    assert_eq!(StatKind::parse_abbrev("CHA"), None);

    // Distinct spellings, or one stat would be unreachable.
    let mut abbrevs: Vec<&str> = StatKind::ALL.iter().map(|k| k.abbrev()).collect();
    abbrevs.sort_unstable();
    let before = abbrevs.len();
    abbrevs.dedup();
    assert_eq!(before, abbrevs.len(), "codes must be unique: {abbrevs:?}");
}

#[test]
fn an_unknown_stat_is_none_rather_than_zero() {
    // `plan/12` §5.2: "`Unknown` is a first-class value, not a default." A
    // zeroed StatValue is indistinguishable from a real stat of 0.
    let stat = cena_model::Stat::default();
    assert!(!stat.is_known(), "a default Stat has been told nothing");
    assert_eq!(stat.effective(), None, "and has no effective value");

    let known = cena_model::Stat {
        ascended: Some(StatValue { value: 0, bonus: 0 }),
        ..cena_model::Stat::default()
    };
    assert!(
        known.is_known(),
        "a stat of zero IS known, and must not read as absent -- this is the \
         distinction the Option exists for"
    );
}

#[test]
fn effective_prefers_the_most_authoritative_column() {
    let base = StatValue {
        value: 10,
        bonus: 1,
    };
    let asc = StatValue {
        value: 20,
        bonus: 2,
    };
    let enh = StatValue {
        value: 30,
        bonus: 3,
    };

    let all = cena_model::Stat {
        normal: Some(base),
        ascended: Some(asc),
        enhanced: Some(enh),
        enhanced_is_bolded: true,
    };
    assert_eq!(all.effective(), Some(enh), "enhanced wins when present");

    let no_enh = cena_model::Stat {
        normal: Some(base),
        ascended: Some(asc),
        enhanced: None,
        enhanced_is_bolded: false,
    };
    assert_eq!(no_enh.effective(), Some(asc), "then ascended");

    let only_base = cena_model::Stat {
        normal: Some(base),
        ascended: None,
        enhanced: None,
        enhanced_is_bolded: false,
    };
    assert_eq!(only_base.effective(), Some(base), "then normal");
}
