//! The skill classifier, against the real `skills full` table, and the table
//! folded into `character.skills` at the prompt (`skills::read_table`).
//!
//! M3 step 5. The fixture is `character_skills.xml`, cut from
//! `GSIV-Nisugi/2026/09/2026-09-18_15-49-13.xml:11197-11266` and scrubbed
//! (`crates/cena-protocol/tests/FIXTURES.md`).
//!
//! # Why a fixture and not snippets
//!
//! `plan/18`'s lesson, and this file is a second instance of it. Every fact
//! below was wrong in my first draft of the design and right only after the
//! capture was read: the column order (`Bonus` before `Ranks`), that untrained
//! skills arrive as explicit zeros rather than being omitted, and that spell
//! circles sit under their own repeated header instead of inside the table. A
//! hand-written snippet would have encoded all three mistakes and passed.

use cena_model::{SkillKind, SkillLine, SkillSet};
use cena_protocol::Parser;
use cena_protocol::frame::Frame;

/// The reassembled lines of the skills fixture, with their bold fragments.
///
/// A frame boundary is not a line boundary, so runs are joined until
/// `ends_line`. Bold is read off `style.bold_depth`, which the parser tracks
/// **across** frames -- necessary here, because the wire's closing `<popBold/>`
/// for a row arrives *after* that row's `ends_line`.
fn skill_lines() -> Vec<(String, Vec<String>)> {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../cena-protocol/tests/fixtures/character_skills.xml");
    let bytes = std::fs::read(&path).unwrap_or_default();
    let mut parser = Parser::new();
    let mut frames = parser.push_bytes(&bytes);
    frames.extend(parser.push_bytes(b"\n"));

    let mut lines = Vec::new();
    let mut current = String::new();
    let mut bold: Vec<String> = Vec::new();
    for frame in &frames {
        if let Frame::Text(text) = frame {
            current.push_str(&text.content);
            if text.style.bold_depth > 0 && !text.content.trim().is_empty() {
                bold.push(text.content.clone());
            }
            if text.ends_line {
                lines.push((std::mem::take(&mut current), std::mem::take(&mut bold)));
            }
        }
    }
    if !current.is_empty() {
        lines.push((current, bold));
    }
    lines
}

/// Every one of the 46 ported names appears in the real capture.
///
/// **This is the completeness test the plan asks for.** A typo in
/// `display_name` -- `Two Handed Weapons` for `Two-Handed Weapons`, or a
/// missing ` - ` in a lore -- makes that skill silently unparseable forever.
/// Here it is a failure that names the skill.
#[test]
fn every_ported_skill_name_appears_in_the_capture() {
    let lines = skill_lines();
    let mut missing = Vec::new();
    for kind in SkillKind::ALL {
        let found = lines
            .iter()
            .any(|(line, _)| line.contains(kind.display_name()));
        if !found {
            missing.push(kind.display_name());
        }
    }
    assert!(missing.is_empty(), "not in the capture: {missing:?}");
}

/// The capture classifies all 46 skills, and nothing else as a skill.
#[test]
fn the_table_yields_exactly_forty_six_skills() {
    let mut set = SkillSet::default();
    let mut circles = 0;
    for (line, bold) in skill_lines() {
        let refs: Vec<&str> = bold.iter().map(String::as_str).collect();
        if let Some((parsed, bolded)) = SkillLine::classify_with_bold(&line, &refs) {
            if matches!(parsed, SkillLine::SpellCircle { .. }) {
                circles += 1;
            }
            set.apply(&parsed, bolded);
        }
    }
    assert_eq!(set.known_count(), 46, "every skill should be present");
    assert_eq!(circles, 2, "the capture has two spell circles");
}

/// The columns are `Bonus` then `Ranks` -- the reverse of the spoken order.
///
/// MEASURED against the capture's own header. `Two Weapon Combat` reads
/// `312 212`: 212 ranks conferring a 312 bonus. Reading them the other way
/// round would give a character 312 ranks in a skill, which is impossible at
/// level 100 and would be believed by everything downstream.
#[test]
fn bonus_comes_before_ranks() {
    let line = "  Two Weapon Combat..................|     312     212";
    let Some(SkillLine::Skill { kind, bonus, ranks }) = SkillLine::classify(line) else {
        panic!("should classify as a skill");
    };
    assert_eq!(kind, SkillKind::TwoWeaponCombat);
    assert_eq!(bonus, 312, "the FIRST column is the bonus");
    assert_eq!(ranks, 212, "the SECOND column is the ranks");
}

