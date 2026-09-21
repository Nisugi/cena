//! Bard song arithmetic.
//!
//! There is no wire here — `spellsong.rb` is 190 lines of formulas and one
//! `nil` guard. So these tests are about **numbers matching Lich's**, and the
//! two curves are checked exhaustively rather than at a few spot values,
//! because a constant that is quietly wrong is never corrected by the game.

use cena_model::{Spellsong, base_duration, to_bonus};

mod curves {
    use super::{base_duration, to_bonus};

    /// Lich's `duration_base_level` (`spellsong.rb:41`), transcribed.
    fn lich_base(level: u32) -> Option<u32> {
        let total = 120;
        match level {
            0..=25 => Some(total + level * 4),
            26..=50 => Some(total + 100 + (level - 25) * 3),
            51..=75 => Some(total + 175 + (level - 50) * 2),
            76..=100 => Some(total + 225 + (level - 75)),
            // Lich logs "unhandled case" and returns the bare 120.
            _ => None,
        }
    }

    /// Lich's `Skills.to_bonus` (`attributes/skills.rb:9`), transcribed as the
    /// loop it actually is rather than as the closed form under test.
    fn lich_bonus(mut ranks: u32) -> u32 {
        let mut bonus = 0;
        while ranks > 0 {
            if ranks > 40 {
                bonus += ranks - 40;
                ranks = 40;
            } else if ranks > 30 {
                bonus += (ranks - 30) * 2;
                ranks = 30;
            } else if ranks > 20 {
                bonus += (ranks - 20) * 3;
                ranks = 20;
            } else if ranks > 10 {
                bonus += (ranks - 10) * 4;
                ranks = 10;
            } else {
                bonus += ranks * 5;
                ranks = 0;
            }
        }
        bonus
    }

    #[test]
    fn the_duration_curve_matches_lich_at_every_level() {
        // Exhaustive, not spot-checked. The curve is four bands and the
        // rewrite folded each band's running total into its constant --
        // exactly the transformation that goes wrong at a boundary.
        for level in 0..=100u16 {
            assert_eq!(
                base_duration(level),
                lich_base(u32::from(level)).expect("within Lich's bands"),
                "level {level}"
            );
        }
    }

    #[test]
    fn the_duration_curve_continues_past_the_level_lich_gives_up_at() {
        // **A deliberate difference.** Above 100 Lich falls through to
        // `Lich.log("unhandled case")` and returns the bare 120
        // (`spellsong.rb:54`) -- so a level 101 bard gets the song of a level
        // 0 bard. The bands are cumulative and the last one continues
        // cleanly, so it is extended.
        assert!(lich_base(101).is_none(), "guard: Lich has no band here");
        assert_eq!(base_duration(100), 370);
        assert_eq!(base_duration(101), 371, "one more second, not 250 fewer");
        assert_eq!(base_duration(120), 390);
    }

    #[test]
    fn the_bonus_curve_matches_lich_at_every_rank() {
        // 0..=400 covers every band including the flat tail past 40. The
        // author's own character has skills past 200 ranks.
        for ranks in 0..=400u16 {
            assert_eq!(
                to_bonus(ranks),
                u16::try_from(lich_bonus(u32::from(ranks))).expect("fits"),
                "{ranks} ranks"
            );
        }
    }

    #[test]
    fn the_bonus_curve_turns_at_the_documented_ranks() {
        // The band edges stated as values, so a rewrite that still matches
        // the transcription above but drifts from the GAME is visible.
        assert_eq!(to_bonus(0), 0);
        assert_eq!(to_bonus(10), 50, "five a rank to ten");
        assert_eq!(to_bonus(20), 90, "then four");
        assert_eq!(to_bonus(30), 120, "then three");
        assert_eq!(to_bonus(40), 140, "then two");
        assert_eq!(to_bonus(100), 200, "then one");
    }
}

mod inputs {
    use cena_model::{GameState, Spellsong};
    use cena_protocol::Parser;

    fn character_after(lines: &[&str]) -> cena_model::Character {
        let mut parser = Parser::new();
        let mut state = GameState::default();
        for line in lines {
            for frame in parser.parse_line(line) {
                state.apply(&frame);
            }
        }
        for frame in parser.parse_line("<prompt time=\"1\">&gt;</prompt>") {
            state.apply(&frame);
        }
        state.character
    }

