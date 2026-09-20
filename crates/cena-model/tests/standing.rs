//! The single-line character facts: society, citizenship, warcries, PSMs,
//! resources.
//!
//! # Cut from the author's live output, not invented
//!
//! The society block below is the author's real `society` command, pasted
//! 2026-09-20, and it corrected two things a hand-written snippet would have
//! got wrong: the report line reads `Master **of** the` rather than `in the`,
//! and it arrives indented **after a `<popBold/>`** rather than at the start
//! of a line.
//!
//! `;infomon show` for the same character is the answer key for what a store
//! should hold afterwards: `society.status : "Guardians of Sunfist"`,
//! `society.rank : 20`, `citizenship : "Kraken's Fall"`.

use cena_model::state::character::standing::{
    PsmChange, ResourceAmounts, SocietyEvent, citizenship_line, covert_arts_line, psm_change,
    resource_line, society_line, suffused_line, warcry_line,
};
use cena_model::state::character::vocabulary::{ResourceType, Society, Warcry};

mod society {
    use super::{Society, SocietyEvent, society_line};

    /// The author's live `society` output, verbatim, markup stripped as the
    /// parser would deliver it -- one entry per reassembled line.
    const REPORT: [&str; 4] = [
        "Current society status:",
        "   You are a Master of the Guardians of Sunfist.",
        "Past society affiliations (resigned or cast out):",
        "   You have no past society affiliations.",
    ];

    #[test]
    fn the_authors_real_report_reads_master_of_sunfist() {
        let found: Vec<SocietyEvent> = REPORT.iter().filter_map(|l| society_line(l)).collect();
        assert_eq!(
            found,
            [SocietyEvent::Report {
                society: Some(Society::GuardiansOfSunfist),
                rank: None,
                master: true,
            }],
            "one line in that block is a fact, and it is the membership line"
        );
    }

    #[test]
    fn a_masters_rank_is_not_on_the_line() {
        // `;infomon show` says `society.rank : 20` for this character, and the
        // wire line carries no number at all. 20 is `Sunfist::max_rank()`,
        // supplied by the caller -- which is why `rank` is `None` here rather
        // than a fabricated value.
        let Some(SocietyEvent::Report { rank, society, .. }) =
            society_line("   You are a Master of the Guardians of Sunfist.")
        else {
            panic!("not a report");
        };
        assert_eq!(rank, None, "absent, not zero");
        assert_eq!(society.map(Society::max_rank), Some(20), "what fills it");
    }

    #[test]
    fn the_past_affiliations_section_states_nothing() {
        // A section Lich has no pattern for at all -- newer than its parser.
        // Its lines must not be mistaken for a resignation.
        assert_eq!(
            society_line("Past society affiliations (resigned or cast out):"),
            None
        );
        assert_eq!(
            society_line("   You have no past society affiliations."),
            None
        );
    }

    #[test]
    fn an_unindented_report_line_is_not_a_report() {
        // What stops a player from restating your society by typing it. Lich
        // anchors `^\s+` for the same reason (`parser.rb:38`); a speech frame
        // carries no leading run of spaces.
        assert_eq!(
            society_line("You are a Master of the Guardians of Sunfist."),
            None
        );
        assert!(society_line("   You are a Master of the Guardians of Sunfist.").is_some());
    }

    #[test]
    fn a_ranked_member_reads_both_spellings() {
        for line in [
            "   You are a member in the Order of Voln at rank 5.",
            "   You are a member in the Order of Voln at step 5.",
        ] {
            assert_eq!(
                society_line(line),
                Some(SocietyEvent::Report {
                    society: Some(Society::OrderOfVoln),
                    rank: Some(5),
                    master: false,
                }),
                "{line}"
            );
        }
    }

    #[test]
    fn membership_in_nothing_is_stated_rather_than_absent() {
        assert_eq!(
            society_line("   You are not a member of any society at this time."),
            Some(SocietyEvent::Report {
                society: None,
                rank: None,
                master: false,
            })
        );
    }

    #[test]
    fn joining_the_council_of_light_is_recorded() {
        // BUG FIX, not a port. Lich scans `match[/Order|Council|Guardians/]`
        // and then tests a `'Lodge'` branch that scan cannot produce
        // (`parser.rb:415-425`), so joining the Council records NOTHING. The
        // Council's greeter is the Grand Poohbah, who says "Lodge".
        assert_eq!(
            society_line(
                "The Grand Poohbah smiles broadly.  \"Welcome to the Lodge,\" he cries jubilantly."
            ),
            Some(SocietyEvent::Joined(Society::CouncilOfLight))
        );
    }

