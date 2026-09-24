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
    // Quite so. The tag is now PARSED rather than asserted as a string --
    // see the `roles` module -- and what a behavior reads is `Role::Offense`
    // plus the bonuses, not the label.
    assert_eq!(heroism.kind.as_deref(), Some("offense"), "as written");
    assert!(heroism.is(cena_model::Role::Offense));
    assert!(!heroism.is(cena_model::Role::Attack), "it does no damage");
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
    fn the_circle_is_total_over_every_number() {
        // `number` is a `pub` field, and `circle` used to slice the decimal
        // string `[..2]` -- a panic on any one-digit number. Ruby's
        // `"5"[0..1]` is `"5"`, so a number under 100 is its own circle, and
        // a five-digit one keeps its first two digits (`"65535"[0..1]`).
        let circle = |number| {
            spells::Spell {
                number,
                ..Default::default()
            }
            .circle()
        };
        assert_eq!(circle(0), 0);
        assert_eq!(circle(5), 5);
        assert_eq!(circle(12), 12);
        assert_eq!(circle(99), 99);
        assert_eq!(circle(100), 1);
        assert_eq!(circle(999), 9);
        assert_eq!(circle(1000), 10);
        assert_eq!(circle(65_535), 65);
        for number in 0..=u16::MAX {
            let _ = circle(number);
        }
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
    fn the_table_was_cut_from_a_copy_that_has_cooldowns() {
        // **The guard against a vacuous pass.** Two effect-list.xml files
        // exist on the author's machine and differ by exactly this feature:
        // the Sep 11 copy predates PR #1597 and declares ZERO cooldowns.
        //
        // Cut from that one, `with_cooldowns()` is empty and every test below
        // passes over nothing -- a green suite reporting a feature that is
        // not there. The right file was picked by luck; this makes luck
        // unnecessary.
        assert_eq!(
            spells::with_cooldowns().count(),
            5,
            "cut from a copy with no cooldowns -- re-run tools/extract_spells.rb              against a current effect-list.xml"
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
        let mut checked = 0;
        for spell in spells::all() {
            let has_target_cooldown = spell.cooldown(CooldownKind::Target).is_some();
            checked += usize::from(has_target_cooldown || spell.target_start.is_some());
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
        assert_eq!(
            checked, 2,
            "the invariant held over nothing -- see              the_table_was_cut_from_a_copy_that_has_cooldowns"
        );
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

mod roles {
    use super::spells;
    use cena_model::Role;
    use std::collections::BTreeSet;

    #[test]
    fn the_four_roles_are_the_authors() {
        // (author, 2026-09-20):
        //
        //   "we have attack, utility, offense, defense. attack would be like
        //    a bolt spell or warding spell, utility would be like floating
        //    disk, water walking, offense would be like heroism, defense
        //    would be like 618."
        assert!(spells::spell(215).expect("Heroism").is(Role::Offense));
        assert!(spells::spell(618).expect("618").is(Role::Defense));
        assert!(
            spells::spell(901)
                .expect("901 Minor Shock")
                .is(Role::Attack),
            "a bolt spell"
        );
        assert!(
            spells::spell(130)
                .expect("130 Floating Disk")
                .is(Role::Utility),
            "a floating disk"
        );
    }

    #[test]
    fn an_offense_spell_is_not_an_attack_spell() {
        // The distinction that matters to a behavior: one does damage, the
        // other makes you better at doing it. MEASURED --
        //
        //   offense spells carrying an AS/CS bonus:  23 of 34
        //   attack  spells carrying an AS/CS bonus:   2 of 136
        let heroism = spells::spell(215).expect("Heroism");
        assert!(heroism.is(Role::Offense));
        assert!(!heroism.is(Role::Attack), "it does no damage itself");

        let confers = |role| {
            spells::all()
                .filter(|s| s.is(role))
                .filter(|s| {
                    s.bonuses
                        .iter()
                        .any(|(t, _)| t.ends_with("-as") || t.ends_with("-cs"))
                })
                .count()
        };
        assert_eq!(confers(Role::Offense), 23);
        assert_eq!(confers(Role::Attack), 2);
    }

    #[test]
    fn both_spellings_of_offense_read_the_same() {
        // The table writes it `offense` 26 times and `offensive` twice. Same
        // word, and 9816 is one of the two.
        let odd = spells::spell(9816).expect("9816 Symbol of Supremacy");
        assert_eq!(odd.kind.as_deref(), Some("offensive"), "guard: as written");
        assert!(odd.is(Role::Offense), "and read as the same role");
    }

    #[test]
    fn order_does_not_change_the_roles() {
        // `attack/utility` (12) and `utility/attack` (1) are the same set.
        // Lich's only consumer is `@type =~ /attack/i` (`spell.rb:700`), a
        // substring test, so order has never carried anything.
        let both: BTreeSet<Role> = [Role::Attack, Role::Utility].into_iter().collect();
        let forward = spells::all().find(|s| s.kind.as_deref() == Some("attack/utility"));
        let reversed = spells::all().find(|s| s.kind.as_deref() == Some("utility/attack"));
        assert_eq!(forward.expect("one exists").roles(), both);
        assert_eq!(reversed.expect("one exists").roles(), both);
    }

    #[test]
    fn area_is_a_modifier_and_not_a_role() {
        // MEASURED: five spells carry `area`, and every one also carries
        // `attack`. It qualifies how an attack lands rather than naming what
        // the spell is for, so it is its own question.
        let area: Vec<_> = spells::all().filter(|s| s.is_area()).collect();
        assert_eq!(area.len(), 5);
        for spell in &area {
            assert!(
                spell.is(Role::Attack),
                "{} {} is an area spell that is not an attack",
                spell.number,
                spell.name
            );
        }
        assert!(
            !spells::spell(215).expect("Heroism").is_area(),
            "and an ordinary spell is not"
        );

        // **`area` must not leak into `roles()`.** A mutation folding it into
        // `Role::Bonus` survived the first version of this test: it checked
        // `is_area` and `Role::Bonus` separately and never asserted that an
        // area spell is not a Bonus. `Elemental Wave` is `attack/area` and
        // that is exactly two facts, not three.
        let wave = spells::spell(410).expect("410 Elemental Wave");
        assert_eq!(
            wave.kind.as_deref(),
            Some("attack/area"),
            "guard: as written"
        );
        assert_eq!(
            wave.roles(),
            [Role::Attack].into_iter().collect::<BTreeSet<_>>(),
            "one role, and `area` is not one of them"
        );
    }

    #[test]
    fn a_spell_with_no_type_has_no_roles() {
        // Twelve state none. Section 5.2: that is not the same as "does
        // nothing", and twelve of them are plainly timers -- `Celerity
        // Recovery`, `Shadow Mastery Cooldown`. They are left untagged
        // rather than reclassified, because guessing which is invention.
        let untyped: Vec<_> = spells::all().filter(|s| s.kind.is_none()).collect();
        assert_eq!(untyped.len(), 12);
        for spell in &untyped {
            assert!(spell.roles().is_empty());
        }
    }

    #[test]
    fn every_tag_in_the_table_is_read() {
        // Rule 2.2 at the data boundary. A tag neither `Role` nor `area`
        // accounts for is REPORTED, not dropped -- so a regeneration that
        // adds a seventh category goes red here instead of silently losing
        // it.
        let unread: Vec<(u16, Vec<&str>)> = spells::all()
            .map(|s| (s.number, s.unreadable_roles()))
            .filter(|(_, tags)| !tags.is_empty())
            .collect();
        assert!(unread.is_empty(), "unreadable type tags: {unread:?}");
    }

    #[test]
    fn every_role_is_used_by_the_table() {
        // The other direction: a variant nothing in the data produces is a
        // category invented rather than observed.
        for role in Role::ALL {
            assert!(
                spells::all().any(|s| s.is(role)),
                "{role:?} is in the enum and not in the table"
            );
        }
    }
}