    #[test]
    fn a_character_with_no_level_has_no_song() {
        // §5.2. Every formula here depends on the level, so there is nothing
        // honest to return -- and defaulting to 0 would give every bard the
        // shortest song in the game, silently.
        let character = cena_model::Character::default();
        assert!(Spellsong::of(&character).is_none());
    }

    #[test]
    fn the_level_is_read_out_of_the_wires_label() {
        // `Experience::level` is stored VERBATIM by deliberate decision --
        // `"Level 100"`, not `100` -- because every consumer that displays it
        // wants the string. Spellsong is the first that wants the number.
        let character = character_after(&[
            "<dialogData id='expr'><label id='yourLvl' value='Level 100'/></dialogData>",
        ]);
        assert_eq!(
            character.experience.level.as_deref(),
            Some("Level 100"),
            "guard: still stored whole"
        );
        let song = Spellsong::of(&character).expect("a level was reported");
        assert_eq!(song.holding_targets(), 1, "and it parsed");
    }
}

mod formulas {
    use super::Spellsong;
    use cena_model::{Character, SkillKind, SkillLine};

    /// A character with a level, some skills and Bard ranks.
    ///
    /// Built through `SkillSet::apply`, the real intake path, rather than
    /// through setters that would exist only for tests. The bonus given to
    /// each skill is the one the CURVE would produce, so a test that wants
    /// the table and the reconstruction to disagree has to say so -- see
    /// `the_skill_tables_own_bonus_beats_the_reconstruction`.
    fn bard(level: u16, air: u16, telepathy: u16, bard_ranks: u16) -> Character {
        let mut character = Character::default();
        character.experience.level = Some(format!("Level {level}"));
        for (kind, ranks) in [
            (SkillKind::ElementalLoreAir, air),
            (SkillKind::MentalLoreTelepathy, telepathy),
        ] {
            character.skills.apply(
                &SkillLine::Skill {
                    kind,
                    bonus: cena_model::to_bonus(ranks),
                    ranks,
                },
                false,
            );
        }
        character.skills.apply(
            &SkillLine::SpellCircle {
                name: "Bard".to_owned(),
                ranks: bard_ranks,
            },
            false,
        );
        character
    }

    #[test]
    fn the_duration_adds_its_four_terms() {
        // `base(level) + LOG + (INF * 3) + (Telepathy * 2)` (`spellsong.rb:35`).
        let character = bard(100, 0, 50, 0);
        let song = Spellsong::of(&character).expect("levelled");
        assert_eq!(
            song.duration(30, 20),
            370 + 30 + 60 + 100,
            "base 370, logic 30, influence 20*3, telepathy 50*2"
        );
    }

    #[test]
    fn the_two_negative_bonuses_stay_negative() {
        // Tonis haste and Depression slow both IMPROVE by going down. A port
        // that made them unsigned would read -3 as a very large number.
        let character = bard(100, 75, 100, 0);
        let song = Spellsong::of(&character).expect("levelled");
        assert_eq!(
            song.tonis_haste_bonus(),
            -3,
            "-1, and one each at 30 and 75"
        );
        assert_eq!(song.depression_slow(), -7, "-2, and one at each of five");

        let none = bard(100, 0, 0, 0);
        let song = Spellsong::of(&none).expect("levelled");
        assert_eq!(song.tonis_haste_bonus(), -1, "no thresholds passed");
        assert_eq!(song.depression_slow(), -2);
    }

    #[test]
    fn holding_targets_is_one_at_zero_ranks() {
        // **Lich is right here by accident.** `1 + ((bard - 1) / 7).truncate`
        // with 0 ranks is `1 + (-1/7)`, and Ruby truncates toward zero, so
        // `1 + 0 = 1`. The same expression in a language that FLOORS gives
        // `1 + (-1) = 0` -- a holding song that holds nobody.
        for (ranks, expected) in [(0, 1), (1, 1), (7, 1), (8, 2), (15, 3)] {
            let character = bard(100, 0, 0, ranks);
            let song = Spellsong::of(&character).expect("levelled");
            assert_eq!(song.holding_targets(), expected, "{ranks} bard ranks");
        }
    }