    #[test]
    fn the_other_two_joins_are_read() {
        assert_eq!(
            society_line("The Grandmaster says, \"Welcome to the Order of Voln.\""),
            Some(SocietyEvent::Joined(Society::OrderOfVoln))
        );
        assert_eq!(
            society_line(
                "The Grandmaster says, \"You are now a member of the Guardians of Sunfist.\""
            ),
            Some(SocietyEvent::Joined(Society::GuardiansOfSunfist))
        );
    }

    #[test]
    fn an_advancement_says_that_it_happened_and_not_to_what() {
        assert_eq!(
            society_line(
                "Zarak traces the outline of a sigil into the air before you and says, \"Rise.\""
            ),
            Some(SocietyEvent::Stepped)
        );
        assert_eq!(
            society_line("The monk concludes ceremoniously, \"You have advanced.\""),
            Some(SocietyEvent::Stepped)
        );
    }

    #[test]
    fn a_resignation_is_read() {
        assert_eq!(
            society_line(
                "The Grandmaster says, \"I'm sorry to hear that.  You are no longer in our service.\""
            ),
            Some(SocietyEvent::Resigned)
        );
    }
}

mod citizenship {
    use super::citizenship_line;

    #[test]
    fn the_authors_town_survives_its_apostrophe() {
        // `;infomon show` says `citizenship : "Kraken's Fall"`. A town list
        // would have to be updated whenever the game adds one, so it is free
        // text -- and free text here contains both an apostrophe and a space.
        assert_eq!(
            citizenship_line("You currently have full citizenship in Kraken's Fall."),
            Some(Some("Kraken's Fall".to_owned()))
        );
    }

    #[test]
    fn having_none_is_stated_rather_than_absent() {
        assert_eq!(
            citizenship_line("You don't seem to have citizenship."),
            Some(None)
        );
    }

    #[test]
    fn an_unrelated_line_is_not_a_citizenship_line() {
        assert_eq!(citizenship_line("You currently have a headache."), None);
    }
}

mod psms {
    use super::{PsmChange, psm_change};

    #[test]
    fn training_a_rank_is_read_from_ordinary_play() {
        // The author's point about why this layer matters: no sync is
        // involved. You train, the game says so, the store follows.
        assert_eq!(
            psm_change("You have now achieved rank 5 of Shield Bash, costing 20 Combat points."),
            Some(PsmChange {
                name: "Shield Bash".to_owned(),
                category: "Combat".to_owned(),
                rank: 5,
            })
        );
    }

    #[test]
    fn unlearning_stores_what_remains_not_what_was_removed() {
        // The line names the rank being GIVEN UP, so one less remains --
        // Lich's `- 1` at `parser.rb:447`.
        assert_eq!(
            psm_change("You decide to unlearn rank 5 of Shield Bash, regaining 20 Combat points.")
                .map(|c| c.rank),
            Some(4)
        );
    }

    #[test]
    fn unlearning_the_first_rank_leaves_zero_rather_than_underflowing() {
        assert_eq!(
            psm_change("You decide to unlearn rank 1 of Shield Bash, regaining 4 Combat points.")
                .map(|c| c.rank),
            Some(0)
        );
    }

    #[test]
    fn a_bracketed_technique_names_its_category_first() {
        assert_eq!(
            psm_change("[You have gained rank 2 of Shield Specialization: Block Mastery.]"),
            Some(PsmChange {
                name: "Block Mastery".to_owned(),
                category: "Shield".to_owned(),
                rank: 2,
            })
        );
    }

    #[test]
    fn losing_a_technique_is_rank_zero() {
        assert_eq!(
            psm_change("[You are no longer trained in Weapon Technique: Cripple.]").map(|c| c.rank),
            Some(0)
        );
    }

    #[test]
    fn an_unrecognised_category_is_still_reported() {
        // Rule 2.2. Lich drops a PSM whose name it does not know
        // (`plan/dazzling`, bug 5); the category travels as the game's own
        // word so a PSM added tomorrow is carried rather than discarded.
        assert_eq!(
            psm_change("You have now achieved rank 1 of Frobnication, costing 2 Invented points.")
                .map(|c| c.category),
            Some("Invented".to_owned())
        );
    }
}

mod warcries {
    use super::{Warcry, warcry_line};

