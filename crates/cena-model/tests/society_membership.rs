//! Society membership lines, and the two Lich bugs this port does not inherit.
//!
//! The wire strings below are the patterns' own literals from
//! `reference/lich-5/lib/gemstone/infomon/parser.rb:38-42`, which is the only
//! record of this messaging anywhere in the reference -- there is no spec
//! corpus for society parsing, unlike bounty. Where a line is reconstructed
//! from a pattern rather than observed, the test says so.

use cena_model::Society;
use cena_model::state::societies::membership::MembershipLine;

/// The standing line, with a rank.
///
/// `parser.rb:38`. Both `rank` and `step` appear in the alternation, because
/// Voln calls its ranks steps.
#[test]
fn a_standing_line_gives_society_and_rank() {
    assert_eq!(
        MembershipLine::classify("   You are a member in the Order of Voln at step 15."),
        Some(MembershipLine::Standing {
            society: Society::OrderOfVoln,
            rank: Some(15),
            master: false,
        })
    );
    assert_eq!(
        MembershipLine::classify("   You are a member of the Council of Light at rank 12."),
        Some(MembershipLine::Standing {
            society: Society::CouncilOfLight,
            rank: Some(12),
            master: false,
        })
    );
    assert_eq!(
        MembershipLine::classify("   You are a member of the Guardians of Sunfist at rank 3.")
            .and_then(|l| l.society()),
        Some(Society::GuardiansOfSunfist)
    );
}

/// **A Master's line carries no number.**
///
/// `parser.rb:38`'s rank group is optional and is absent for a Master; Lich
/// supplies the number from knowledge of the game (`parser.rb:400-407`). The
/// classifier keeps "the wire said 26" and "the wire said Master" distinct, and
/// [`MembershipLine::rank`] does the substitution in one place.
#[test]
fn a_master_has_no_stated_rank_and_gets_the_societys_maximum() {
    let voln = MembershipLine::classify("   You are a Master in the Order of Voln.")
        .expect("a standing line");
    assert_eq!(
        voln,
        MembershipLine::Standing {
            society: Society::OrderOfVoln,
            rank: None,
            master: true,
        },
        "the wire states no number"
    );
    assert_eq!(voln.rank(), Some(26), "but a Voln Master is rank 26");

    let council = MembershipLine::classify("   You are a Master of the Council of Light.")
        .expect("a standing line");
    assert_eq!(council.rank(), Some(20), "the other two cap at 20");

    let sunfist = MembershipLine::classify("   You are a Master of the Guardians of Sunfist.")
        .expect("a standing line");
    assert_eq!(sunfist.rank(), Some(20));
}

/// Not a member.
#[test]
fn the_no_society_line_is_recognised() {
    let line = MembershipLine::classify("   You are not a member of any society at this time.")
        .expect("a membership line");
    assert_eq!(line, MembershipLine::NoSociety);
    assert_eq!(line.rank(), Some(0));
    assert_eq!(line.society(), None, "it names no society, by definition");
}

/// **THE BUG: joining the Council of Light records nothing in Lich.**
///
/// `parser.rb:413-425` scans the matched line for `/Order|Council|Guardians/`
/// and then branches on `'Lodge'` for the Council. The word `Lodge` is in the
/// wire text -- the Poohbah says "Welcome to the Lodge" -- but the scan looks
/// for `Council`, which that line never contains, so the branch is unreachable
/// and a new Council member's status and rank are never written.
///
/// `inventory/10` §8. This asserts the fix directly: the Council join line
/// classifies, with the right society.
#[test]
fn joining_the_council_of_light_is_recorded() {
    let line = MembershipLine::classify(
        r#"The Grand Poohbah smiles broadly.  "Welcome to the Lodge," he cries as he pats you on the back."#,
    )
    .expect("the Council join line must classify");

    assert_eq!(
        line,
        MembershipLine::Joined {
            society: Society::CouncilOfLight,
            rank: 1,
        },
        "Lich reaches an unreachable branch here and records nothing"
    );

    // The line genuinely does not contain the word Lich scans for, which is
    // the whole mechanism of the bug.
    let wire = r#"The Grand Poohbah smiles broadly.  "Welcome to the Lodge," he cries"#;
    assert!(
        !wire.contains("Council"),
        "if this ever contains 'Council', Lich's scan would have worked"
    );
    assert!(wire.contains("Lodge"));
}