/// An untrained skill arrives as an explicit zero, not as an absent row.
///
/// This is what makes clear-before-apply safe, and it is the fact that
/// reversed my own recommendation. Guard-before-assert: the skill must be
/// present at all before its zero means anything.
#[test]
fn untrained_skills_are_present_as_zero() {
    let mut set = SkillSet::default();
    for (line, _) in skill_lines() {
        if let Some(parsed) = SkillLine::classify(&line) {
            set.apply(&parsed, false);
        }
    }
    let armor = set.get(SkillKind::ArmorUse).copied();
    assert!(armor.is_some(), "Armor Use must be in the table at all");
    assert_eq!(armor.and_then(|s| s.ranks), Some(0));
    assert!(armor.is_some_and(|s| s.is_untrained()));
}

/// Bold marks the enhanced rows, and only those.
///
/// The capture has six bolded skills. Asserting the unbolded ones too is what
/// makes this a test of the signal rather than of one row.
#[test]
fn bold_marks_exactly_the_enhanced_skills() {
    let mut set = SkillSet::default();
    for (line, bold) in skill_lines() {
        let refs: Vec<&str> = bold.iter().map(String::as_str).collect();
        if let Some((parsed, bolded)) = SkillLine::classify_with_bold(&line, &refs) {
            set.apply(&parsed, bolded);
        }
    }

    let enhanced: Vec<&str> = SkillKind::ALL
        .into_iter()
        .filter(|k| set.get(*k).is_some_and(|s| s.enhanced))
        .map(SkillKind::display_name)
        .collect();

    assert_eq!(
        enhanced,
        vec![
            "Two Weapon Combat",
            "Combat Maneuvers",
            "Ranged Weapons",
            "Ambush",
            "Dodging",
            "Spiritual Lore - Blessings",
            "Spiritual Lore - Summoning",
        ]
    );
}

/// The bonus cell's bold fragment keeps its column padding, and still matches.
///
/// Found by mutation: removing `.trim()` from the bold comparison left the
/// suite green, because `bold_marks_exactly_the_enhanced_skills` keys on ranks
/// and the ranks fragment happens to arrive unpadded. The bonus fragment does
/// not -- MEASURED as `"  312"` against `"212"` in the capture -- and the bonus
/// is the number an enhancive actually inflates.
///
/// Asserting the padded cell alone is what makes the trim load-bearing.
#[test]
fn a_padded_bold_fragment_still_matches() {
    let line = "  Two Weapon Combat..................|     312     212";
    let Some((_, bolded)) = SkillLine::classify_with_bold(line, &["  312"]) else {
        panic!("should classify");
    };
    assert!(bolded, "a padded bonus fragment must match its column");
}

/// A row with no bold is not reported as enhanced.
///
/// The negative half: without it, `classify_with_bold` returning `true`
/// unconditionally would pass every other bold test here.
#[test]
fn an_unbolded_row_is_not_enhanced() {
    let line = "  Edged Weapons......................|     302     202";
    let Some((_, bolded)) = SkillLine::classify_with_bold(line, &[]) else {
        panic!("should classify");
    };
    assert!(!bolded);
}

/// A spell circle has one number and its own header; it is not a skill.
#[test]
fn a_spell_circle_is_not_a_skill() {
    let line = "  Minor Spiritual....................|              40";
    let Some(SkillLine::SpellCircle { name, ranks }) = SkillLine::classify(line) else {
        panic!("one number should classify as a circle");
    };
    assert_eq!(name, "Minor Spiritual");
    assert_eq!(ranks, 40);
}

