//! The six combat stances.
//!
//! The wire lines here are MEASURED from the 208 live Lich XML logs: four
//! distinct `(value, text)` pairs across 19,526 `pbarStance` readings.

use cena_model::state::character::stance::Stance;

#[test]
fn the_four_readings_the_corpus_actually_contains() {
    // MEASURED, with counts: offensive(0%) x8,466, guarded(80%) x1,656,
    // defensive(99%) x2, defensive(100%) x9,402. Nothing else was ever sent.
    for (text, stance, percent) in [
        ("offensive (0%)", Stance::Offensive, 0),
        ("guarded (80%)", Stance::Guarded, 80),
        ("defensive (99%)", Stance::Defensive, 99),
        ("defensive (100%)", Stance::Defensive, 100),
    ] {
        assert_eq!(
            Stance::parse_bar_text(text),
            Some((stance, percent)),
            "{text}"
        );
    }
}

#[test]
fn a_percent_that_is_not_a_multiple_of_ten_still_reads() {
    // **THE CASE THAT SEPARATES READING FROM SETTING.** Lich's
    // `normalize_percent` (stance.rb:163-170) RAISES unless the percent is a
    // multiple of ten -- a rule about what you may ASK FOR. Porting it into
    // the reader would reject a real game state.
    //
    // MEASURED: `defensive (99%)` occurs, and both readings are the same
    // moment in combat -- "a gold-bristled hinterboar feints to the left. You
    // spot the ruse too late as you move to block". A game event can knock
    // the stance off a round number.
    assert_eq!(Stance::from_percent(99), Some(Stance::Defensive));
    assert_eq!(Stance::from_percent(7), Some(Stance::Advance));
    assert_eq!(Stance::from_percent(41), Some(Stance::Neutral));
}

#[test]
fn the_bands_cover_every_percent_exactly_once() {
    // Contiguous and total over 0..=100, asserted rather than trusted.
    //
    // A GAP would make some real game state nameless; an OVERLAP would make
    // `from_percent` depend on declaration order, which is the kind of
    // accident that survives every hand-written test.
    for percent in 0..=100u32 {
        let matches: Vec<_> = Stance::ALL
            .into_iter()
            .filter(|s| {
                let (low, high) = s.band();
                (low..=high).contains(&percent)
            })
            .collect();
        assert_eq!(
            matches.len(),
            1,
            "{percent}% is in {matches:?}, want exactly one"
        );
    }
}

#[test]
fn above_one_hundred_is_not_a_stance() {
    // The game has never sent it. A protocol change rather than a stance, so
    // None rather than clamping to defensive -- §5.2, absent is not a value.
    assert_eq!(Stance::from_percent(101), None);
    assert_eq!(Stance::from_percent(u32::MAX), None);
}

#[test]
fn offensive_is_a_single_point_not_a_range() {
    // `'offensive' => (0..0)` (stance.rb:32). Zero percent to defense is the
    // only offensive reading, and 1% is already advance.
    assert_eq!(Stance::from_percent(0), Some(Stance::Offensive));
    assert_eq!(Stance::from_percent(1), Some(Stance::Advance));
    assert_eq!(Stance::Offensive.band(), (0, 0));
}

mod names_and_prefixes {
    use super::Stance;

    #[test]
    fn the_six_prefixes_scripts_actually_type() {
        for (prefix, expected) in [
            ("off", Stance::Offensive),
            ("adv", Stance::Advance),
            ("for", Stance::Forward),
            ("neu", Stance::Neutral),
            ("gua", Stance::Guarded),
            ("def", Stance::Defensive),
        ] {
            assert_eq!(Stance::parse(prefix), Some(expected), "{prefix}");
        }
    }

    #[test]
    fn fewer_than_three_characters_is_not_a_stance() {
        // Lich's floor (stance.rb:104), and it is load-bearing rather than
        // arbitrary: "o" matches only `offensive` today, but a stance added
        // later would silently change what a one-letter abbreviation means.
        assert_eq!(Stance::parse("o"), None);
        assert_eq!(Stance::parse("de"), None);
        assert_eq!(Stance::parse(""), None);
    }

    #[test]
    fn case_and_surrounding_space_do_not_matter() {
        assert_eq!(Stance::parse("  DEFENSIVE "), Some(Stance::Defensive));
        assert_eq!(Stance::parse("Guarded"), Some(Stance::Guarded));
    }

    #[test]
    fn a_word_that_is_not_a_stance_is_none() {
        assert_eq!(Stance::parse("aggressive"), None);
        assert_eq!(Stance::parse("defensively"), None, "longer than the name");
    }