    #[test]
    fn a_learned_warcry_is_read_from_the_report() {
        assert_eq!(
            warcry_line("     Bertrandt's Bellow"),
            Some(Some(Warcry::BertrandtsBellow))
        );
    }

    #[test]
    fn not_being_a_warrior_states_having_none() {
        assert_eq!(
            warcry_line("You must be an active member of the Warrior Guild to use this skill."),
            Some(None)
        );
    }

    #[test]
    fn an_unindented_name_is_not_a_report_line() {
        assert_eq!(warcry_line("Bertrandt's Bellow"), None);
    }
}

mod resources {
    use super::{ResourceAmounts, ResourceType, covert_arts_line, resource_line, suffused_line};

    #[test]
    fn the_amounts_line_does_not_say_which_resource_it_is() {
        // Lich's alternation is non-capturing (`parser.rb:51`), so the type
        // comes only from a `Suffused` line. That is why the two are stored
        // independently.
        assert_eq!(
            resource_line("Nature's Grace: 0/50,000 (Weekly)     200,000/200,000 (Total)"),
            Some(ResourceAmounts {
                weekly: 0,
                total: 200_000,
            }),
            "matching the author's `resources.total : 200000`"
        );
    }

    #[test]
    fn a_suffused_line_teaches_the_type_and_the_amount() {
        // `;infomon show`: `resources.type : "Nature's Grace"` and
        // `resources.suffused : 1`, which are two keys from this one line.
        assert_eq!(
            suffused_line("Suffused Nature's Grace: 1"),
            Some((ResourceType::NaturesGrace, 1))
        );
    }

    #[test]
    fn the_authors_whole_resource_block_reads_as_three_facts() {
        // The author's live `resource` output, verbatim. The vitals line is
        // not this layer's business -- it is `<progressBar>` data the wire
        // already sends typed -- so exactly three lines here are facts.
        const BLOCK: [&str; 4] = [
            "Health: 192/193     Mana: 405/405     Stamina: 158/158     Spirit: 10/10",
            "Nature's Grace: 0/50,000 (Weekly)     200,000/200,000 (Total)",
            "Suffused Nature's Grace: 1",
            "Covert Arts Charges: 164/200",
        ];
        assert_eq!(BLOCK.iter().filter_map(|l| resource_line(l)).count(), 1);
        assert_eq!(BLOCK.iter().filter_map(|l| suffused_line(l)).count(), 1);
        assert_eq!(
            BLOCK
                .iter()
                .filter_map(|l| covert_arts_line(l))
                .collect::<Vec<_>>(),
            [164],
            "matching the author's `resources.covert_arts_charges : 164`"
        );
    }

    #[test]
    fn covert_arts_charges_can_be_negative() {
        // Lich captures `[-\d,]+` (`parser.rb:54`), so the wire can send a
        // negative. An unsigned type would fail to parse it and store nothing
        // -- the same defect the creature-health work found this month.
        assert_eq!(covert_arts_line("Covert Arts Charges: -5/200"), Some(-5));
    }

    #[test]
    fn a_different_cap_is_refused_rather_than_read_against_the_wrong_scale() {
        assert_eq!(covert_arts_line("Covert Arts Charges: 164/400"), None);
    }

    #[test]
    fn an_unknown_resource_name_is_refused() {
        assert_eq!(
            resource_line("Frobnication: 1/50,000 (Weekly)   2/200,000 (Total)"),
            None
        );
    }
}

mod storing {
    use super::{Society, SocietyEvent, society_line};
    use cena_model::state::character::standing::Standing;

    #[test]
    fn the_authors_master_standing_stores_rank_twenty() {
        // END TO END against real data. The wire line carries no number:
        //
        //   "   You are a Master of the Guardians of Sunfist."
        //
        // and `;infomon show` for the same character says
        // `society.rank : 20`. That 20 is `max_rank()`, applied here.
        let mut standing = Standing::default();
        let event = society_line("   You are a Master of the Guardians of Sunfist.")
            .expect("the author's real line");
        assert!(standing.apply_society(event), "something changed");

        assert_eq!(standing.society, Some(Some(Society::GuardiansOfSunfist)));
        assert_eq!(standing.society_rank, Some(20));
    }

