//! The ways home.

use cena_model::state::fog::{FogMethod, uidless};
use cena_model::{GameState, SkillLine};
use cena_protocol::Parser;

/// A character with a level and some spell circles.
fn character(level: u16, circles: &[(&str, u16)]) -> GameState {
    let mut state = GameState::default();
    state.character.experience.level = Some(format!("Level {level}"));
    for (name, ranks) in circles {
        state.character.skills.apply(
            &SkillLine::SpellCircle {
                name: (*name).to_owned(),
                ranks: *ranks,
            },
            false,
        );
    }
    state
}

mod methods {
    use super::FogMethod;

    #[test]
    fn bigshots_numbering_is_one_based() {
        // `fog.rb:37`'s NUMBERS, in order.
        assert_eq!(FogMethod::from_number(1), Some(FogMethod::SpiritGuide));
        assert_eq!(FogMethod::from_number(2), Some(FogMethod::SymbolOfReturn));
        assert_eq!(FogMethod::from_number(5), Some(FogMethod::FamiliarGate));
    }

    #[test]
    fn zero_is_not_a_method() {
        // The off-by-one a one-based table invites. `0` would index the first
        // entry if it were not guarded.
        assert_eq!(FogMethod::from_number(0), None);
    }

    #[test]
    fn six_is_the_profiles_own_commands_and_not_a_method() {
        // bigshot uses 6 for "the profile's own command list", which is
        // POLICY and belongs to the caller. A sixth variant would make it
        // look like a way home nobody implemented.
        assert_eq!(FogMethod::from_number(6), None);
    }

    #[test]
    fn a_name_or_a_number_both_read() {
        assert_eq!(
            FogMethod::parse("spirit_guide"),
            Some(FogMethod::SpiritGuide)
        );
        assert_eq!(
            FogMethod::parse("  Spirit_Guide "),
            Some(FogMethod::SpiritGuide)
        );
        assert_eq!(FogMethod::parse("3"), Some(FogMethod::TravelersSong));
        assert_eq!(FogMethod::parse("not a fog"), None);
    }

    #[test]
    fn three_are_spells_and_two_are_society_abilities() {
        let spells: Vec<_> = FogMethod::ALL
            .into_iter()
            .filter(|m| m.spell().is_some())
            .collect();
        let abilities: Vec<_> = FogMethod::ALL
            .into_iter()
            .filter(|m| m.ability().is_some())
            .collect();
        assert_eq!(spells.len(), 3);
        assert_eq!(abilities.len(), 2);
        assert!(
            FogMethod::ALL
                .into_iter()
                .all(|m| m.spell().is_some() != m.ability().is_some()),
            "each is one or the other, never both or neither"
        );
    }

    #[test]
    fn the_society_abilities_exist_in_the_society_tables() {
        // A name that does not resolve would make `knows_fog` answer `false`
        // forever, which is indistinguishable from a character who is not a
        // member.
        for method in FogMethod::ALL {
            if let Some((society, name)) = method.ability() {
                assert!(
                    society.ability(name).is_some(),
                    "{} names {name:?}, which {society:?} does not have",
                    method.as_str()
                );
            }
        }
    }
}

mod moving {
    use super::{GameState, Parser, uidless};

    fn arrive(state: &mut GameState, id: &str) {
        let mut parser = Parser::new();
        for frame in parser.parse_line(&format!("<nav rm='{id}'/>")) {
            state.apply(&frame);
        }
    }

    #[test]
    fn a_real_uid_is_not_uidless() {
        assert!(!uidless("7503251"), "a real room");
        assert!(uidless("1234567890123"), "an MD5 stand-in");
        assert!(uidless("not a number"));
    }

    #[test]
    fn a_different_room_is_a_move() {
        let mut state = GameState::default();
        arrive(&mut state, "7503251");
        let start = state.position();
        arrive(&mut state, "7086");
        assert!(state.moved_from(&start));
    }

    #[test]
    fn the_same_real_uid_is_never_a_move() {
        // A UID is unique, so the room re-declaring itself is not an arrival
        // somewhere else. `Room::arrive` already refuses to treat it as one.
        let mut state = GameState::default();
        arrive(&mut state, "7503251");
        let start = state.position();
        arrive(&mut state, "7503251");
        assert!(!state.moved_from(&start), "standing still");
    }

