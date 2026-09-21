//! The spell table.
//!
//! The data is Lich's `data/effect-list.xml`, cut by
//! `tools/extract_spells.rb`. These tests are mostly **parity with the
//! source**: a table that silently loses rows or mis-splits a column is the
//! failure mode, and the game never corrects it.

use cena_model::spells::{self, CastType, CooldownKind, Duration};

#[test]
fn every_spell_in_the_source_survived_the_cut() {
    // MEASURED at `C:/Gemstone/lich-5/data/effect-list.xml`: the file holds
    // **515 `<spell>` elements and 514 distinct numbers.**
    //
    // The difference is a real duplicate in Lich's own data -- 9052 "Duck and
    // Weave Cooldown" is listed twice, byte-identical. Lich handles it the
    // same way this does, by keeping the first:
    //
    //   @@list.push(self) unless @@list.find { |spell| spell.num == @num }
    //                                                    -- `spell.rb:160`
    //
    // So 514 is the correct answer and an earlier version of this test
    // asserting 515 was wrong about the data, not about the reader.
    assert_eq!(spells::all().count(), 514);
    assert_eq!(
        spells::spell(9052).map(|s| s.name.as_str()),
        Some("Duck and Weave Cooldown"),
        "the duplicate resolves to one spell, not none"
    );
}

#[test]
fn a_spell_carries_what_the_table_said() {
    let heroism = spells::spell(215).expect("215 Heroism");
    assert_eq!(heroism.name, "Heroism");
    assert_eq!(heroism.circle(), 2, "a three-digit number's first digit");
    // The table says `offense`, and an earlier version of this test asserted
    // that as though the label settled what the spell does. It does not
    // (author, 2026-09-20):
    //
    //   "heroism provides an offensive bonus, so probably a utility spell,
    //    it does not damage on it's own so it's not an attack spell, which
    //    may be different than offense/defense."
    //
    // Quite so, and the data agrees -- see `the_type_tag_is_free_text` for
    // the vocabulary and `an_offense_spell_is_not_an_attack_spell` for the
    // measurement. What a behavior should read is the BONUSES column, which
    // says what the spell actually confers.
    assert_eq!(heroism.kind.as_deref(), Some("offense"));
    assert_eq!(heroism.availability.as_deref(), Some("group"));
    assert_eq!(heroism.mana, Some(15));
    let bonuses: Vec<&str> = heroism.bonuses.iter().map(|(k, _)| k.as_str()).collect();
    assert!(
        bonuses.contains(&"bolt-as") && bonuses.contains(&"physical-as"),
        "attack-strength bonuses, not damage: {bonuses:?}"
    );
}

#[test]
fn a_spell_is_found_by_name_whatever_the_case() {
    assert_eq!(spells::spell_named("heroism").map(|s| s.number), Some(215));
    assert_eq!(
        spells::spell_named("  Heroism ").map(|s| s.number),
        Some(215)
    );
    assert_eq!(spells::spell_named("Not A Spell"), None);
}

mod circles {
    use super::spells;

    #[test]
    fn the_circle_is_the_leading_digits() {
        // `spell.rb`: three digits means one leading digit, otherwise two. So
        // 215 is circle 2 and 1215 is circle 12 -- not 1.
        assert_eq!(spells::spell(215).expect("215").circle(), 2);
        assert_eq!(spells::spell(1215).expect("1215").circle(), 12);
    }

    #[test]
    fn the_circle_names_are_ported_whole_including_their_gaps() {
        // `spells.rb:6`. The numbering is not contiguous -- there is no 13,
        // 14 or 15, and it jumps to 65, 66, then the 90s.
        assert_eq!(spells::circle_name(1), Some("Minor Spirit"));
        assert_eq!(spells::circle_name(12), Some("Minor Mental"));
        assert_eq!(spells::circle_name(16), Some("Paladin"));
        assert_eq!(spells::circle_name(99), Some("Council of Light"));
        assert_eq!(spells::circle_name(13), None, "there is no circle 13");
    }

    #[test]
    fn an_unknown_circle_is_none_rather_than_a_string() {
        // Lich returns the STRING "Unknown Circle" (`spells.rb:30`), which a
        // caller cannot distinguish from a real name without comparing
        // against that literal. `None` lets them ask.
        assert_eq!(spells::circle_name(255), None);
    }

    #[test]
    fn the_miscellaneous_typo_is_corrected() {
        // Lich spells circle 90 "Micellaneous" (`spells.rb:26`). It is a
        // display string with no wire meaning and nothing matches on it, so
        // shipping the typo would not be fidelity.
        assert_eq!(spells::circle_name(90), Some("Miscellaneous"));
    }
}

mod durations {
    use super::{CastType, Duration, spells};