    #[test]
    fn every_stance_round_trips_through_its_name() {
        for stance in Stance::ALL {
            assert_eq!(Stance::parse(stance.as_str()), Some(stance), "{stance}");
        }
    }
}

mod malformed_bar_text {
    use super::Stance;

    #[test]
    fn text_without_a_percent_is_not_read() {
        assert_eq!(Stance::parse_bar_text("defensive"), None);
        assert_eq!(Stance::parse_bar_text("defensive (100)"), None);
        assert_eq!(Stance::parse_bar_text("defensive 100%"), None);
    }

    #[test]
    fn a_percent_that_is_not_a_number_is_not_read() {
        assert_eq!(Stance::parse_bar_text("defensive (lots%)"), None);
        assert_eq!(Stance::parse_bar_text("defensive (-5%)"), None);
    }

    #[test]
    fn an_unknown_stance_name_is_not_read() {
        // Rule 2.2's shape: a name the game adds later must not arrive as
        // some existing stance.
        assert_eq!(Stance::parse_bar_text("berserk (50%)"), None);
    }
}

#[test]
fn the_table_is_complete_and_ordered_most_offensive_first() {
    assert_eq!(Stance::ALL.len(), 6, "stance.rb NAMES lists six");

    // The order is the one Lich lists and the one the bands ascend in --
    // asserted so a reordering that broke `from_percent`'s scan shows up
    // here rather than as a wrong stance in play.
    let mut previous = None;
    for stance in Stance::ALL {
        let (low, high) = stance.band();
        assert!(low <= high, "{stance} has an inverted band");
        if let Some(last_high) = previous {
            assert_eq!(
                low,
                last_high + 1,
                "{stance} does not follow the previous band"
            );
        }
        previous = Some(high);
    }
    assert_eq!(previous, Some(100), "the last band must reach 100");
}

mod through_the_parser {
    //! The typed accessor over the real wire, not over a hand-built struct.
    use cena_model::GameState;
    use cena_model::state::character::stance::Stance;
    use cena_protocol::Parser;

    fn state_after(lines: &[&str]) -> GameState {
        let mut parser = Parser::new();
        let mut state = GameState::default();
        for line in lines {
            for frame in parser.parse_line(line) {
                state.apply(&frame);
            }
        }
        state
    }

    #[test]
    fn a_real_stance_bar_reaches_the_typed_accessor() {
        // Verbatim wire, `character.rs:25`.
        let state = state_after(&[
            "<dialogData id='stance'><progressBar id='pbarStance' value='100' text='defensive (100%)'/></dialogData>",
        ]);
        assert_eq!(state.character.stance_typed(), Some(Stance::Defensive));
        assert_eq!(state.character.stance_percent, Some(100));
        assert_eq!(
            state.character.stance.as_deref(),
            Some("defensive (100%)"),
            "the raw string is kept beside the typed value"
        );
    }

    #[test]
    fn the_ninety_nine_percent_reading_from_the_corpus() {
        // MEASURED: this exact reading occurs twice, mid-combat. A reader that
        // accepted only multiples of ten would lose it.
        let state = state_after(&[
            "<dialogData id='stance'><progressBar id='pbarStance' value='99' text='defensive (99%)'/></dialogData>",
        ]);
        assert_eq!(state.character.stance_typed(), Some(Stance::Defensive));
        assert_eq!(state.character.stance_percent, Some(99));
    }

    #[test]
    fn an_unknown_stance_name_keeps_its_raw_string() {
        // **Rule 2.2.** A stance the game adds later cannot be named by the
        // enum, and must still reach a display. The typed accessor falls back
        // to the percent rather than inventing a name -- and the raw text is
        // still there for whoever can show it.
        let state = state_after(&[
            "<dialogData id='stance'><progressBar id='pbarStance' value='50' text='berserk (50%)'/></dialogData>",
        ]);
        assert_eq!(
            state.character.stance.as_deref(),
            Some("berserk (50%)"),
            "the unknown name survives"
        );
        assert_eq!(
            state.character.stance_typed(),
            Some(Stance::Neutral),
            "the percent still bands, because 50% IS neutral whatever it is called"
        );
    }

    #[test]
    fn nothing_reported_is_none_rather_than_a_default_stance() {
        // Section 5.2: absent is not a value. A character nobody has told us
        // about is not standing offensive.
        let state = GameState::default();
        assert_eq!(state.character.stance_typed(), None);
        assert_eq!(state.character.stance, None);
    }
}
