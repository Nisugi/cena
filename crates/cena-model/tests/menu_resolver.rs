//! A `<menu>` of bare coordinates becomes labelled, sendable commands.
//!
//! The wire answers `_menu #<exist id>` with coordinates and **no text at
//! all** -- MEASURED over all 425 `<mi>` in the author's September logs, the
//! only attributes are `coord`, `noun` and `menu_cat`, and **zero** carry a
//! label or command. Every label and every command comes from a dictionary
//! the client ships (`data/menu_commands.tsv`, 1,106 entries), so without
//! this resolution a context menu is a list of numbers and there is no
//! fallback.
//!
//! Resolution lives in the model because every frontend needs the same
//! answer: what a frontend owns is presentation, not what "2524,1543" means.

use cena_model::MenuCommands;
use cena_protocol::{Frame, Parser};

/// A real menu, trimmed from the author's 2026-09-20 log
/// (`2026-09-20_12-19-45.xml`, 60 items). Kept verbatim in shape: the
/// envelope's `cat_list`, coordinates with no labels, and one coordinate
/// (`2524,2564`) that the shipped dictionary does not know.
const REAL_MENU: &str = concat!(
    r#"<menu id="1" path="" cat_list="1 2 3 4 5 6 7 8 9 10 11 12 13">"#,
    r#"<mi coord="2524,1543"/>"#, // attack  -> cat 6
    r#"<mi coord="2524,1613"/>"#, // look at -> cat 1
    r#"<mi coord="9999,9999"/>"#, // no such coordinate, in any edition
    r#"<mi coord="2524,1541"/>"#, // ambush  -> cat 6
    r#"</menu>"#,
);

/// The exist id of whatever was right-clicked. **The caller's**, not the
/// menu's -- see `a_menu_id_is_a_request_number_not_an_object`.
const EXIST: &str = "121654846";

/// Every menu frame the line produced. `cena-model`'s tests ban `panic!`, so
/// a test asserts on the count rather than unwrapping.
fn menus(line: &str) -> Vec<cena_protocol::Menu> {
    Parser::new()
        .parse_line(line)
        .into_iter()
        .filter_map(|f| match f {
            Frame::MenuResponse(m) => Some(m),
            _ => None,
        })
        .collect()
}

/// The menu the line produced, or an empty one when it produced none.
fn menu(line: &str) -> cena_protocol::Menu {
    menus(line).pop().unwrap_or_default()
}

#[test]
fn the_dictionary_is_the_live_clients_copy() {
    // 1,106 entries, from `%APPDATA%/Wrayth/GS4/cmdlist1.xml`
    // (`timestamp="1788300900.1.1.1"`, 2026-09-05) -- NOT the 592-entry copy
    // `VellumFE` ships, whose timestamp is from 2003.
    //
    // The corpus caught the difference: 8 of the 60 coordinates in a real
    // menu were missing from the old file, every one above `2524,2500`.
    assert_eq!(MenuCommands::get().len(), 1106);
}

#[test]
fn the_coordinates_the_2003_file_lacked_now_resolve() {
    // UCS attacks, focused multistrike and `symbol of sleep`: commands the
    // shipped-with-Vellum file predates by fifteen years.
    let dict = MenuCommands::get();
    let missing = [
        ("2524,2564", "jab @", "6"),
        ("2524,2565", "grapple @", "6"),
        ("2524,2619", "multistrike (jab)", "6_focused multistrike"),
        ("2524,2567", "symbol of sleep @", "6_voln"),
    ];
    for (coord, label, category) in missing {
        let entry = dict.entry(coord).expect(coord);
        assert_eq!(entry.label, label);
        assert_eq!(entry.category, category);
    }
}

mod depth {
    use super::*;
    use cena_model::category_path;

    #[test]
    fn a_category_splits_into_its_menu_path() {
        assert_eq!(category_path("6"), ["6"]);
        assert_eq!(
            category_path("6_focused multistrike"),
            ["6", "focused multistrike"]
        );
        assert_eq!(
            category_path("5_roleplay-swear"),
            ["5", "roleplay", "swear"]
        );
    }

    #[test]
    fn the_menu_is_three_levels_deep_at_most() {
        // MEASURED across all 1,106 entries: 634 at depth 1, 453 at depth 2,
        // and 19 at depth 3 -- all nineteen of them `5_roleplay-swear`.
        let dict = MenuCommands::get();
        let mut counts = std::collections::BTreeMap::new();
        for coord in dict.coords() {
            if let Some(entry) = dict.entry(coord) {
                *counts.entry(entry.category_path().len()).or_insert(0usize) += 1;
            }
        }
        assert_eq!(counts.get(&1), Some(&634));
        assert_eq!(counts.get(&2), Some(&453));
        assert_eq!(counts.get(&3), Some(&19));
        assert_eq!(counts.keys().max(), Some(&3), "nothing goes deeper");
    }