    #[test]
    fn the_three_kinds_split_as_measured() {
        // 178 / 120 / 40 over 338 bodies. The counts are in the TSV's header
        // too, so a regeneration that shifts them is visible in the diff --
        // but a diff is not a test, and this is the test.
        let (mut fixed, mut derived, mut unknown) = (0, 0, 0);
        for spell in spells::all() {
            for (_, duration) in &spell.durations {
                match duration {
                    Duration::Fixed(_) => fixed += 1,
                    Duration::Derived(_) => derived += 1,
                    Duration::Unknown(_) => unknown += 1,
                }
            }
        }
        // 337, not the extractor's 338: the duplicated spell 9052 declares one
        // duration, and only one copy of it survives the dedupe above.
        assert_eq!((fixed, derived, unknown), (177, 120, 40));
        assert_eq!(fixed + derived + unknown, 337, "every duration accounted");
    }

    #[test]
    fn a_plain_number_reads_as_minutes() {
        let fixed = Duration::Fixed("120".to_owned());
        assert_eq!(fixed.minutes(), Some(120.0));
    }

    #[test]
    fn a_derived_duration_keeps_its_expression_and_answers_no_minutes() {
        // **Knowable, but not from here.** `20 + Spells.minorspiritual` needs
        // a character; the table has none. Returning a number would mean
        // inventing the ranks.
        let warding = spells::spell(101).expect("101 Spirit Warding I");
        let target = warding
            .duration(CastType::Target)
            .expect("a target duration");
        assert!(matches!(target, Duration::Derived(_)));
        assert_eq!(target.source(), "20 + Spells.minorspiritual");
        assert_eq!(target.minutes(), None, "not a guess");
    }

    #[test]
    fn real_ruby_reads_as_unknown_and_keeps_its_source() {
        // §5.2: absent is not zero. Five of the 40 `reget` the scrollback and
        // re-parse a CS/TD line -- a scripting language embedded in a data
        // file, which this project rules out by settled decision. A consumer
        // gets `None` rather than the `0.25` fallback buried in the Ruby.
        let unknown: Vec<_> = spells::all()
            .flat_map(|s| &s.durations)
            .filter(|(_, d)| matches!(d, Duration::Unknown(_)))
            .collect();
        assert_eq!(unknown.len(), 40);
        assert!(
            unknown.iter().any(|(_, d)| d.source().contains("reget")),
            "the scrollback-scraping ones are among them"
        );
        for (_, duration) in &unknown {
            assert_eq!(duration.minutes(), None);
            assert!(!duration.source().is_empty(), "the Ruby is kept");
        }
    }

    #[test]
    fn a_spell_with_two_forms_states_both() {
        let warding = spells::spell(101).expect("101");
        assert!(warding.duration(CastType::SelfCast).is_some());
        assert!(warding.duration(CastType::Target).is_some());
    }
}

mod cooldowns {
    use super::{CooldownKind, spells};

    #[test]
    fn the_five_spells_that_lock_a_character_out() {
        let found: Vec<_> = spells::with_cooldowns()
            .map(|s| (s.number, s.name.as_str()))
            .collect();
        assert_eq!(
            found,
            vec![
                (140, "Wall of Force"),
                (211, "Bravery"),
                (215, "Heroism"),
                (219, "Spell Shield"),
                (506, "Celerity"),
            ]
        );
    }

    #[test]
    fn the_seconds_are_the_tables() {
        assert_eq!(
            spells::spell(219)
                .expect("219")
                .cooldown(CooldownKind::Group),
            Some(360)
        );
        assert_eq!(
            spells::spell(140)
                .expect("140")
                .cooldown(CooldownKind::Target),
            Some(270)
        );
        assert_eq!(
            spells::spell(140)
                .expect("140")
                .cooldown(CooldownKind::Group),
            None,
            "Wall of Force has no group cooldown"
        );
    }

    #[test]
    fn the_two_cooldown_kinds_split_on_cast_mechanics() {
        // **THE INVARIANT**, and it is about how a spell is CAST rather than
        // about where the data comes from (author, 2026-09-20):
        //
        //   "the ones looking at member are ones that are self cast but
        //    affect your group."
        //
        // A `group` cooldown is a spell you cast on YOURSELF that lands on
        // everyone grouped -- so the line names nobody and there is nothing
        // to capture. A `target` cooldown is cast at one character, and the
        // `target-start` message names them.
        //
        // Checked across the WHOLE table, not the five rows, because five
        // samples agreeing is a coincidence and 515 disagreeing nowhere is a
        // rule.
        for spell in spells::all() {
            let has_target_cooldown = spell.cooldown(CooldownKind::Target).is_some();
            let has_group_cooldown = spell.cooldown(CooldownKind::Group).is_some();
            assert_eq!(
                has_target_cooldown,
                spell.target_start.is_some(),
                "{} {}: a target cooldown and a target-start message go together",
                spell.number,
                spell.name
            );
            assert!(
                !(has_group_cooldown && spell.target_start.is_some()),
                "{} {}: a self-cast group spell names nobody",
                spell.number,
                spell.name
            );
        }
    }