    #[test]
    fn an_unknown_rank_is_not_turned_into_rank_one_by_a_step() {
        // BUG FIX, not a port. Lich does `get + 1` on a possibly-nil value
        // (`parser.rb:428`), turning "we never asked" into "rank 1".
        let mut standing = Standing::default();
        assert!(
            !standing.apply_society(SocietyEvent::Stepped),
            "nothing to change"
        );
        assert_eq!(standing.society_rank, None, "unknown stays unknown");
    }

    #[test]
    fn a_step_advances_a_known_rank() {
        let mut standing = Standing::default();
        standing.apply_society(SocietyEvent::Joined(Society::OrderOfVoln));
        assert_eq!(standing.society_rank, Some(1), "Voln starts at step 1");
        assert!(standing.apply_society(SocietyEvent::Stepped));
        assert_eq!(standing.society_rank, Some(2));
    }

    #[test]
    fn sunfist_starts_at_rank_zero_and_voln_at_one() {
        // Lich's reading at `parser.rb:417-424`, which differs per society.
        let mut voln = Standing::default();
        voln.apply_society(SocietyEvent::Joined(Society::OrderOfVoln));
        assert_eq!(voln.society_rank, Some(1));

        let mut sunfist = Standing::default();
        sunfist.apply_society(SocietyEvent::Joined(Society::GuardiansOfSunfist));
        assert_eq!(sunfist.society_rank, Some(0));
    }

    #[test]
    fn resigning_states_none_rather_than_forgetting() {
        // The difference that matters for a store: "stated: not a member" is
        // knowledge, and must not read the same as "never asked".
        let mut standing = Standing::default();
        standing.apply_society(SocietyEvent::Joined(Society::OrderOfVoln));
        assert!(standing.apply_society(SocietyEvent::Resigned));
        assert_eq!(standing.society, Some(None), "stated, not absent");
        assert_ne!(standing.society, None, "which is NOT the unknown case");
        assert_eq!(standing.society_rank, Some(0));
    }

    #[test]
    fn nothing_is_known_before_the_game_says_so() {
        let fresh = Standing::default();
        assert_eq!(fresh.society, None);
        assert_eq!(fresh.society_rank, None);
        assert_eq!(fresh.citizenship, None);
        assert!(fresh.warcries.is_empty());
    }

    #[test]
    fn restating_the_same_standing_reports_no_change() {
        // What lets a caller mark a group dirty only when there is something
        // to write -- a re-sync that finds everything unchanged should not
        // schedule a file write.
        let mut standing = Standing::default();
        let event = society_line("   You are a Master of the Guardians of Sunfist.").expect("line");
        assert!(standing.apply_society(event), "first time changes");
        assert!(!standing.apply_society(event), "second time does not");
    }
}

mod groups {
    use cena_model::state::character::snapshot::Group;

    #[test]
    fn standing_is_a_group_of_its_own() {
        // It is separately staleable: the others are only ever taught by a
        // command someone runs, and this one is also taught by ordinary play.
        // A character who joins a society mid-session has fresh standing and
        // stats as old as the last sync.
        assert!(Group::ALL.contains(&Group::Standing));
    }

    #[test]
    fn every_group_names_a_command_that_refreshes_it() {
        // The `ALL` list and the `refresh_command` match are two places the
        // same set is written. This is what stops them drifting -- the lesson
        // the frame variant-count test taught this month, where a third
        // hardcoded copy decided the result.
        for group in Group::ALL {
            assert!(
                !group.refresh_command().is_empty(),
                "{group:?} has no refresh command"
            );
        }
    }

    #[test]
    fn the_group_count_is_stated_and_checked() {
        // `Group::ALL`'s doc says "All six". A doc comment is a copy of a
        // number like any other; this is the command that proves it.
        assert_eq!(Group::ALL.len(), 6);
    }
}
/// Through the real parser, from the author's live wire bytes.
///
/// The tests above call the classifiers directly. These go through
/// `Parser` -> `GameState::apply` -> `close_chunk` -> `consume_standing`, which
/// is the only path that proves the wiring exists -- and the only one that
/// exercises markup. The author's `society` report arrives INDENTED AFTER a
/// `<popBold/>` on the same wire line, so a test built from hand-typed plain
/// text would pass while the real thing failed.
mod through_the_parser {
    use cena_model::GameState;
    use cena_model::state::character::standing::Standing;
    use cena_model::state::character::vocabulary::{ResourceType, Society};
    use cena_protocol::Parser;

    fn state_after(wire: &str) -> GameState {
        let mut parser = Parser::new();
        let mut state = GameState::default();
        for line in wire.lines() {
            for frame in parser.parse_line(line) {
                state.apply(&frame);
            }
        }
        state
    }