    #[test]
    fn swearing_is_the_only_third_level_branch() {
        let dict = MenuCommands::get();
        let deep: std::collections::BTreeSet<&str> = dict
            .coords()
            .filter_map(|c| dict.entry(c))
            .filter(|e| e.category_path().len() == 3)
            .map(|e| e.category.as_str())
            .collect();
        assert_eq!(deep.into_iter().collect::<Vec<_>>(), ["5_roleplay-swear"]);
    }
}

#[test]
fn a_scrambled_source_row_is_kept_verbatim() {
    // `2524,2014` reads `menu="7" command="7" menu_cat="drag"` in
    // Simutronics' own file, and is UNCHANGED in the 2026 copy -- so it is
    // not staleness. Correcting source data on inference is how a silent
    // divergence from the game starts, so it is carried as-is.
    let entry = MenuCommands::get()
        .entry("2524,2014")
        .expect("the drag row");
    assert_eq!(
        (entry.label.as_str(), entry.category.as_str()),
        ("7", "drag")
    );
}

#[test]
fn a_coordinate_becomes_a_label_and_a_command() {
    let dict = MenuCommands::get();
    let entry = dict.entry("2524,1543").expect("attack");
    assert_eq!(entry.label, "attack @");
    assert_eq!(entry.command, "attack #");
    assert_eq!(entry.category, "6");
}

mod substitution {
    use super::*;

    #[test]
    fn at_becomes_the_noun() {
        let dict = MenuCommands::get();
        assert_eq!(
            dict.command_for("2524,2201", "kobold", EXIST, None)
                .as_deref(),
            Some("write kobold"),
            "`write @` takes the noun"
        );
    }

    #[test]
    fn hash_becomes_hash_plus_the_exist_id() {
        // The hash STAYS. `#12345` is the game's syntax for addressing an
        // object by id, so stripping it would send `attack 12345` -- which
        // targets nothing.
        let dict = MenuCommands::get();
        assert_eq!(
            dict.command_for("2524,1543", "kobold", EXIST, None)
                .as_deref(),
            Some("attack #121654846")
        );
    }

    #[test]
    fn percent_waits_for_a_secondary_argument() {
        // 20 of the 592 entries take a second argument -- a target item for
        // `transfer`, a demeanour for `demeanor`. Left in place when the
        // caller has not supplied one, so the gap is visible rather than
        // silently producing a malformed command.
        let dict = MenuCommands::get();
        let unfilled = dict.command_for("2524,2184", "orc", EXIST, None);
        assert_eq!(unfilled.as_deref(), Some("transfer #121654846 %"));
        let filled = dict.command_for("2524,2184", "orc", EXIST, Some("left arm"));
        assert_eq!(filled.as_deref(), Some("transfer #121654846 left arm"));
    }

    #[test]
    fn an_empty_noun_does_not_leave_ragged_whitespace() {
        // Most `<mi>` carry no `noun=` at all, so this is the common path.
        // `"sheathe @"` would become `"sheathe "`, and the two entries where
        // `@` sits mid-command would double a space:
        // `"convert set @ confirm"` -> `"convert set  confirm"`.
        let dict = MenuCommands::get();
        assert_eq!(
            dict.command_for("2524,1834", "", EXIST, None).as_deref(),
            Some("convert set confirm"),
            "no doubled space where the noun was"
        );
        assert_eq!(
            dict.command_for("2524,2201", "", EXIST, None).as_deref(),
            Some("write"),
            "no trailing space"
        );
    }

    #[test]
    fn an_unknown_coordinate_yields_no_command() {
        assert_eq!(
            MenuCommands::get().command_for("9999,9999", "kobold", EXIST, None),
            None
        );
    }
}

mod resolving_a_real_menu {
    use super::*;

    #[test]
    fn every_item_survives_resolution() {
        let items = MenuCommands::get().resolve(&menu(REAL_MENU), EXIST, None, None);
        assert_eq!(items.len(), 4, "the menu the game sent, entire");
    }

    #[test]
    fn a_known_coordinate_is_fully_resolved() {
        let items = MenuCommands::get().resolve(&menu(REAL_MENU), EXIST, None, None);
        let attack = &items[0];
        assert_eq!(attack.label.as_deref(), Some("attack"));
        assert_eq!(attack.command.as_deref(), Some("attack #121654846"));
        assert_eq!(attack.category.as_deref(), Some("6"));
        assert!(!attack.needs_secondary);
    }

    #[test]
    fn an_unknown_coordinate_is_carried_rather_than_dropped() {
        // Updating the dictionary fixed the 8 real unknowns, but cannot
        // fix the general case: the client's copy will always lag the
        // server's, and the wire carries no label to fall back on.
        //
        // Dropping the item would silently shorten the player's menu, which
        // is Rule 2.2's whole point. A visibly unnamed entry is the honest
        // failure.
        let items = MenuCommands::get().resolve(&menu(REAL_MENU), EXIST, None, None);
        let unknown = items.iter().find(|i| i.coord == "9999,9999").expect("kept");
        assert_eq!(unknown.label, None);
        assert_eq!(unknown.command, None);
        assert_eq!(unknown.category, None, "the wire placed it nowhere either");
    }