    #[test]
    fn a_group_spells_start_message_carries_the_clause_that_tells_the_views_apart() {
        // The caster's ONLY notice that a group casting landed is a clause
        // inside their own start message (`parser.rb:662` tests
        // `line.include?('your group')`). Each of the three is one pattern
        // with the clause as an optional alternation.
        for number in [211, 215, 219] {
            let spell = spells::spell(number).expect("a group-cooldown spell");
            let message = spell.message_up.as_deref().expect("a start message");
            assert!(
                message.contains("your group"),
                "{number} {}: {message}",
                spell.name
            );
        }
    }

    #[test]
    fn celerity_carries_both_forms() {
        // The case that proves the split is about mechanics and not about the
        // spell: self-cast with `and your group` it starts group cooldowns;
        // cast at someone else its `target-start` names them.
        let celerity = spells::spell(506).expect("506 Celerity");
        assert!(
            celerity
                .message_up
                .as_deref()
                .expect("a start message")
                .contains("your group"),
            "the self-cast-on-group form"
        );
        assert!(
            celerity
                .target_start
                .as_deref()
                .expect("a target-start")
                .contains("noun"),
            "and the form that names one character"
        );
    }

    #[test]
    fn a_target_start_captures_a_name_the_game_permits() {
        // `(?<noun>[A-Z][a-z]+)` -- one capital then lowercase, which is what
        // GemStone allows a name to be. Recorded because an earlier file in
        // this project invented names to call that pattern a defect, and it
        // is not one (author, 2026-09-20).
        let wall = spells::spell(140).expect("140");
        assert_eq!(
            wall.target_start.as_deref(),
            Some(r"A wall of force surrounds (?<noun>[A-Z][a-z]+)\.")
        );
    }
}

#[test]
fn costs_and_bonuses_are_read() {
    let warding = spells::spell(101).expect("101");
    assert_eq!(warding.mana, Some(1));
    assert_eq!(warding.spirit, None, "it costs no spirit");
    let bonuses: Vec<&str> = warding.bonuses.iter().map(|(k, _)| k.as_str()).collect();
    assert!(bonuses.contains(&"bolt-ds"), "{bonuses:?}");
    assert_eq!(
        warding
            .bonuses
            .iter()
            .find(|(k, _)| k == "bolt-ds")
            .map(|(_, v)| v.as_str()),
        Some("10")
    );
}

#[test]
fn an_absent_field_is_none_rather_than_empty() {
    // §5.2 at the TSV boundary: an empty cell means the table said nothing,
    // and `Some("")` would be a consumer's problem to detect.
    let calm = spells::spell(201).expect("201 Calm");
    assert_eq!(calm.message_up, None, "Calm declares no start message");
    assert!(calm.durations.is_empty(), "nor a duration");
    assert_eq!(calm.mana, Some(1), "but it does declare a cost");
}

mod the_type_tag {
    use super::spells;
    use std::collections::BTreeSet;

    #[test]
    fn the_type_tag_is_free_text_not_a_vocabulary() {
        // **It looks like an enum and is not one.** MEASURED over the 514
        // spells: 19 distinct values, slash-separated, with the same idea
        // spelled more than one way --
        //
        //   `offense` (26) and `offensive` (2)
        //   `offense/utility` (1) and `offensive/utility` (1)
        //   `attack/utility` (12) and `utility/attack` (1)
        //   `defense/utility` (5) and `utility/defense` (1)
        //
        // so order varies too. That is why `Spell::kind` is a `String` and
        // not a typed enum: C21 reserves typed fields for CLOSED
        // vocabularies, and this is an open, inconsistent tag list.
        //
        // A test that pinned the 19 values would go red on a Lich data
        // update for no reason. What is worth pinning is the SHAPE: the
        // underlying tags are few, and a new one is worth noticing.
        let tags: BTreeSet<&str> = spells::all()
            .filter_map(|s| s.kind.as_deref())
            .flat_map(|kind| kind.split('/'))
            .collect();
        assert_eq!(
            tags,
            [
                "area",
                "attack",
                "bonus",
                "defense",
                "offense",
                "offensive",
                "timer",
                "utility"
            ]
            .into_iter()
            .collect::<BTreeSet<_>>(),
            "a tag outside this set is a data change worth reading"
        );
    }

    #[test]
    fn an_offense_spell_is_not_an_attack_spell() {
        // The author's distinction, measured. An `offense` spell improves
        // your offence; an `attack` spell does damage. They are different
        // axes, and the separation in the data is stark:
        //
        //   offense spells carrying an AS/CS bonus:  23 of 34
        //   attack  spells carrying an AS/CS bonus:   2 of 136
        //
        // So a behavior asking "will this spell hurt something" must not
        // read `offense` as yes, and one asking "will this make me hit
        // harder" must not read `attack` as yes.
        let confers_a_bonus = |kind: &str| {
            spells::all()
                .filter(|s| s.kind.as_deref().is_some_and(|k| k.contains(kind)))
                .filter(|s| {
                    s.bonuses
                        .iter()
                        .any(|(t, _)| t.ends_with("-as") || t.ends_with("-cs"))
                })
                .count()
        };
        assert_eq!(confers_a_bonus("offens"), 23);
        assert_eq!(confers_a_bonus("attack"), 2);
    }
}
