//! The `experience` report, and the gift pulse it drives.
//!
//! # The numbers are the author's
//!
//! `;infomon show` for Nisugi, pasted 2026-09-20, is the answer key:
//!
//! ```text
//! experience.fame : 1453539090
//! experience.field_experience_max : 1403
//! experience.ascension_experience : 24865590
//! experience.total_experience : 68770511
//! experience.deaths_sting : "None"
//! experience.long_term_experience : 493
//! experience.deeds : 11
//! stat.experience : 43904921
//! ```
//!
//! Note what is NOT in that list: a plain `experience.` key and a recent-deaths
//! key. Lich matches both lines and captures neither
//! (`infomon/parser.rb:17-18`), so those two numbers exist on the wire and
//! nowhere in its store. Rule 2.2 says they are not dropped here.

use cena_model::GameState;
use cena_model::state::character::vocabulary::DeathsSting;
use cena_protocol::Parser;

/// The report as the game lays it out: two columns, `Label: value`.
///
/// Reconstructed from Lich's patterns (`parser.rb:16-21`) with the author's
/// own values, because a capture of the command itself was not to hand. The
/// column widths are what those patterns require -- `\s+` before each label
/// and a run of spaces between the columns.
const REPORT: &str = concat!(
    "          Level: 100                         Fame: 1,453,539,090\n",
    "     Experience: 43,904,921             Field Exp: 1,234/1,403\n",
    "  Ascension Exp: 24,865,590         Recent Deaths: 0\n",
    "      Total Exp: 68,770,511          Death's Sting: None\n",
    "  Long-Term Exp: 493                        Deeds: 11\n",
    "  Exp until lvl: 11,999,265\n",
    "<prompt time=\"1\">&gt;</prompt>\n",
);

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

#[test]
fn the_authors_numbers_all_arrive() {
    let exp = state_after(REPORT).character.experience;
    assert_eq!(exp.fame, Some(1_453_539_090), "past i32, hence i64");
    assert_eq!(exp.field_experience, Some(1_234));
    assert_eq!(exp.field_experience_max, Some(1_403));
    assert_eq!(exp.ascension_experience, Some(24_865_590));
    assert_eq!(exp.total_experience, Some(68_770_511));
    assert_eq!(exp.long_term_experience, Some(493));
    assert_eq!(exp.deeds, Some(11));
    assert_eq!(exp.deaths_sting, Some(DeathsSting::None));
}

#[test]
fn the_two_numbers_lich_discards_are_read() {
    // Rule 2.2. `;infomon show` has no key for either, because
    // `parser.rb:17-18` match the lines and capture the other column.
    let exp = state_after(REPORT).character.experience;
    assert_eq!(
        exp.experience,
        Some(43_904_921),
        "the plain total, beside Field Exp"
    );
    assert_eq!(exp.recent_deaths, Some(0), "beside Ascension Exp");
}

#[test]
fn exp_until_level_is_deliberately_not_stored() {
    // The one number this drops, with a reason rather than by oversight: it is
    // the remainder to the next level, which `<progressBar id='nextLvlPB'>`
    // carries continuously. Two answers to one question, the stale one looking
    // authoritative, is worse than one.
    let state = state_after(REPORT);
    assert_eq!(
        state.character.experience.next_level, None,
        "the report does not fill the live bar's field"
    );
}

#[test]
fn a_chunk_without_a_fame_line_is_not_a_report() {
    // `Fame:` is Lich's own opener -- `parser.rb:16` comments that it "serves
    // as ExprStart". Without it, a player typing `Deeds: 11` into a channel
    // would rewrite the character's deeds.
    let state = state_after("Deeds: 11\n<prompt time=\"1\">&gt;</prompt>\n");
    assert_eq!(state.character.experience.deeds, None);
}

