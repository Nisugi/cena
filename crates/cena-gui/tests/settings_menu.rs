//! `plan/50` §7 steps 1 and 2: the settings menu, the one every way in
//! opens. It draws the pages it is given, knowing no behavior, and asks the
//! binary for each change, written as the behavior's `;` command would type
//! it; Hydra's own pages, *Window* and *Keys*, it asks of the window.

use cena_gui::{MenuAsked, typed};
use cena_ui::HubRequest;
use cena_ui::settings::{Change, Row, RowKind, Value};
use egui::accesskit::Role;
use egui_kittest::Harness;
use egui_kittest::kittest::Queryable as _;

mod settings;

use settings::{Board, card, page, pages, row};

/// The menu open on Nisugi, with Nisugi's pages given.
fn menu<'a>() -> Harness<'a, Board> {
    let mut board = Board {
        roster: vec![card("Nisugi"), card("Dicate")],
        pages: Some(("GS3:Nisugi".to_owned(), pages())),
        ..Board::default()
    };
    board.menu.open_for(Some("GS3:Nisugi".to_owned()));
    let mut harness = Harness::builder()
        .with_size((760.0, 520.0))
        .build_ui_state(|ui, board: &mut Board| board.draw(ui), board);
    harness.run();
    harness.state_mut().asked.clear();
    harness
}

fn change(key: &str, to: Option<&str>) -> MenuAsked {
    MenuAsked::Binary(HubRequest::Change(Change {
        character: "GS3:Nisugi".to_owned(),
        page: "heal".to_owned(),
        key: key.to_owned(),
        to: to.map(str::to_owned),
    }))
}

/// Type `text` into the field labelled `label`, and press Enter.
fn enter(harness: &mut Harness<'_, Board>, label: &str, text: &str) {
    harness
        .get_by_role_and_label(Role::TextInput, label)
        .focus();
    harness.run();
    harness
        .get_by_role_and_label(Role::TextInput, label)
        .type_text(text);
    harness.run();
    harness.key_press(egui::Key::Enter);
    harness.run();
}

/// Opened as the hub opens it, it shows Hydra's own pages and asks the
/// binary nothing; a character picked, it asks for that one's pages once,
/// and says it is reading them until they come.
#[test]
fn it_asks_for_a_characters_pages_once() {
    let mut board = Board {
        roster: vec![card("Nisugi"), card("Dicate")],
        own: vec![page("window", "Window", Vec::new())],
        ..Board::default()
    };
    board.menu.open_for(None);
    let mut harness = Harness::builder()
        .with_size((760.0, 520.0))
        .build_ui_state(|ui, board: &mut Board| board.draw(ui), board);
    harness.run();
    assert!(harness.state().asked.is_empty());
    assert!(harness.query_by_label("Keys").is_some(), "Hydra's own");

    let pick = |harness: &mut Harness<'_, Board>, name: &str| {
        harness
            .get_by_role_and_label(Role::ComboBox, "Settings for")
            .click();
        harness.run();
        harness.get_by_label(name).click();
        harness.run();
    };
    pick(&mut harness, "Nisugi (Prime)");
    assert_eq!(
        harness.state().of_binary(),
        [HubRequest::Settings("GS3:Nisugi".to_owned())]
    );
    assert!(harness.query_by_label("Reading the settings...").is_some());
    harness.state_mut().pages = Some(("GS3:Nisugi".to_owned(), pages()));
    harness.run();
    assert!(harness.query_by_label("Herb container").is_some());
    assert_eq!(harness.state().asked.len(), 1, "asked once");

    pick(&mut harness, "Dicate (Prime)");
    assert_eq!(
        harness.state().of_binary().last(),
        Some(&HubRequest::Settings("GS3:Dicate".to_owned()))
    );
    assert!(
        harness.query_by_label("Herb container").is_none(),
        "Nisugi's pages are not Dicate's"
    );

    // Opened again, it asks again: a `;` command may have changed a file.
    let asked = harness.state().asked.len();
    harness
        .state_mut()
        .menu
        .open_for(Some("GS3:Dicate".to_owned()));
    harness.run();
    assert_eq!(harness.state().asked.len(), asked + 1);

    pick(&mut harness, "Hydra (every character)");
    assert_eq!(harness.state().menu.character(), None);
    assert_eq!(harness.state().asked.len(), asked + 1, "asked nothing");
}

/// A toggle asks at once; a typed value is asked for on Enter, written as
/// the command would type it; one out of its range is refused here, and
/// nothing is sent.
#[test]
fn each_change_is_asked_for_as_the_command_would_type_it() {
    let mut harness = menu();
    harness
        .get_by_role_and_label(Role::CheckBox, "Prefer potions")
        .click();
    harness.run();
    enter(&mut harness, "Stock to (percent)", "150");
    enter(&mut harness, "Herb container", " pouch");
    assert_eq!(
        harness.state().asked,
        [
            change("potions", Some("on")),
            change("stock", Some("150")),
            change("container", Some("\"herbsack pouch\"")),
        ]
    );
    enter(&mut harness, "Stock to (percent)", "5000");
    assert_eq!(harness.state().asked.len(), 3, "not sent");
    assert!(
        harness
            .query_by_label_contains("a whole number from 0 to 1000")
            .is_some()
    );
}

