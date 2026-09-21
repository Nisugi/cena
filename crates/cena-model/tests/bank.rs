//! What the bank says you have.

use cena_model::GameState;
use cena_protocol::Parser;

fn state_after(lines: &[&str]) -> GameState {
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
    state
}

/// The author's own `bank account` output, copied from
/// `E:/Gemstone/dev/lich-5/logs`, `2026-09-01_15-12-27`.
///
/// Real wire, not a reconstruction from `bank.rb`'s regex -- which is the
/// point, because the regex and the wire disagree. See
/// [`the_bank_whose_name_does_not_end_in_bank`].
const LISTING: &[&str] = &[
    "You currently have the following amounts on deposit:",
    "",
    "<output class=\"mono\"/>",
    "     First Elanith Secured Bank: 586,836,811",
    "             Icemule Trace Bank: 3,800,267",
    "      Vornavis Bank of Solhaven: 10,627,060",
    "                Four Winds Bank: 425,802,149",
    "             Kraken's Fall Bank: 26,743,838",
    "                          Total: 1,053,810,125",
    "<output class=\"\"/>",
    "",
    "You currently have 0 inter-town bank transfer options available.",
    "",
    "You currently have 11 urchin bank runner uses remaining.",
];

#[test]
fn every_town_balance_is_read() {
    let bank = state_after(LISTING).bank;
    assert_eq!(bank.balances().len(), 5, "five banks in the listing");
    assert_eq!(bank.at("First Elanith Secured Bank"), Some(586_836_811));
    assert_eq!(bank.at("Kraken's Fall Bank"), Some(26_743_838));
}

#[test]
fn the_bank_whose_name_does_not_end_in_bank() {
    // **THE DEFECT THIS PORT FIXES**, verified against the author's own
    // account. `ACCOUNT_LINE` (`bank.rb:69`) is
    //
    //   /^\s+(?<bank>.+?) Bank: (?<silver>[\d,]+)$/
    //
    // which requires the name to END in `Bank`. `Vornavis Bank of Solhaven`
    // has it in the middle, so the row does not match and 10,627,060 silver
    // is invisible.
    //
    // The consequence is worse than a missing row. `balance` falls back to
    // `banks[local_bank(...)] || 0` (`bank.rb:125`), so a character standing
    // in Solhaven reads a balance of ZERO while holding 10.6 million, and a
    // deposit routine trusting that figure behaves as though the account were
    // empty.
    //
    // Here the separator is the LAST `": "`, which is what the fixed-width
    // layout actually guarantees. No bank name has to be any shape.
    let bank = state_after(LISTING).bank;
    assert_eq!(
        bank.at("Vornavis Bank of Solhaven"),
        Some(10_627_060),
        "the row Lich's pattern cannot see"
    );
}

#[test]
fn the_stated_total_is_read_not_summed() {
    // **Lich sums the rows it managed to parse** (`bank.rb:126`), which is
    // precisely why its dropped row goes unnoticed: the sum is
    // self-consistent and wrong. Reading the game's own figure makes the
    // disagreement expressible.
    let bank = state_after(LISTING).bank;
    assert_eq!(bank.total(), Some(1_053_810_125), "the game's figure");
    assert_eq!(bank.sum_of_balances(), 1_053_810_125, "and the rows agree");
    assert_eq!(bank.agrees(), Some(true));
}

#[test]
fn a_missed_row_is_detectable() {
    // The guard the stated total buys. Had a row been dropped -- as Lich
    // drops Vornavis -- the sum would disagree with the total and a consumer
    // could say so rather than quietly under-reporting.
    let bank = state_after(&[
        "You currently have the following amounts on deposit:",
        "                Four Winds Bank: 425,802,149",
        "                          Total: 1,053,810,125",
    ])
    .bank;
    assert_eq!(bank.agrees(), Some(false), "the listing does not add up");
}

#[test]
fn the_total_line_is_not_a_bank() {
    let bank = state_after(LISTING).bank;
    assert_eq!(bank.at("Total"), None, "Total is not a town");
    assert!(
        !bank.balances().iter().any(|b| b.bank == "Total"),
        "nor a row"
    );
}

#[test]
fn unindented_prose_with_a_colon_is_not_a_balance() {
    // The listing's rows are indented and the prose around them is not.
    //
    // **The first version of this test did not test that.** It asserted only
    // that the real listing yields 5 rows, and a mutation removing the
    // indentation guard left it GREEN -- because none of the real prose lines
    // happen to contain `": "`. That is `plan/05` section 0's decoration: a
    // guard with a written justification and nothing asserting it.
    //
    // What does reach it: a game message quoting a figure. The teller and the
    // bank runner both speak in this response's neighbourhood, and a line
    // like the one below has the exact shape of a balance row.
    let bank = state_after(&[
        "You currently have the following amounts on deposit:",
        "                Four Winds Bank: 425,802,149",
        "The teller says, \"Your limit is: 50,000\"",
        "Total: 9",
    ])
    .bank;
    assert_eq!(bank.balances().len(), 1, "only the indented row");
    assert_eq!(bank.at("Four Winds Bank"), Some(425_802_149));
    assert_eq!(
        bank.total(),
        None,
        "an unindented `Total:` is prose, not the listing's total"
    );

    // And the real listing still reads whole.
    assert_eq!(state_after(LISTING).bank.balances().len(), 5);
}