    #[test]
    fn an_item_says_when_it_still_needs_an_argument() {
        // A frontend has to know which entries open a prompt rather than
        // sending straight away. Without this flag it would either send
        // `transfer #123 %` verbatim or have to re-scan the command itself.
        let m = menu(concat!(
            r#"<menu id="1" cat_list="8">"#,
            r#"<mi coord="2524,2184"/>"#, // transfer # %
            r#"<mi coord="2524,1543"/>"#, // attack #
            r#"</menu>"#,
        ));
        let items = MenuCommands::get().resolve(&m, EXIST, None, None);
        assert!(items[0].needs_secondary, "transfer wants a second item");
        assert!(!items[1].needs_secondary, "attack does not");

        // ...and stops needing one once it is supplied.
        let filled = MenuCommands::get().resolve(&m, EXIST, Some("left arm"), None);
        assert!(!filled[0].needs_secondary);
    }

    #[test]
    fn a_menu_id_is_a_request_number_not_an_object() {
        // The hazard this test exists for: `Menu::id` LOOKS like an id. The
        // corpus shows `id="1"`, `"2"`, `"3"` -- a request sequence number.
        // Substituting it for `#` would send `attack #1`.
        let m = menu(REAL_MENU);
        assert_eq!(m.id, "1");
        let items = MenuCommands::get().resolve(&m, EXIST, None, None);
        assert_eq!(
            items[0].command.as_deref(),
            Some("attack #121654846"),
            "the caller's object, not the menu's sequence number"
        );
    }
}

mod grouping {
    use super::*;

    #[test]
    fn categories_follow_the_wires_own_order() {
        // `<menu cat_list="1 2 3 …">` is the game stating how to present
        // them, so it leads -- even though the items arrived attack-first,
        // and even though `BTreeMap` would otherwise sort "12" before "6".
        let groups = MenuCommands::get().resolve_grouped(&menu(REAL_MENU), EXIST, None, None);
        let order: Vec<Option<&str>> = groups.iter().map(|(c, _)| c.as_deref()).collect();
        assert_eq!(
            order,
            [Some("1"), Some("6"), None],
            "cat_list order first, uncategorised last"
        );
    }

    #[test]
    fn items_stay_grouped_with_their_category() {
        let groups = MenuCommands::get().resolve_grouped(&menu(REAL_MENU), EXIST, None, None);
        let combat = groups
            .iter()
            .find(|(c, _)| c.as_deref() == Some("6"))
            .expect("the combat category");
        let labels: Vec<&str> = combat.1.iter().filter_map(|i| i.label.as_deref()).collect();
        assert_eq!(labels, ["attack", "ambush"]);
    }

    #[test]
    fn the_wires_category_overrides_the_dictionarys() {
        // An `<mi menu_cat=>` is the game saying where the item belongs in
        // THIS menu, which is newer and more specific than the shipped
        // file's guess. `2524,1543` is category 6 in the dictionary.
        let m = menu(concat!(
            r#"<menu id="1" cat_list="1 2">"#,
            r#"<mi coord="2524,1543" menu_cat="2"/>"#,
            r#"</menu>"#,
        ));
        let items = MenuCommands::get().resolve(&m, EXIST, None, None);
        assert_eq!(items[0].category.as_deref(), Some("2"), "the wire wins");
        assert_eq!(
            MenuCommands::get()
                .entry("2524,1543")
                .map(|e| e.category.as_str()),
            Some("6"),
            "guard: the dictionary really did disagree"
        );
    }

    #[test]
    fn a_category_the_cat_list_omits_is_still_shown() {
        // `cat_list` cannot be treated as exhaustive: an item placed in a
        // category the envelope did not name still has to appear, or the
        // player loses a command that the game sent.
        let m = menu(concat!(
            r#"<menu id="1" cat_list="1">"#,
            r#"<mi coord="2524,1543" menu_cat="99"/>"#,
            r#"</menu>"#,
        ));
        let groups = MenuCommands::get().resolve_grouped(&m, EXIST, None, None);
        assert_eq!(groups.len(), 1);
        assert_eq!(groups[0].0.as_deref(), Some("99"));
    }
}

#[test]
fn the_noun_disambiguates_a_repeated_coordinate() {
    // One real menu carries nine items all on `2524,1906`, separated only by
    // `noun=` -- amplify, codex, shatter and six more. Resolving without it
    // collapses nine commands into one.
    let m = menu(concat!(
        r#"<menu id="1" cat_list="3">"#,
        r#"<mi coord="2524,1906" noun="amplify"/>"#,
        r#"<mi coord="2524,1906" noun="shatter"/>"#,
        r#"</menu>"#,
    ));
    let items = MenuCommands::get().resolve(&m, EXIST, None, None);
    let labels: Vec<&str> = items.iter().filter_map(|i| i.label.as_deref()).collect();
    assert_eq!(labels.len(), 2);
    assert_ne!(labels[0], labels[1], "two commands, not one repeated");
    assert!(labels[0].contains("amplify") && labels[1].contains("shatter"));
}