/// The discriminator is the FIELD COUNT, not the clause order.
///
/// Lich tries `Pattern::Skill` before `Pattern::SpellRanks` and must, because
/// `SpellRanks` (`parser.rb:24`) matches a skill row too and would take its
/// *bonus* as the rank. This asserts our classifier does not have that
/// ordering dependence: a two-number row is a skill no matter what else has
/// been tried, and a one-number row never reports a skill's bonus as ranks.
#[test]
fn a_two_number_row_never_reads_as_a_circle() {
    let line = "  Survival..........................|     403     303";
    match SkillLine::classify(line) {
        Some(SkillLine::Skill { ranks, bonus, .. }) => {
            assert_eq!(ranks, 303);
            assert_eq!(bonus, 403);
        }
        other => panic!("two numbers must be a skill, got {other:?}"),
    }
}

/// The header, the footer and the blanks are not rows.
///
/// `state.rs:306-309`'s rule -- *"a player can say anything"* -- applies to the
/// table's own furniture as much as to chat.
#[test]
fn table_furniture_is_not_classified() {
    for line in [
        "  Skill Name                         | Current Current",
        "                                     |   Bonus   Ranks",
        "Spell Lists",
        "",
        "Training Points: 3673 Phy 0 Mnt (2866 Phy converted to Mnt)",
        "(Use SKILLS BASE to display unmodified ranks and goals)",
    ] {
        assert_eq!(
            SkillLine::classify(line),
            None,
            "should not classify: {line}"
        );
    }
}

/// A KNOWN skill name with only one number is refused, not read as a circle.
///
/// Found by mutation: deleting the `SkillKind::parse(name).is_none()` guard on
/// the circle branch left the whole suite green. Without it, a one-number row
/// naming a real skill becomes a `SpellCircle` called "Survival" -- a circle
/// that does not exist, holding a number of unknown meaning.
///
/// This shape is not hypothetical. The capture's own footer advertises
/// `SKILLS BASE`, a second form of the table, and until it has been captured
/// and read we do not know how many columns it carries. Refusing an
/// unrecognised shape is Rule 2.2; inventing a circle from it is the failure
/// `plan/12` §5.2 calls believing something nobody said.
#[test]
fn a_known_skill_with_one_number_is_not_a_circle() {
    let line = "  Survival..........................|             303";
    assert_eq!(
        SkillLine::classify(line),
        None,
        "a known skill name must never classify as a spell circle"
    );
}

/// An unknown name with two numbers is refused, not guessed at.
#[test]
fn an_unknown_two_number_row_is_refused() {
    let line = "  Basket Weaving....................|     100     200";
    assert_eq!(SkillLine::classify(line), None);
}

/// A third number means a shape we have not seen; refuse it.
#[test]
fn three_numbers_are_refused() {
    let line = "  Survival..........................|     403     303     101";
    assert_eq!(SkillLine::classify(line), None);
}

/// `clear` drops everything, so a fresh table cannot merge onto a stale one.
///
/// Guard-before-clear, per `reconnect_invalidation.rs:7-12`: assert the fact
/// was known first, or the test passes without the code existing.
#[test]
fn clear_forgets_skills_and_circles() {
    let mut set = SkillSet::default();
    for (line, _) in skill_lines() {
        if let Some(parsed) = SkillLine::classify(&line) {
            set.apply(&parsed, false);
        }
    }
    assert_eq!(set.known_count(), 46, "guard: the table was read");
    assert!(set.circle("Ranger").is_some(), "guard: a circle was read");

    set.clear();

    assert_eq!(set.known_count(), 0);
    assert_eq!(set.circle("Ranger"), None);
    assert_eq!(set.get(SkillKind::Survival), None);
}

/// The Lich key spelling matches `Infomon._key`'s normalisation.
///
/// `infomon.rb:132` is `downcase`, `tr(' -', '_')`, then collapse runs. The
/// lores are the case that proves it: `Elemental Lore - Air` has a space,
/// hyphen, space, which becomes three underscores and collapses to one.
#[test]
fn keys_match_lichs_normalisation() {
    assert_eq!(SkillKind::TwoWeaponCombat.key(), "two_weapon_combat");
    assert_eq!(SkillKind::TwoHandedWeapons.key(), "two_handed_weapons");
    assert_eq!(SkillKind::ElementalLoreAir.key(), "elemental_lore_air");
    assert_eq!(SkillKind::StalkingAndHiding.key(), "stalking_and_hiding");
    assert_eq!(SkillKind::Pickpocketing.key(), "pickpocketing");
}