/// *Use default* is offered where the file sets a value, and puts it back;
/// a setting at its default says so.
#[test]
fn a_setting_the_file_sets_is_put_back_to_its_default() {
    let mut harness = menu();
    // What the menu said of an earlier change gives way to the binary's.
    harness
        .state_mut()
        .menu
        .tell("Window: old news.".to_owned());
    harness.run();
    assert!(harness.query_by_label("Window: old news.").is_some());
    assert_eq!(harness.query_all_by_label("Use default").count(), 1);
    assert_eq!(harness.query_all_by_label("default").count(), 2);
    harness.get_by_label("Use default").click();
    harness.run();
    assert_eq!(harness.state().asked, [change("container", None)]);
    assert!(harness.query_by_label("Window: old news.").is_none());
}

/// A page says where it is kept and when a change takes effect; a map is
/// shown and not edited; a file that does not read shows why and no rows.
#[test]
fn a_page_says_where_it_is_kept_and_what_it_cannot_change() {
    let mut harness = menu();
    assert!(
        harness
            .query_by_label(
                "Saved in hunt/heal/prime_nisugi.toml; a change takes effect the next time Heal runs."
            )
            .is_some()
    );
    harness.get_by_label("Spellcaster").click();
    harness.run();
    assert!(harness.query_by_label("boom = 910").is_some());
    assert!(
        harness
            .query_by_role_and_label(Role::TextInput, "Aliases")
            .is_none()
    );

    let mut broken = pages();
    broken[0].problem = Some("Nothing here is changed while it does not read.".to_owned());
    broken[0].rows.clear();
    harness.state_mut().pages = Some(("GS3:Nisugi".to_owned(), broken));
    harness.get_by_label("Heal").click();
    harness.run();
    assert!(
        harness
            .query_by_label("Nothing here is changed while it does not read.")
            .is_some()
    );
    assert!(harness.query_by_label("Herb container").is_none());
}

/// What is typed for each kind is written as the command would type it: a
/// list in brackets, words quoted with a quote in them escaped, a number as
/// it is; what is not of its kind is refused.
#[test]
fn what_is_typed_is_written_as_the_command_types_it() {
    assert_eq!(
        typed(&RowKind::Numbers, "401, 406 409").as_deref(),
        Ok("[401, 406, 409]")
    );
    assert!(typed(&RowKind::Numbers, "401, boom").is_err());
    assert_eq!(
        typed(&RowKind::Words, "a pouch, the \"good\" sack").as_deref(),
        Ok("[\"a pouch\", \"the \\\"good\\\" sack\"]")
    );
    assert_eq!(
        typed(
            &RowKind::Number {
                min: 0.0,
                max: 250.0
            },
            "180.5"
        )
        .as_deref(),
        Ok("180.5")
    );
    assert!(
        typed(
            &RowKind::Number {
                min: 0.0,
                max: 250.0
            },
            "251"
        )
        .is_err()
    );
    assert_eq!(
        typed(&RowKind::Text, "on").as_deref(),
        Ok("\"on\""),
        "words, not a switch"
    );
}

/// The menu as a player sees it, rendered and compared with the committed
/// image.
#[test]
fn the_menu_as_drawn() {
    let board = Board {
        roster: vec![card("Nisugi"), card("Dicate")],
        pages: Some(("GS3:Nisugi".to_owned(), pages())),
        ..Board::default()
    };
    let mut board = board;
    board.menu.open_for(Some("GS3:Nisugi".to_owned()));
    let mut harness = Harness::builder()
        .with_size((760.0, 360.0))
        .wgpu()
        .build_ui_state(|ui, board: &mut Board| board.draw(ui), board);
    harness.run();
    harness.snapshot("settings_menu");
}

