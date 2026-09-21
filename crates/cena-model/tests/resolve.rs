//! Turning a name into an item the game will accept.
//!
//! The pure half of `lib/stash.rb`: which item does `"long bow"` mean, given
//! what the character is holding, wearing and carrying?

use cena_model::{GameState, Held, Location, Match, Specificity};
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

fn hand(side: &str, id: &str, noun: &str, text: &str) -> String {
    format!("<{side} exist=\"{id}\" noun=\"{noun}\">{text}</{side}>")
}

/// A container whose window is open, as `<container>` / `<inv>` sends it.
fn container(id: &str, items: &[(&str, &str, &str)]) -> String {
    let mut rows = String::new();
    for (iid, noun, text) in items {
        use std::fmt::Write as _;
        let _ = write!(
            rows,
            "<inv id='{id}'><a exist=\"{iid}\" noun=\"{noun}\">{text}</a></inv>"
        );
    }
    format!("<container id='{id}' title='a pack'></container>{rows}")
}

mod matching {
    use cena_model::{Specificity, match_of};

    #[test]
    fn the_whole_name_is_the_best_match() {
        assert_eq!(
            match_of("a glowbark long bow", "a glowbark long bow"),
            Some(Specificity::Exact)
        );
    }

    #[test]
    fn whole_words_inside_the_name_are_a_phrase() {
        assert_eq!(
            match_of("a scorched glowbark long bow", "long bow"),
            Some(Specificity::Phrase)
        );
    }

    #[test]
    fn a_partial_word_is_not_a_match() {
        // **The guard Lich writes as `(?:\A|\s)...(?:\s|\z)`** (`stash.rb:587`).
        // Without it `"ow"` matches `"bow"`, and a behavior asked for an owl
        // feather reaches for a bow.
        assert_eq!(match_of("a long bow", "ow"), None);
        assert_eq!(match_of("a longbow", "long"), None, "not a word here");
    }

    #[test]
    fn words_in_order_but_apart_match_loosely() {
        // `name_matches?`'s second form (`stash.rb:610`): the first word,
        // anything, then the rest.
        assert_eq!(
            match_of("a scorched glowbark long bow", "glowbark bow"),
            Some(Specificity::Loose)
        );
    }

    #[test]
    fn words_out_of_order_do_not_match() {
        assert_eq!(match_of("a glowbark long bow", "bow glowbark"), None);
    }

    #[test]
    fn matching_ignores_case_and_surrounding_space() {
        assert_eq!(
            match_of("a Glowbark Long Bow", "  long bow  "),
            Some(Specificity::Phrase)
        );
    }

    #[test]
    fn an_empty_name_matches_nothing() {
        // Lich returns false for this (`stash.rb:607`). Without the guard an
        // empty profile field would match every item in inventory.
        assert_eq!(match_of("a long bow", ""), None);
        assert_eq!(match_of("a long bow", "   "), None);
    }

    #[test]
    fn a_name_with_regex_metacharacters_is_matched_literally() {
        // **NOT A PORT -- the bug this design removes.** `find_container`
        // (`stash.rb:13`) interpolates the caller's string into a regex, so a
        // bag named `pack (old)` raises `RegexpError` and takes the script
        // with it. Lich fixed this for items (`name_matches?` escapes) and
        // never for containers. Here nothing is compiled, so the question
        // does not arise.
        assert_eq!(
            match_of("a pack (old)", "pack (old)"),
            Some(Specificity::Phrase)
        );
        assert_eq!(match_of("a pack", "*"), None, "and it does not raise");
    }
}

mod ordering {
    use super::{Location, Specificity, container, hand, state_after};

    #[test]
    fn a_held_item_beats_one_in_a_container() {
        // `known_items_ranked` (`stash.rb:573`): hands rank 0, containers 3.
        // The comment at `:309` says why -- it is "the order the game itself
        // resolves a bare noun in".
        let state = state_after(&[
            &hand("right", "111", "bow", "a glowbark long bow"),
            &container("900", &[("222", "bow", "a ruic long bow")]),
        ]);
        let found = state.find_items("long bow");
        assert_eq!(found.len(), 2, "both match");
        assert_eq!(found[0].id, "111", "the one in hand first");
        assert_eq!(found[0].location, Location::Hand);
        assert_eq!(found[1].location, Location::Container);
    }