    #[test]
    fn two_unmapped_rooms_that_read_the_same_are_told_apart() {
        // **THE CASE THIS EXISTS FOR** (`fog.rb:230`). A room with no UID
        // gets an MD5 of its title, description and exits, so two unmapped
        // rooms whose text matches SHARE an id -- and a fog between them
        // looks like standing still on the id alone.
        //
        // Cena counts arrivals rather than room streams, so the tie-break is
        // the count. Here the same MD5 id arrives twice, which `Room::arrive`
        // treats as a re-declaration... so the count does NOT move, and this
        // correctly reads as no move.
        let mut state = GameState::default();
        arrive(&mut state, "1234567890123");
        let start = state.position();
        arrive(&mut state, "1234567890123");
        assert!(
            !state.moved_from(&start),
            "the same id arriving twice is a re-declaration"
        );

        // But an id-less arrival in between IS a move, and the count says so.
        let mut parser = Parser::new();
        for frame in parser.parse_line("<nav/>") {
            state.apply(&frame);
        }
        assert!(state.moved_from(&start), "a room with no id at all");
    }

    #[test]
    fn a_position_taken_before_any_room_is_not_a_move_by_itself() {
        let state = GameState::default();
        let start = state.position();
        assert!(!state.moved_from(&start), "nothing has happened");
    }

    #[test]
    fn the_arrival_count_survives_a_reconnect() {
        // It counts rooms this SESSION has entered. Resetting it would make
        // the first arrival after a reconnect compare equal to a count taken
        // before -- the exact false "did not move" it exists to prevent.
        let mut state = GameState::default();
        arrive(&mut state, "7503251");
        let before = state.arrivals;
        assert!(before > 0, "guard: something was counted");
        state.invalidate_for_reconnect();
        assert_eq!(state.arrivals, before);
    }
}

mod knowing {
    use super::{FogMethod, character};
    use cena_model::state::character::vocabulary::Society;

    #[test]
    fn a_spell_is_known_by_its_position_in_its_circle() {
        // `spell.rb:517`: `(num % 100) <= ranks`. Spirit Guide is 130 --
        // circle 1, position 30 -- so it needs 30 ranks of MINOR SPIRIT.
        // (I first wrote Minor Elemental here, which is circle 4.)
        let known = character(100, &[("Minor Spirit", 30)]);
        assert_eq!(known.knows_spell(130), Some(true));

        let short = character(100, &[("Minor Spirit", 29)]);
        assert_eq!(short.knows_spell(130), Some(false), "one rank short");
    }

    #[test]
    fn circle_ranks_are_capped_by_level() {
        // **The line the first draft of `knows_spell` omitted**
        // (`spell.rb:474`): `[Spells.minorelemental, XMLData.level].min`.
        //
        // A character with 30 circle ranks at level 10 knows the 10th spell
        // of that circle, not the 30th -- so Spirit Guide is NOT known.
        let low_level = character(10, &[("Minor Spirit", 30)]);
        assert_eq!(
            low_level.knows_spell(130),
            Some(false),
            "30 ranks, but only level 10"
        );
        assert_eq!(
            low_level.knows_spell(110),
            Some(true),
            "and the 10th of the circle is"
        );
    }

    #[test]
    fn nothing_read_is_unknown_not_unknown_spell() {
        // §5.2. A character whose circles have never arrived does not "not
        // know" Spirit Guide, and answering `false` would have a travel
        // behavior conclude it is stranded.
        let blank = cena_model::GameState::default();
        assert_eq!(blank.knows_spell(130), None);
        assert_eq!(blank.knows_fog(FogMethod::SpiritGuide), None);
    }

    #[test]
    fn a_society_circle_uses_the_society_rank() {
        // `spell.rb:504-509`: circles 97, 98 and 99 read SOCIETY rank, not
        // circle ranks -- and only while a member. Two of the five ways home
        // are society abilities, so this is not an edge case here.
        let mut voln = character(100, &[]);
        voln.character.standing.society = Some(Some(Society::OrderOfVoln));
        voln.character.standing.society_rank = Some(20);
        assert_eq!(
            voln.knows_spell(9820),
            Some(true),
            "the 20th Voln symbol at rank 20"
        );
        assert_eq!(voln.knows_spell(9821), Some(false), "the 21st is not");
    }

    #[test]
    fn a_society_circle_is_unknown_to_a_non_member() {
        let mut sunfist = character(100, &[]);
        sunfist.character.standing.society = Some(Some(Society::GuardiansOfSunfist));
        sunfist.character.standing.society_rank = Some(20);
        assert_eq!(
            sunfist.knows_spell(9820),
            Some(false),
            "a Sunfist member knows no Voln symbols"
        );
    }