#[test]
fn a_bank_that_refuses_access_says_so() {
    let bank = state_after(&["You don't have access to an account here."]).bank;
    assert!(bank.no_access());
    assert!(bank.balances().is_empty());
}

#[test]
fn nothing_read_is_unknown_not_empty() {
    // §5.2. "No balances" and "nobody has asked" are different facts, and a
    // behavior must not read the second as an empty account.
    let bank = GameState::default().bank;
    assert!(!bank.is_known());
    assert!(bank.balances().is_empty());
    assert_eq!(bank.total(), None);
}

#[test]
fn a_second_listing_replaces_the_first() {
    let mut state = state_after(LISTING);
    assert_eq!(state.bank.balances().len(), 5, "guard: known first");
    let mut parser = Parser::new();
    for line in [
        "You currently have the following amounts on deposit:",
        "                Four Winds Bank: 1,000",
        "                          Total: 1,000",
    ] {
        for frame in parser.parse_line(line) {
            state.apply(&frame);
        }
    }
    for frame in parser.parse_line("<prompt time=\"2\">&gt;</prompt>") {
        state.apply(&frame);
    }
    assert_eq!(state.bank.balances().len(), 1, "the old rows are gone");
    assert_eq!(state.bank.at("First Elanith Secured Bank"), None);
}

mod local {
    use super::{LISTING, state_after};

    #[test]
    fn the_town_you_are_in_names_its_bank() {
        // `local_bank` (`bank.rb:139`): an exact prefix first.
        let bank = state_after(LISTING).bank;
        assert_eq!(
            bank.local("Four Winds Isle").map(|b| b.silver),
            Some(425_802_149)
        );
    }

    #[test]
    fn a_location_inside_the_bank_name_still_matches() {
        // Lich's second pass, containment either way.
        let bank = state_after(LISTING).bank;
        assert_eq!(
            bank.local("Solhaven").map(|b| b.bank.as_str()),
            Some("Vornavis Bank of Solhaven"),
            "and this is the bank Lich could not see at all"
        );
    }

    #[test]
    fn an_unknown_town_names_no_bank() {
        let bank = state_after(LISTING).bank;
        assert!(bank.local("Teras Isle").is_none());
    }

    #[test]
    fn an_empty_location_names_no_bank() {
        // Lich guards this because `Map#location` can be `false`, and
        // `false.to_s` is the non-empty string `"false"` -- which matches no
        // bank but is not empty either (`bank.rb:140`). Here a non-string
        // cannot be passed, so only the empty case needs the guard.
        let bank = state_after(LISTING).bank;
        assert!(bank.local("").is_none());
        assert!(bank.local("   ").is_none());
    }
}

mod notes {
    use cena_model::note_line;
    use cena_protocol::Parser;

    /// Real wire, `2026-09-04_00-35-44`.
    const READ: &str = concat!(
        r#"The Mist Harbor <a exist="46215417" noun="note">promissory note</a> "#,
        r#"has a value of 42,170 silver and reads, "Hold in right hand to use.""#,
    );

    /// The line as the chunk assembler would hand it to a classifier: one
    /// `ChunkLine` over every run the parser emitted.
    fn line(wire: &str) -> cena_model::ChunkLine {
        let mut parser = Parser::new();
        let mut runs = Vec::new();
        for frame in parser.parse_line(wire) {
            if let cena_protocol::frame::Frame::Text(text) = frame {
                runs.push(text.as_run());
            }
        }
        cena_model::ChunkLine {
            runs: cena_protocol::runs::Runs { runs },
        }
    }

    #[test]
    fn a_notes_value_and_id_are_both_read() {
        // Lich's `NOTE_VALUE` (`bank.rb:75`) captures only the figure and
        // discards the link. The id is what a later `deposit note` targets,
        // and the corpus shows it is always there.
        let note = note_line(&line(READ)).expect("a note");
        assert_eq!(note.silver, 42_170);
        assert_eq!(note.id.as_deref(), Some("46215417"));
    }

    #[test]
    fn an_unrelated_value_line_is_not_a_note() {
        let wire = "The gem has a value of 500 silver, the merchant says.";
        assert!(note_line(&line(wire)).is_none(), "no `and reads`");
    }
}

#[test]
fn the_balance_survives_a_reconnect() {
    // Silver on deposit is not connection state -- nobody spends it while we
    // are logged off -- and only a `bank account` command re-reads it, never
    // the login burst. Clearing would leave a behavior believing the account
    // empty with no event coming to correct it.
    let mut state = state_after(LISTING);
    assert_eq!(state.bank.balances().len(), 5, "guard: known first");
    state.invalidate_for_reconnect();
    assert_eq!(state.bank.at("Four Winds Bank"), Some(425_802_149));
}
