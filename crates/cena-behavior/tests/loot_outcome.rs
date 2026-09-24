//! What the game said back to a loot command, read as a closed set
//! (`plan/31` §2b). The search lines are real, from the replay fixtures;
//! the rest are eloot's own patterns (`eloot.lic:541-580`).

use cena_behavior::loot::{Outcome, classify};

#[test]
fn the_search_lines_the_fixtures_hold() {
    // `smithy_kill.xml:253`, the text of the line without its links.
    assert_eq!(
        classify("You search the armor-clad pegasus."),
        Some(Outcome::Searched)
    );
    assert_eq!(
        classify("You plunge your hand into the ooze."),
        Some(Outcome::Searched)
    );
    assert_eq!(
        classify("It didn't carry any silver."),
        None,
        "the lines after a search say nothing the planner acts on"
    );
}

#[test]
fn every_phrase_eloot_acts_on_has_an_outcome() {
    let cases = [
        (
            "You are not in any condition to do that.",
            Outcome::NotInCondition,
        ),
        ("There is no loot here.", Outcome::NothingHere),
        ("You need a free hand to do that.", Outcome::NeedFreeHand),
        ("The emerald won't fit in the sack.", Outcome::WontFit),
        (
            "You can't put that in the pack.  It's closed!",
            Outcome::Closed,
        ),
        ("That is closed.", Outcome::Closed),
        (
            "The lyre crumbles and decays away as you touch it.",
            Outcome::Crumbled,
        ),
        (
            "The dagger crumbles into a pile of dust.",
            Outcome::Crumbled,
        ),
        ("That is not yours to take.", Outcome::NotYours),
        ("Hey, that belongs to Ashryn!", Outcome::NotYours),
        (
            "You can't put something that you can't hold into a container.",
            Outcome::Unlootable,
        ),
        (
            "I could not find what you were referring to.",
            Outcome::NotFound,
        ),
        ("Get what?", Outcome::NotFound),
        (
            "You gather up and stow as much treasure as you can manage, but there is more than you can carry.",
            Outcome::TooMuch,
        ),
        (
            "You put a small emerald in your leather sack.",
            Outcome::Stored,
        ),
    ];
    for (line, expect) in cases {
        assert_eq!(classify(line), Some(expect), "{line}");
    }
}

#[test]
fn ordinary_game_text_is_not_an_outcome() {
    for line in [
        "A tawny armor-clad pegasus soars in.",
        "You feel fully energetic again.",
        "Roundtime: 2 sec.",
        "",
    ] {
        assert_eq!(classify(line), None, "{line:?}");
    }
}