    /// The author's `society` output, verbatim including markup.
    const SOCIETY: &str = concat!(
        "<pushBold/>\n",
        "Current society status:\n",
        "<popBold/>   You are a Master of the Guardians of Sunfist.\n",
        "<pushBold/>\n",
        "You have learned and are able to use the following abilities:\n",
        "<popBold/>   <d>Sigil of Recognition</d>\n",
        "   <d>Sigil of Escape</d>\n",
        "<pushBold/>\n",
        "Past society affiliations (resigned or cast out):\n",
        "<popBold/>   You have no past society affiliations.\n",
        "You may view your society task information via the <d>SOCIETY TASK</d> command.\n",
        "<prompt time=\"1789941560\">&gt;</prompt>\n",
    );

    #[test]
    fn the_authors_society_report_reaches_the_character() {
        let state = state_after(SOCIETY);
        let standing = &state.character.standing;
        assert_eq!(
            standing.society,
            Some(Some(Society::GuardiansOfSunfist)),
            "the report line survives its <popBold/> prefix"
        );
        assert_eq!(
            standing.society_rank,
            Some(20),
            "`max_rank()` fills what the line does not carry"
        );
    }

    #[test]
    fn the_sigil_list_is_not_mistaken_for_anything() {
        // Twenty `<d>` links and a "Past society affiliations" section, none of
        // which states a fact. Lich has no pattern for the past-affiliations
        // section at all -- it is newer than its parser.
        let state = state_after(SOCIETY);
        assert_eq!(state.character.standing.citizenship, None);
        assert!(state.character.standing.warcries.is_empty());
    }

    /// The author's `resource` output, verbatim.
    const RESOURCE: &str = concat!(
        "<output class=\"mono\"/>\n",
        "Health: 192/<pushBold/>193<popBold/>     Mana: 405/405     ",
        "Stamina: 158/<pushBold/>158<popBold/>     Spirit: 10/10\n",
        "Nature's Grace: 0/50,000 (Weekly)     200,000/200,000 (Total)\n",
        "Suffused Nature's Grace: 1\n",
        "Covert Arts Charges: 164/200\n",
        "<output class=\"\"/>\n",
        "<prompt time=\"1789941593\">&gt;</prompt>\n",
    );

    #[test]
    fn the_authors_resource_report_reaches_the_character() {
        let state = state_after(RESOURCE);
        let standing = &state.character.standing;
        assert_eq!(standing.resource_type, Some(ResourceType::NaturesGrace));
        assert_eq!(standing.suffused, Some(1), "`resources.suffused : 1`");
        assert_eq!(
            standing.resources.map(|r| r.total),
            Some(200_000),
            "`resources.total : 200000`"
        );
        assert_eq!(
            standing.resources.map(|r| r.weekly),
            Some(0),
            "sent as 0, and hidden by `;infomon show`'s zero filter"
        );
        assert_eq!(
            standing.covert_arts_charges,
            Some(164),
            "`resources.covert_arts_charges : 164`"
        );
    }

    #[test]
    fn the_vitals_line_in_that_report_teaches_no_resource() {
        // `Health: 192/193 ...` is in the same block and is not this layer's
        // business -- the wire already sends vitals typed.
        let state = state_after(concat!(
            "<output class=\"mono\"/>\n",
            "Health: 192/193     Mana: 405/405     Stamina: 158/158     Spirit: 10/10\n",
            "<prompt time=\"1\">&gt;</prompt>\n",
        ));
        assert_eq!(state.character.standing.resources, None);
        assert_eq!(state.character.standing.resource_type, None);
    }

    #[test]
    fn a_bare_feat_usage_block_teaches_nothing() {
        // The adversarial case, through the parser this time. Both a bare
        // `feat` and `feat list` open with `<output class="mono"/>`, which is a
        // FONT SWITCH and not a block boundary.
        let state = state_after(concat!(
            "<output class=\"mono\"/>\n",
            "USAGE: FEAT {feat} {target} or\n",
            "  <d cmd='feat learn'>LEARN</d> {feat}              - Spend Feat Training Points for feat\n",
            "Note: {feat} references above require use of the feat mnemonic.\n",
            "<output class=\"\"/>\n",
            "<prompt time=\"1\">&gt;</prompt>\n",
        ));
        assert_eq!(state.character.standing, Standing::default());
    }
}