    #[test]
    fn mirrors_does_not_go_below_its_floor() {
        // Lich computes `20 + ((bard - 19) / 2).round`, which goes NEGATIVE
        // below 19 ranks -- a dodge bonus of 15 at rank 9. Unreachable in
        // play, since the spell needs the ranks to be known at all, but a
        // number that means nothing should not propagate.
        let low = bard(100, 0, 0, 9);
        assert_eq!(
            Spellsong::of(&low).expect("levelled").mirrors_dodge_bonus(),
            20,
            "clamped, not 15"
        );
        let high = bard(100, 0, 0, 39);
        assert_eq!(
            Spellsong::of(&high)
                .expect("levelled")
                .mirrors_dodge_bonus(),
            30,
            "20 + (39-19)/2"
        );
    }

    #[test]
    fn valor_is_capped_by_whichever_is_lower() {
        // `[Spells.bard, Stats.level].min` (`spellsong.rb:150`): a bard with
        // more ranks than levels is capped by level, and vice versa.
        let ranks_exceed = bard(20, 0, 0, 50);
        assert_eq!(
            Spellsong::of(&ranks_exceed)
                .expect("levelled")
                .valor_bonus(),
            10 + (20 - 10) / 2,
            "capped by level"
        );
        let level_exceeds = bard(100, 0, 0, 30);
        assert_eq!(
            Spellsong::of(&level_exceeds)
                .expect("levelled")
                .valor_bonus(),
            10 + (30 - 10) / 2,
            "capped by ranks"
        );
    }

    #[test]
    fn lucks_second_term_keeps_lichs_arithmetic() {
        // **A Lich bug, ported as written.** `(6 + ((bard - 6) / 4) / 2).round`
        // applies `/ 2` to the INNER quotient only, so the renew cost is
        // `6 + over/8` -- not `(6 + over/4) / 2`, which the parallel with
        // every other `*_cost` implies.
        //
        // Not fixed, because "fixing" it would be a guess at the game's real
        // number. Pinned here so the choice is deliberate and a later
        // measurement has something to change.
        let character = bard(100, 0, 0, 46);
        let (cast, renew) = Spellsong::of(&character).expect("levelled").luck_cost();
        assert_eq!(cast, 6 + 40 / 4, "16");
        assert_eq!(renew, 6 + 40 / 4 / 2, "11, not (6+10)/2 = 8");
    }

    #[test]
    fn the_skill_tables_own_bonus_beats_the_reconstruction() {
        // Lich calls `Skills.to_bonus(Skills.elair)` unconditionally
        // (`spellsong.rb:76`), throwing away the figure the game stated in
        // favour of recomputing it from ranks. The two DISAGREE whenever an
        // enhancive is on: the table's bonus includes it and the curve
        // cannot.
        let plain = bard(100, 30, 0, 0);
        assert_eq!(
            Spellsong::of(&plain)
                .expect("levelled")
                .sonic_armor_durability(),
            210 + 50 + 120,
            "guard: 30 ranks is 120 by the curve, and the table agrees"
        );

        // The same 30 ranks, but the game states a bonus an enhancive has
        // inflated. The curve cannot know that; the table does.
        let mut enhanced = plain.clone();
        enhanced.skills.apply(
            &SkillLine::Skill {
                kind: SkillKind::ElementalLoreAir,
                bonus: 200,
                ranks: 30,
            },
            true,
        );
        assert_eq!(
            Spellsong::of(&enhanced)
                .expect("levelled")
                .sonic_armor_durability(),
            210 + 50 + 200,
            "the game's figure, enhancive included -- not the curve's 120"
        );
    }

    #[test]
    fn the_constant_costs_are_transcribed() {
        use cena_model::state::character::spellsong::cost;
        assert_eq!(cost::MANA, (18, 15));
        assert_eq!(cost::FORTITUDE, (3, 1));
        assert_eq!(cost::SHIELD, (9, 4));
        assert_eq!(cost::WEAPON, (12, 4));
        assert_eq!(cost::ARMOR, (14, 5));
        assert_eq!(cost::SWORD, (25, 15));
    }
}