/// Every key is distinct.
///
/// This is the `feat.wps` lesson as a test: two names collapsing to one key is
/// how Lich loses data, and it is cheap to assert that ours cannot.
#[test]
fn no_two_skills_share_a_key() {
    let mut keys: Vec<String> = SkillKind::ALL.into_iter().map(SkillKind::key).collect();
    keys.sort();
    let before = keys.len();
    keys.dedup();
    assert_eq!(keys.len(), before, "two skills collapsed to one key");
}

/// Round-trip: every display name parses back to its own variant.
#[test]
fn display_names_round_trip() {
    for kind in SkillKind::ALL {
        assert_eq!(SkillKind::parse(kind.display_name()), Some(kind));
    }
}

// --- the table, folded from the wire (`read_table`, `SkillSet::replace`) -----
//
// The full table is the committed capture. The plain `skills` table is
// SYNTHETIC: no committed fixture carries one, so it is the wiki's own example
// (`reference/wiki_clean/Verb_SKILLS.txt:21-48`, the MediaWiki leading space
// on its header kept), which is Lich's shape too (`infomon/parser.rb:22-25`).

const SKILLS_FULL: &str = include_str!("../../cena-protocol/tests/fixtures/character_skills.xml");

/// `Verb_SKILLS.txt:21-48`, verbatim, then a prompt to close the chunk.
const SKILLS_PLAIN: &str =
    " Person (at level 100), your current skill bonuses and ranks (including all modifiers) are:
  Skill Name                         | Current Current
                                     |   Bonus   Ranks
  Armor Use..........................|      40       8
  Combat Maneuvers...................|     147      47
  Ranged Weapons.....................|     249     149
  Physical Fitness...................|     201     101
  Dodging............................|     136      38
  Arcane Symbols.....................|     204     104
  Magic Item Use.....................|     202     102
  Harness Power......................|     207     107
  Elemental Mana Control.............|     202     102
  Elemental Lore - Air...............|     150      50
  Elemental Lore - Water.............|     113      27
  Perception.........................|     185      85
  Climbing...........................|     155      55
  Swimming...........................|     155      55

Spell Lists
  Major Elemental....................|              66

Spell Lists
  Minor Elemental....................|              75

Spell Lists
  Wizard.............................|              93

Training Points: 59 Phy 0 Mnt (1458 Phy converted to Mnt)
<prompt time=\"2\">&gt;</prompt>
";

fn feed(state: &mut cena_model::GameState, wire: &str) {
    let mut parser = Parser::new();
    for frame in parser.push_bytes(wire.as_bytes()) {
        state.apply(&frame);
    }
}

/// Feed `wire`, then report what the model holds for Multi Opponent Combat.
fn moc_after(wire: &str) -> Option<u16> {
    let mut state = cena_model::GameState::default();
    feed(&mut state, wire);
    state.character.skills.ranks(SkillKind::MultiOpponentCombat)
}

#[test]
fn the_real_skills_full_table_is_folded_into_the_model() {
    use cena_model::state::character::snapshot::Group;
    let mut state = cena_model::GameState::default();
    let skills = &state.character.skills;
    assert_eq!(
        skills.ranks(SkillKind::MultiOpponentCombat),
        None,
        "guard: nothing is known before the table"
    );

    feed(&mut state, SKILLS_FULL);

    let skills = &state.character.skills;
    assert_eq!(skills.known_count(), 46);
    // bigshot's two (`bigshot.lic:6147-6201`): ranks, not bonus.
    assert_eq!(skills.ranks(SkillKind::MultiOpponentCombat), Some(101));
    assert_eq!(skills.ranks(SkillKind::SpiritualLoreBlessings), Some(122));
    // Bonus first on the wire, and the bold cell carried through the chunk.
    let twc = skills.get(SkillKind::TwoWeaponCombat).copied();
    assert_eq!(
        twc.map(|s| (s.bonus, s.ranks, s.enhanced)),
        Some((Some(312), Some(212), true))
    );
    assert_eq!(
        skills.get(SkillKind::EdgedWeapons).map(|s| s.enhanced),
        Some(false)
    );
    assert_eq!(
        skills.ranks(SkillKind::ArmorUse),
        Some(0),
        "an untrained row is zero"
    );
    assert_eq!(skills.circle("Ranger"), Some(162));
    assert_eq!(skills.circle("Minor Spiritual"), Some(40));
    assert!(state.character.take_taught().contains(&Group::Skills));

    // A later prompt with no table teaches nothing and changes nothing.
    feed(
        &mut state,
        "You look around.\n<prompt time=\"3\">&gt;</prompt>\n",
    );
    assert!(!state.character.take_taught().contains(&Group::Skills));
    assert_eq!(
        state.character.skills.ranks(SkillKind::MultiOpponentCombat),
        Some(101)
    );
}