    #[test]
    fn specificity_outranks_location() {
        // An exact match in a container beats a loose match in hand. Lich
        // sorts on `[specificity, location, index]` in that order
        // (`stash.rb:311`), and this is the pair that proves which key leads.
        let state = state_after(&[
            &hand("right", "111", "bow", "a scorched glowbark long bow"),
            &container("900", &[("222", "bow", "long bow")]),
        ]);
        let found = state.find_items("long bow");
        assert_eq!(found[0].id, "222", "exact, though it is in a bag");
        assert_eq!(found[0].specificity, Specificity::Exact);
        assert_eq!(found[1].specificity, Specificity::Phrase);
    }

    #[test]
    fn the_ready_list_ranks_between_hands_and_containers() {
        let state = state_after(&[
            "Your current settings are:",
            concat!(
                "  weapon: <d cmd=\"store WEAPON clear\">a ",
                "<a exist=\"333\" noun=\"katar\">mithril katar</a></d> ",
                "(<d cmd='store set'>stowed</d>)"
            ),
            // A DIFFERENT name, deliberately. Two items called
            // `mithril katar` would collapse under the identical-names rule
            // and this would test that instead -- which is what the first
            // draft of this test accidentally did.
            &container("900", &[("444", "katar", "steel mithril katar")]),
        ]);
        let found = state.find_items("mithril katar");
        assert_eq!(found.len(), 2, "guard: both are candidates");
        assert_eq!(found[0].location, Location::Ready, "ready beats container");
        assert_eq!(found[0].id, "333");
    }
}

mod finding {
    use super::{GameState, Held, Match, container, hand, state_after};

    #[test]
    fn nothing_matching_resolves_to_nothing() {
        // **Not an error at this layer.** Lich's `find_item` raises
        // (`stash.rb:296`); here the caller decides what an unresolvable name
        // means, because a model answers questions and does not abort.
        let state = state_after(&[&hand("right", "111", "bow", "a long bow")]);
        assert_eq!(state.find_item("halberd"), None);
        assert!(state.find_items("halberd").is_empty());
    }

    #[test]
    fn a_name_resolves_to_the_id_a_command_targets() {
        let state = state_after(&[&hand("left", "157925365", "bow", "glowbark long bow")]);
        let found = state.find_item("long bow").expect("resolves");
        assert_eq!(found.id, "157925365", "not the name");
    }

    #[test]
    fn identical_names_collapse_to_one() {
        // Lich's `uniq(&:name)` (`stash.rb:315`), and its reason at `:307`:
        // items with identical names are interchangeable. A caller choosing
        // between two of them has no basis to choose.
        let state = state_after(&[&container(
            "900",
            &[
                ("111", "arrow", "a faewood arrow"),
                ("222", "arrow", "a faewood arrow"),
            ],
        )]);
        let found: Vec<Match> = state.find_items("faewood arrow");
        assert_eq!(found.len(), 1, "two ids, one name");
    }

    #[test]
    fn one_item_in_two_places_collapses_to_one() {
        // The ready list names the katar AND it is in hand. Same id, so it is
        // one item -- reported once, at its best location.
        let state = state_after(&[
            &hand("right", "333", "katar", "mithril katar"),
            "Your current settings are:",
            concat!(
                "  weapon: <d cmd=\"store WEAPON clear\">a ",
                "<a exist=\"333\" noun=\"katar\">mithril katar</a></d> ",
                "(<d cmd='store set'>stowed</d>)"
            ),
        ]);
        let found = state.find_items("mithril katar");
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].location, super::Location::Hand, "the better rank");
    }

    #[test]
    fn which_hand_holds_it_is_answered_by_id() {
        // **What the `Hand` fix bought.** Before hands carried `exist=`, this
        // could only compare names -- and two items with the same name are
        // exactly the case where the answer matters.
        let state = state_after(&[
            &hand("right", "111", "bow", "a long bow"),
            &hand("left", "222", "bow", "a long bow"),
        ]);
        assert_eq!(state.hand_holding("111"), Some(Held::Right));
        assert_eq!(state.hand_holding("222"), Some(Held::Left));
        assert_eq!(state.hand_holding("333"), None);
    }

    #[test]
    fn an_empty_hand_holds_nothing() {
        let state = state_after(&["<right>Empty</right>"]);
        assert_eq!(state.hand_holding("111"), None);
    }

    #[test]
    fn a_state_that_knows_nothing_resolves_nothing() {
        // The honest failure. Resolution reads only what the game has said,
        // so before it has said anything every name is unresolvable -- and a
        // caller needing certainty refreshes inventory first, which is a
        // behavior's job.
        assert_eq!(GameState::default().find_item("long bow"), None);
    }
}