/// A hunt page: each setting says where its value came from in place of
/// *default*, *Use default* only where the profile sets it, and a heading
/// stands over each table's settings.
#[test]
fn a_hunt_page_says_where_each_setting_came_from() {
    let from = |mut row: Row, level: &str| {
        row.from = Some(level.to_owned());
        row
    };
    let hunt = page(
        "hunt:p",
        "Hunt: p",
        vec![
            from(
                row(
                    "priority",
                    "priority",
                    RowKind::Toggle,
                    Value::On(false),
                    false,
                ),
                "built in",
            ),
            from(
                row(
                    "rooms.hunting",
                    "rooms.hunting",
                    RowKind::Whole {
                        min: 0,
                        max: u32::MAX,
                    },
                    Value::Text("10".to_owned()),
                    true,
                ),
                "the profile",
            ),
            from(
                row(
                    "rest.fried",
                    "rest.fried",
                    RowKind::Whole {
                        min: 0,
                        max: u32::MAX,
                    },
                    Value::Text("90".to_owned()),
                    false,
                ),
                "global",
            ),
        ],
    );
    let mut board = Board {
        roster: vec![card("Nisugi")],
        pages: Some(("GS3:Nisugi".to_owned(), vec![hunt])),
        ..Board::default()
    };
    board.menu.open_for(Some("GS3:Nisugi".to_owned()));
    let mut harness = Harness::builder()
        .with_size((760.0, 520.0))
        .build_ui_state(|ui, board: &mut Board| board.draw(ui), board);
    harness.run();
    for said in ["built in", "global", "rooms", "rest"] {
        assert!(harness.query_by_label(said).is_some(), "{said}");
    }
    assert!(
        harness.query_by_label("default").is_none(),
        "where, not default"
    );
    assert_eq!(harness.query_all_by_label("Use default").count(), 1);
}

/// A page whose settings are all in one table, as Skinning's are under
/// `[skin]`, has no heading over them: the page's title says it.
#[test]
fn one_table_has_no_heading() {
    let skinning = page(
        "skin",
        "Skinning",
        vec![
            row(
                "skin.enable",
                "Skin",
                RowKind::Toggle,
                Value::On(true),
                true,
            ),
            row(
                "skin.kneel",
                "Kneel to skin",
                RowKind::Toggle,
                Value::On(false),
                false,
            ),
        ],
    );
    let mut board = Board {
        roster: vec![card("Nisugi")],
        pages: Some(("GS3:Nisugi".to_owned(), vec![skinning])),
        ..Board::default()
    };
    board.menu.open_for(Some("GS3:Nisugi".to_owned()));
    let mut harness = Harness::builder()
        .with_size((760.0, 520.0))
        .build_ui_state(|ui, board: &mut Board| board.draw(ui), board);
    harness.run();
    assert!(harness.query_all_by_label("Kneel to skin").next().is_some());
    assert!(harness.query_by_label("skin").is_none(), "no heading");
}

/// Every other way in opens the one menu at its place (`plan/50` §7 step
/// 8): a page by its id, or the first whose id begins so, a hunt page.
#[test]
fn a_way_in_opens_the_menu_at_its_page() {
    let mut board = Board {
        roster: vec![card("Nisugi")],
        pages: Some(("GS3:Nisugi".to_owned(), pages())),
        ..Board::default()
    };
    board
        .menu
        .open_at(Some("GS3:Nisugi".to_owned()), Some("sc"));
    let mut harness = Harness::builder()
        .with_size((760.0, 520.0))
        .build_ui_state(|ui, board: &mut Board| board.draw(ui), board);
    harness.run();
    assert!(
        harness.query_by_label("boom = 910").is_some(),
        "Spellcaster's"
    );
    assert_eq!(harness.state().menu.page(), Some("sc"));

    let mut with_hunt = pages();
    with_hunt.push(page("hunt:ojandhaart", "Hunt: ojandhaart", Vec::new()));
    harness.state_mut().pages = Some(("GS3:Nisugi".to_owned(), with_hunt));
    harness
        .state_mut()
        .menu
        .open_at(Some("GS3:Nisugi".to_owned()), Some("hunt:"));
    harness.run();
    assert_eq!(harness.state().menu.page(), Some("hunt:ojandhaart"));
}

/// A read-only row shows the value in effect, in the form a hunt page gives
/// it -- a character's own setting as words -- and only nothing as "none"
/// (the crate review of 2026-09-28, R12: a resting room of 29877 showed
/// "none").
#[test]
fn a_read_only_row_shows_the_value_in_effect() {
    let read_only =
        |key: &str, label: &str, value: Value| row(key, label, RowKind::Map, value, false);
    let hunt = page(
        "hunt:p",
        "Hunt: p",
        vec![
            read_only(
                "rooms.resting",
                "Resting room",
                Value::Text("29877".to_owned()),
            ),
            read_only(
                "targets",
                "Targets",
                Value::List(vec!["kobold".to_owned(), "rat".to_owned()]),
            ),
            read_only("spells", "Spells", Value::Map(Vec::new())),
            read_only(
                "stances",
                "Stances",
                Value::Map(vec![("attack".to_owned(), "offensive".to_owned())]),
            ),
        ],
    );
    let mut board = Board {
        roster: vec![card("Nisugi")],
        pages: Some(("GS3:Nisugi".to_owned(), vec![hunt])),
        ..Board::default()
    };
    board.menu.open_for(Some("GS3:Nisugi".to_owned()));
    let mut harness = Harness::builder()
        .with_size((760.0, 520.0))
        .build_ui_state(|ui, board: &mut Board| board.draw(ui), board);
    harness.run();
    for shown in ["29877", "kobold, rat", "none", "attack = offensive"] {
        assert!(harness.query_by_label(shown).is_some(), "{shown}");
    }
}