/// The other two joins, which Lich does record.
///
/// Asserted beside the Council so all three fail together if the shape breaks.
#[test]
fn the_other_two_joins_are_recorded_too() {
    assert_eq!(
        MembershipLine::classify(r#"The Grandmaster says, "Welcome to the Order of Voln.""#),
        Some(MembershipLine::Joined {
            society: Society::OrderOfVoln,
            rank: 1,
        })
    );
    assert_eq!(
        MembershipLine::classify(
            r#"The Grandmaster says, "You are now a member of the Guardians of Sunfist.""#
        ),
        Some(MembershipLine::Joined {
            society: Society::GuardiansOfSunfist,
            rank: 0,
        }),
        "a new Guardian starts at rank 0, not 1"
    );
}

/// **A new Guardian starts at rank 0 and the other two at rank 1.**
///
/// `parser.rb:417-421`. Worth asserting on its own: it looks like an
/// off-by-one and is not, and a rank-0 Guardian knows no sigils, which is
/// correct -- `Sigil of Recognition` is rank 1.
#[test]
fn sunfist_starts_a_rank_below_the_others() {
    let joins: Vec<(Society, u8)> = [
        r#"The Grandmaster says, "Welcome to the Order of Voln.""#,
        r#"The Grand Poohbah smiles broadly.  "Welcome to the Lodge," he cries"#,
        r#"The Grandmaster says, "You are now a member of the Guardians of Sunfist.""#,
    ]
    .iter()
    .filter_map(|line| match MembershipLine::classify(line) {
        Some(MembershipLine::Joined { society, rank }) => Some((society, rank)),
        _ => None,
    })
    .collect();

    assert_eq!(
        joins,
        [
            (Society::OrderOfVoln, 1),
            (Society::CouncilOfLight, 1),
            (Society::GuardiansOfSunfist, 0),
        ]
    );

    // And a rank-0 Guardian really does know nothing yet.
    assert!(
        !Society::GuardiansOfSunfist.abilities()[0].known_at(0),
        "Sigil of Recognition is rank 1"
    );
}

/// **THE SECOND BUG: advancement carries no number, deliberately.**
///
/// `parser.rb:428` is `Infomon.set('society.rank', Infomon.get('society.rank') + 1)`,
/// which raises when the stored rank is `nil` -- the state before anything has
/// taught Lich the character's rank.
///
/// [`MembershipLine::Advanced`] states *that* a rank was gained and not *which*
/// rank, so a consumer with no current rank cannot compute a wrong one. It also
/// names no society, because none of the nine phrasings does.
#[test]
fn advancement_states_a_change_not_a_value() {
    let line = MembershipLine::classify(
        "Zarak traces the outline of a sigil into the air before you and says, \"You have earned this.\"",
    )
    .expect("an advancement line");
    assert_eq!(line, MembershipLine::Advanced);
    assert_eq!(
        line.rank(),
        None,
        "a consumer that does not know the current rank must not invent one"
    );
    assert_eq!(
        line.society(),
        None,
        "none of the nine advancement phrasings names a society"
    );
}

/// Every advancement phrasing classifies.
///
/// Nine NPCs plus two impersonal forms (`parser.rb:40`). A dropped alternative
/// would silently stop recording advancement for one society.
#[test]
fn every_advancement_phrasing_is_recognised() {
    let npcs = [
        "Zarak", "Faylanna", "Draelox", "Marl", "Vindar", "Taryn", "Meaha", "Oxanna", "Cyndelle",
    ];
    for npc in npcs {
        let line = format!("{npc} traces the outline of a sigil into the air before you and says");
        assert_eq!(
            MembershipLine::classify(&line),
            Some(MembershipLine::Advanced),
            "{npc} advances a member"
        );
    }
    assert_eq!(
        MembershipLine::classify(
            "The High Taskmaster looks at you, consults her notes, and then announces in a loud voice"
        ),
        Some(MembershipLine::Advanced)
    );
    assert_eq!(
        MembershipLine::classify(
            "The High Taskmaster looks at you, consults his notes, and then announces in a loud voice"
        ),
        Some(MembershipLine::Advanced),
        "the Taskmaster has two spellings"
    );
    assert_eq!(
        MembershipLine::classify("The monk concludes ceremoniously, \"You are advanced.\""),
        Some(MembershipLine::Advanced)
    );
}

/// Every resignation phrasing classifies.
#[test]
fn resignation_ends_membership() {
    let lines = [
        r#"The Grandmaster says, "I'm sorry to hear that.  You are no longer in our service."#,
        r#"The Poohbah looks at you sternly.  "I had high hopes for you," he says, "but if this be your decision, so be it.  I hereby strip you of membership"#,
        r#"The Grandmaster says, "I'm sorry to hear that, and I wish you well with any of your future endeavors."#,
    ];
    for line in lines {
        let classified = MembershipLine::classify(line)
            .unwrap_or_else(|| panic!("should classify as a resignation: {line}"));
        assert_eq!(classified, MembershipLine::Resigned);
        assert_eq!(classified.rank(), Some(0), "resigning leaves no rank");
    }
}

/// Ordinary game text is not a membership line.
///
/// The advancement patterns are unanchored on the right and the resignation
/// ones match long prefixes, so this is where an over-broad pattern would show.
#[test]
fn ordinary_text_does_not_classify() {
    for line in [
        "",
        "You are a member of the local gardening club.",
        "The Grandmaster says, \"Hello.\"",
        "Zarak looks at you.",
        "You are not a member of any society at this time",
        "   You are a member in the Order of Voln at step fifteen.",
    ] {
        assert_eq!(
            MembershipLine::classify(line),
            None,
            "should not classify: {line:?}"
        );
    }
}