#[test]
fn a_plain_table_records_what_it_omits_as_untrained() {
    let mut state = cena_model::GameState::default();
    feed(&mut state, SKILLS_FULL);
    assert_eq!(
        state.character.skills.ranks(SkillKind::MultiOpponentCombat),
        Some(101),
        "guard: the full table was read"
    );

    feed(&mut state, SKILLS_PLAIN);

    let skills = &state.character.skills;
    assert_eq!(
        skills.known_count(),
        46,
        "every skill answers after a table"
    );
    assert_eq!(
        skills.get(SkillKind::ArmorUse).map(|s| (s.bonus, s.ranks)),
        Some((Some(40), Some(8)))
    );
    // Listed in the full table, left out of this one: untrained now, and no
    // longer enhanced -- not a stale 101 carried over.
    let moc = skills.get(SkillKind::MultiOpponentCombat).copied();
    assert_eq!(moc.map(|s| (s.ranks, s.bonus)), Some((Some(0), Some(0))));
    assert_eq!(
        skills.get(SkillKind::TwoWeaponCombat).map(|s| s.enhanced),
        Some(false)
    );
    assert_eq!(skills.circle("Wizard"), Some(93));
    assert_eq!(skills.circle("Ranger"), None, "the old circles are gone");
}

#[test]
fn a_table_is_folded_only_with_its_footer() {
    let unclosed = SKILLS_PLAIN.replace("Training Points: 59 Phy 0 Mnt", "Training Points:");
    assert_eq!(moc_after(&unclosed), None, "no footer, nothing known");
    assert_eq!(
        moc_after(SKILLS_PLAIN),
        Some(0),
        "guard: the same table closed"
    );
}

#[test]
fn a_spoken_or_base_header_opens_nothing() {
    // SYNTHETIC: a player saying the header, with a real table's body after.
    let spoken = SKILLS_PLAIN.replace(
        " Person (at level 100)",
        "Bob says, \"Person (at level 100)",
    );
    assert_eq!(moc_after(&spoken), None);
    // `SKILLS BASE`'s header (`Verb_SKILLS.txt:61`) over the same body.
    let base = SKILLS_PLAIN.replace(
        "your current skill bonuses and ranks (including all modifiers) are:",
        "your base skill bonuses, ranks and goals are:",
    );
    assert_eq!(moc_after(&base), None);
}

#[test]
fn the_header_and_footer_shapes() {
    use cena_model::state::character::skills::{is_table_end, is_table_header};
    let header = "Ashryn (at level 100), your current skill bonuses and ranks (including all modifiers) are:";
    assert!(is_table_header(header));
    assert!(
        is_table_header(&format!(" {header}")),
        "Lich's leading space"
    );
    assert!(!is_table_header(
        "Ashryn (at level ten), your current skill bonuses and ranks"
    ));
    assert!(!is_table_header(
        "Ashryn (at level ), your current skill bonuses and ranks"
    ));

    assert!(is_table_end(
        "Training Points: 3673 Phy 0 Mnt (2866 Phy converted to Mnt)"
    ));
    for line in [
        "Training Points: many Phy 0 Mnt",
        "Training Points: 3673 Mnt 0 Phy",
        // Only the `Phy` word is wrong: the case the Phy check alone refuses
        // (found by mutation; the line above is also refused by `Mnt`).
        "Training Points: 3673 Ptp 0 Mnt",
        "Training Points: 3673 Phy lots Mnt",
        "Training Points: 3673 Phy 0",
        "  Training Points: 3673 Phy 0 Mnt",
    ] {
        assert!(!is_table_end(line), "not a footer: {line}");
    }
}