#[test]
fn a_partial_report_fills_only_what_it_states() {
    // A character with no ascension experience has no `Ascension Exp:` line.
    // The rest must still be read, and the absent field must stay unknown
    // rather than becoming zero (§5.2).
    let state = state_after(concat!(
        "          Level: 5                          Fame: 1,000\n",
        "      Total Exp: 2,000            Death's Sting: None\n",
        "<prompt time=\"1\">&gt;</prompt>\n",
    ));
    let exp = state.character.experience;
    assert_eq!(exp.fame, Some(1_000));
    assert_eq!(exp.total_experience, Some(2_000));
    assert_eq!(exp.ascension_experience, None, "absent, not zero");
    assert_eq!(exp.deeds, None);
}

#[test]
fn negative_fame_survives() {
    // Lich captures `-?[\d,]+` (`parser.rb:16`), so the wire can send one. An
    // unsigned field would fail to parse and store nothing -- the defect the
    // creature-health and covert-arts work both found this month.
    let state = state_after(concat!(
        "          Level: 1                          Fame: -500\n",
        "<prompt time=\"1\">&gt;</prompt>\n",
    ));
    assert_eq!(state.character.experience.fame, Some(-500));
}

#[test]
fn a_label_inside_a_longer_one_is_not_matched() {
    // MUTATION-FOUND. This first asserted only that `Total Exp` and
    // `Ascension Exp` read correctly, and removing the word-boundary guard
    // left it green -- because searching for `"Total Exp"` cannot collide with
    // anything in this report.
    //
    // The collision that IS real: `Experience` contains `Exp`. So a label that
    // is a prefix of a longer one, searched for on a line carrying the longer
    // one, must not match. `Deeds` on a `Recent Deaths` line is the same shape
    // and is the one the report actually contains.
    let exp = state_after(concat!(
        "          Level: 5                          Fame: 1,000
",
        "  Ascension Exp: 24,865,590         Recent Deaths: 7
",
        "<prompt time=\"1\">&gt;</prompt>
",
    ))
    .character
    .experience;
    assert_eq!(exp.recent_deaths, Some(7));
    assert_eq!(
        exp.deeds, None,
        "`Deeds` must not match inside `Recent Deaths`"
    );

    // And the original assertions, which are still worth keeping.
    let exp = state_after(REPORT).character.experience;
    assert_eq!(
        exp.total_experience,
        Some(68_770_511),
        "Total Exp, not Exp until lvl"
    );
    assert_eq!(exp.ascension_experience, Some(24_865_590));
}

mod gift {
    use super::state_after;

    /// One `expr` dialog carrying the experience bar.
    fn bar(text: &str) -> String {
        format!(
            "<dialogData id='expr'><progressBar id='nextLvlPB' value='50' text='{text}'/></dialogData>\n"
        )
    }

    #[test]
    fn a_changed_bar_is_one_pulse() {
        // Lich's `Gift.pulse` (`common/xmlparser.rb:751`), which is the ONLY
        // part of its gift tracker with a live caller.
        let state = state_after(&format!(
            "{}{}",
            bar("100 experience"),
            bar("90 experience")
        ));
        assert_eq!(state.character.experience.gift.pulses, 2);
    }

    #[test]
    fn an_unchanged_bar_is_not_a_pulse() {
        // The `unless @next_level_text == attributes['text']` guard. The dialog
        // is re-sent constantly, so counting every arrival would tick several
        // times a second and the count would mean nothing.
        let state = state_after(&format!(
            "{}{}{}",
            bar("100 experience"),
            bar("100 experience"),
            bar("100 experience"),
        ));
        assert_eq!(state.character.experience.gift.pulses, 1, "one change");
    }

    #[test]
    fn nothing_claims_to_know_how_long_is_left() {
        // Lich reports `(360 - pulses)` minutes. MEASURED, its counter has no
        // live reset: `started`, `ended` and the serialization are called only
        // from its own specs, so the count begins at zero every launch and the
        // remainder is right only for someone who started Lich exactly when
        // their gift began. The pulse count is the fact; the minutes are not.
        let state = state_after(&bar("100 experience"));
        assert_eq!(state.character.experience.gift.pulses, 1);
    }
}