    #[test]
    fn arcane_is_gated_on_profession_and_combat_maneuvers_always_false() {
        // `spell.rb:497` and `:510`. Circle 17 is one spell, known only to
        // five professions; circle 96 was deprecated out of `Spell` entirely.
        let mut wizard = character(100, &[]);
        wizard.character.identity.profession = Some("Wizard".to_owned());
        assert_eq!(wizard.knows_spell(1700), Some(true));
        assert_eq!(wizard.knows_spell(1701), Some(false), "only 1700");

        let mut ranger = character(100, &[]);
        ranger.character.identity.profession = Some("Ranger".to_owned());
        assert_eq!(ranger.knows_spell(1700), Some(false));
    }

    #[test]
    fn a_society_fog_needs_the_society_and_the_rank() {
        let mut voln = character(100, &[]);
        voln.character.standing.society = Some(Some(Society::OrderOfVoln));
        // Symbol of Return is rank 25 (`societies/voln.rs`), not 20 -- I
        // guessed 20 and the test correctly failed.
        voln.character.standing.society_rank = Some(25);
        assert_eq!(voln.knows_fog(FogMethod::SymbolOfReturn), Some(true));
        assert_eq!(
            voln.knows_fog(FogMethod::SigilOfEscape),
            Some(false),
            "a Voln member has no Sunfist sigils"
        );

        let mut novice = voln.clone();
        novice.character.standing.society_rank = Some(24);
        assert_eq!(
            novice.knows_fog(FogMethod::SymbolOfReturn),
            Some(false),
            "not one rank short of it"
        );
    }

    #[test]
    fn a_character_stating_no_society_knows_neither() {
        // `Some(None)` is "stated: none", which is an answer -- distinct from
        // `None`, which is "never asked".
        let mut none = character(100, &[]);
        none.character.standing.society = Some(None);
        assert_eq!(none.knows_fog(FogMethod::SymbolOfReturn), Some(false));
        assert_eq!(none.knows_fog(FogMethod::SigilOfEscape), Some(false));
    }
}

mod availability {
    use super::{FogMethod, character};
    use cena_model::state::vitals::Vital;

    fn with_mana(
        mut state: cena_model::GameState,
        current: i32,
        max: i32,
    ) -> cena_model::GameState {
        state.vitals.insert(
            "mana".to_owned(),
            Vital {
                percent: 100,
                current: Some(current),
                max: Some(max),
            },
        );
        state
    }

    #[test]
    fn a_known_and_affordable_spell_is_available() {
        let state = with_mana(character(100, &[("Minor Spirit", 30)]), 500, 500);
        assert_eq!(state.fog_is_available(FogMethod::SpiritGuide), Some(true));
        assert!(state.fog_available().contains(&FogMethod::SpiritGuide));
    }

    #[test]
    fn a_known_spell_you_cannot_pay_for_is_not_available() {
        // The distinction `available?` draws over `known?` (`fog.rb:66`), and
        // it is the one a travel behavior actually asks.
        let broke = with_mana(character(100, &[("Minor Spirit", 30)]), 1, 500);
        assert_eq!(broke.knows_fog(FogMethod::SpiritGuide), Some(true));
        assert_eq!(broke.fog_is_available(FogMethod::SpiritGuide), Some(false));
        assert!(broke.fog_available().is_empty());
    }

    #[test]
    fn a_method_whose_inputs_are_unknown_is_omitted_not_assumed() {
        // The failure this avoids is a behavior committing to a fog it cannot
        // cast. `fog_available` filters on `Some(true)`, so `None` is out.
        let unread = cena_model::GameState::default();
        assert_eq!(unread.fog_is_available(FogMethod::SpiritGuide), None);
        assert!(unread.fog_available().is_empty());
    }

    #[test]
    fn available_lists_every_way_home_that_works_right_now() {
        let mut state = with_mana(
            character(100, &[("Minor Spirit", 30), ("Bard", 20)]),
            500,
            500,
        );
        state.character.standing.society = Some(Some(
            cena_model::state::character::vocabulary::Society::OrderOfVoln,
        ));
        state.character.standing.society_rank = Some(25);
        let available = state.fog_available();
        assert!(available.contains(&FogMethod::SpiritGuide));
        assert!(available.contains(&FogMethod::TravelersSong));
        assert!(available.contains(&FogMethod::SymbolOfReturn));
        assert!(
            !available.contains(&FogMethod::SigilOfEscape),
            "a Voln member has no Sunfist sigils"
        );
    }
}
