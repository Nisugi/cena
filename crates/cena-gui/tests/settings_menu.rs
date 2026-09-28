//! `plan/50` §7 steps 1 and 2: the settings menu, the one every way in
//! opens. It draws the pages it is given, knowing no behavior, and asks the
//! binary for each change, written as the behavior's `;` command would type
//! it; Hydra's own pages, *Window* and *Keys*, it asks of the window.

use cena_gui::{KeyChange, KeysView, Menu, MenuAsked, MenuView, typed};
use cena_ui::settings::{Change, Page, Row, RowKind, Value};
use cena_ui::{HubRequest, RosterCard};
use egui::accesskit::Role;
use egui_kittest::Harness;
use egui_kittest::kittest::Queryable as _;

/// What the window would give the menu, and what it asked.
#[derive(Default)]
struct Board {
    menu: Menu,
    roster: Vec<RosterCard>,
    pages: Option<(String, Vec<Page>)>,
    said: Option<String>,
    own: Vec<Page>,
    bound: Vec<(String, String)>,
    numpad_always: bool,
    numpad: Option<String>,
    widgets: Vec<Page>,
    asked: Vec<MenuAsked>,
}

impl Board {
    fn draw(&mut self, ui: &mut egui::Ui) {
        let view = MenuView {
            roster: &self.roster,
            pages: self
                .pages
                .as_ref()
                .map(|(whose, pages)| (whose.as_str(), pages.as_slice())),
            said: self.said.as_deref(),
            own: &self.own,
            keys: KeysView {
                bound: &self.bound,
                numpad_always: self.numpad_always,
                said: &[],
                numpad: self.numpad.as_deref(),
            },
            widgets: &self.widgets,
        };
        if let Some(request) = self.menu.show(ui, &view) {
            self.asked.push(request);
        }
    }

    /// What it asked of the binary.
    fn of_binary(&self) -> Vec<HubRequest> {
        self.asked
            .iter()
            .filter_map(|asked| match asked {
                MenuAsked::Binary(request) => Some(request.clone()),
                _ => None,
            })
            .collect()
    }
}

fn card(character: &str) -> RosterCard {
    RosterCard {
        character: character.to_owned(),
        account: format!("{}01", character.to_lowercase()),
        game: "GS3".to_owned(),
        kept: true,
        favourite: false,
    }
}

fn row(key: &str, label: &str, kind: RowKind, value: Value, here: bool) -> Row {
    Row {
        key: key.to_owned(),
        label: label.to_owned(),
        help: format!("What {label} does."),
        kind,
        value,
        here,
        from: None,
    }
}

fn page(id: &str, title: &str, rows: Vec<Row>) -> Page {
    Page {
        id: id.to_owned(),
        title: title.to_owned(),
        file: format!("hunt/{id}/prime_nisugi.toml"),
        takes: format!("the next time {title} runs"),
        problem: None,
        rows,
    }
}

/// Nisugi's Heal and Spellcaster pages.
fn pages() -> Vec<Page> {
    vec![
        page(
            "heal",
            "Heal",
            vec![
                row(
                    "container",
                    "Herb container",
                    RowKind::Text,
                    Value::Text("herbsack".to_owned()),
                    true,
                ),
                row(
                    "potions",
                    "Prefer potions",
                    RowKind::Toggle,
                    Value::On(false),
                    false,
                ),
                row(
                    "stock",
                    "Stock to (percent)",
                    RowKind::Whole { min: 0, max: 1000 },
                    Value::Unset,
                    false,
                ),
            ],
        ),
        page(
            "sc",
            "Spellcaster",
            vec![row(
                "alias",
                "Aliases",
                RowKind::Map,
                Value::Map(vec![("boom".to_owned(), "910".to_owned())]),
                true,
            )],
        ),
    ]
}

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

/// The menu as the hub opens it, on Hydra's own pages, with `bound` bound.
fn hydra<'a>(bound: &[(&str, &str)]) -> Harness<'a, Board> {
    let window = page(
        "window",
        "Window",
        vec![
            row(
                "card_width",
                "Card width",
                RowKind::Number {
                    min: 160.0,
                    max: 2000.0,
                },
                Value::Text("312".to_owned()),
                false,
            ),
            row(
                "close_with_session",
                "Close a play window when its session closes",
                RowKind::Toggle,
                Value::On(false),
                false,
            ),
        ],
    );
    let mut board = Board {
        roster: vec![card("Nisugi")],
        own: vec![window],
        bound: bound
            .iter()
            .map(|(key, line)| ((*key).to_owned(), (*line).to_owned()))
            .collect(),
        ..Board::default()
    };
    board.menu.open_for(None);
    let mut harness = Harness::builder()
        .with_size((760.0, 520.0))
        .build_ui_state(|ui, board: &mut Board| board.draw(ui), board);
    harness.run();
    harness
}

/// Replace what the field labelled `label` holds with `text`, and press
/// Enter.
fn replace(harness: &mut Harness<'_, Board>, label: &str, text: &str) {
    harness
        .get_by_role_and_label(Role::TextInput, label)
        .focus();
    harness.run();
    harness.key_press_modifiers(egui::Modifiers::COMMAND, egui::Key::A);
    harness.run();
    enter(harness, label, text);
}

fn bind(key: &str, line: &str, was: Option<&str>) -> MenuAsked {
    MenuAsked::Key(KeyChange::Bind {
        key: key.to_owned(),
        line: line.to_owned(),
        was: was.map(str::to_owned),
    })
}

/// A change on Hydra's *Window* page is asked of the window, not the
/// binary, and what the window answers is said at the top.
#[test]
fn hydras_own_page_is_changed_by_the_window() {
    let mut harness = hydra(&[]);
    harness
        .get_by_role_and_label(
            Role::CheckBox,
            "Close a play window when its session closes",
        )
        .click();
    harness.run();
    replace(&mut harness, "Card width", "400");
    assert_eq!(
        harness.state().asked,
        [
            MenuAsked::Own {
                key: "close_with_session".to_owned(),
                to: Some("on".to_owned()),
            },
            MenuAsked::Own {
                key: "card_width".to_owned(),
                to: Some("400".to_owned()),
            },
        ]
    );
    harness
        .state_mut()
        .menu
        .tell("Window: the card width is 400.".to_owned());
    harness.run();
    assert!(
        harness
            .query_by_label("Window: the card width is 400.")
            .is_some()
    );
}

/// A key is bound by pressing it: *Add a key*, the key, then the line it
/// sends. One that types, or one bound already, is refused and said;
/// Escape stops the wait.
#[test]
fn a_key_is_bound_by_pressing_it() {
    let mut harness = hydra(&[("Ctrl+F1", "look")]);
    harness.get_by_label("Keys").click();
    harness.run();
    harness.get_by_label("Add a key").click();
    harness.run();
    assert!(harness.state().menu.waiting_for_key());
    harness.key_press(egui::Key::A);
    harness.run();
    assert!(harness.query_by_label_contains("KeyA types").is_some());
    assert!(harness.state().menu.waiting_for_key(), "still waiting");
    harness.key_press_modifiers(egui::Modifiers::CTRL, egui::Key::F1);
    harness.run();
    assert!(
        harness
            .query_by_label("Ctrl+F1 already sends `look`.")
            .is_some()
    );

    harness.get_by_label("Add a key").click();
    harness.run();
    harness.key_press(egui::Key::F5);
    harness.run();
    assert!(!harness.state().menu.waiting_for_key());
    enter(&mut harness, "F5", "hide");
    assert_eq!(harness.state().asked, [bind("F5", "hide", None)]);

    harness.get_by_label("Add a key").click();
    harness.run();
    harness.key_press(egui::Key::Escape);
    harness.run();
    assert!(!harness.state().menu.waiting_for_key(), "stopped");
    assert_eq!(harness.state().asked.len(), 1);
}

/// A bound key's line is changed where it is typed; the key itself moves
/// by pressing another, a numpad key coming through the window; *Remove*
/// unbinds it; the numpad's switch is asked for as it is ticked.
#[test]
fn a_bound_key_is_changed_moved_and_removed() {
    let mut harness = hydra(&[("Ctrl+F1", "look"), ("Numpad8", "north")]);
    harness.get_by_label("Keys").click();
    harness.run();
    enter(&mut harness, "Ctrl+F1", " around");
    harness
        .get_by_role_and_label(Role::Button, "Ctrl+F1")
        .click();
    harness.run();
    assert!(harness.state().menu.waiting_for_key());
    harness.state_mut().numpad = Some("Numpad2".to_owned());
    harness.run();
    harness.state_mut().numpad = None;
    if let Some(remove) = harness.get_all_by_label("Remove").nth(1) {
        remove.click();
    }
    harness.run();
    harness
        .get_by_role_and_label(
            Role::CheckBox,
            "The numpad sends its keys with NumLock on too",
        )
        .click();
    harness.run();
    assert_eq!(
        harness.state().asked,
        [
            bind("Ctrl+F1", "look around", None),
            bind("Numpad2", "look", Some("Ctrl+F1")),
            MenuAsked::Key(KeyChange::Unbind("Numpad8".to_owned())),
            MenuAsked::Key(KeyChange::NumpadAlways(true)),
        ]
    );
}

/// The *Keys* page as a player sees it, rendered and compared with the
/// committed image.
#[test]
fn the_keys_page_as_drawn() {
    let mut board = Board {
        roster: vec![card("Nisugi")],
        bound: vec![
            ("Ctrl+F1".to_owned(), "look".to_owned()),
            ("Numpad8".to_owned(), "north".to_owned()),
        ],
        ..Board::default()
    };
    board.menu.open_for(None);
    let mut harness = Harness::builder()
        .with_size((760.0, 260.0))
        .wgpu()
        .build_ui_state(|ui, board: &mut Board| board.draw(ui), board);
    // With no other page of Hydra's given, Keys is the one showing.
    harness.run();
    harness.snapshot("settings_keys");
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

/// A widget's own page, from its play window, sits after the character's
/// others; a choice and a colour are drawn as such, and a change on it is
/// asked of the window, not the binary.
#[test]
fn a_widget_page_is_asked_of_its_window() {
    let health = page(
        "widget:3",
        "Health (Vitals)",
        vec![
            row(
                "fills",
                "Fills",
                RowKind::Choice(vec![
                    ("right".to_owned(), "Across, from the left".to_owned()),
                    ("up".to_owned(), "Upright, from the bottom".to_owned()),
                ]),
                Value::Text("right".to_owned()),
                false,
            ),
            row(
                "color",
                "Colour",
                RowKind::Color,
                Value::Text("#cd4d4d".to_owned()),
                false,
            ),
        ],
    );
    let mut board = Board {
        roster: vec![card("Nisugi")],
        pages: Some(("GS3:Nisugi".to_owned(), pages())),
        widgets: vec![health],
        ..Board::default()
    };
    board
        .menu
        .open_at(Some("GS3:Nisugi".to_owned()), Some("widget:3"));
    let mut harness = Harness::builder()
        .with_size((760.0, 520.0))
        .build_ui_state(|ui, board: &mut Board| board.draw(ui), board);
    harness.run();
    assert!(harness.query_by_label("Heal").is_some(), "the others too");
    assert_eq!(
        harness.query_all_by_label("Colour").count(),
        2,
        "the row's name, and its colour button"
    );
    assert!(
        harness.query_by_label("Upright, from the bottom").is_none(),
        "the choices drop down, not all shown at once"
    );
    harness
        .get_by_role_and_label(Role::ComboBox, "Fills")
        .click();
    harness.run();
    harness.get_by_label("Upright, from the bottom").click();
    harness.run();
    assert_eq!(
        harness.state().asked,
        [
            MenuAsked::Binary(HubRequest::Settings("GS3:Nisugi".to_owned())),
            MenuAsked::Widget {
                page: "widget:3".to_owned(),
                key: "fills".to_owned(),
                to: Some("up".to_owned()),
            }
        ],
        "the character's pages asked for on opening, then the change of the window"
    );
}

/// A bar's page as a player sees it, each choice one line that drops down
/// (the author, 2026-09-28: *"vitals settings menu is busted looking"*),
/// rendered and compared with the committed image.
#[test]
fn a_bars_page_as_drawn() {
    let choice = |named: &[(&str, &str)]| {
        RowKind::Choice(
            named
                .iter()
                .map(|(value, called)| ((*value).to_owned(), (*called).to_owned()))
                .collect(),
        )
    };
    let on = |key: &str, label: &str| row(key, label, RowKind::Toggle, Value::On(true), false);
    let mana = page(
        "widget:4",
        "Mana (Vitals)",
        vec![
            row(
                "fills",
                "Fills",
                choice(&[
                    ("right", "Across, from the left"),
                    ("left", "Across, from the right"),
                    ("up", "Upright, from the bottom"),
                    ("down", "Upright, from the top"),
                ]),
                Value::Text("up".to_owned()),
                true,
            ),
            row(
                "text",
                "Text",
                choice(&[
                    ("inside", "Inside"),
                    ("above", "Above"),
                    ("below", "Below"),
                    ("left", "Left"),
                    ("right", "Right"),
                    ("hidden", "None"),
                ]),
                Value::Text("inside".to_owned()),
                false,
            ),
            on("label", "Says its label"),
            on("numbers", "Says current/max"),
            on("percent", "Says its percent"),
            row(
                "color",
                "Colour",
                RowKind::Color,
                Value::Text("#4784d9".to_owned()),
                false,
            ),
            row(
                "overlay",
                "Overlay",
                choice(&[("", "None")]),
                Value::Text(String::new()),
                false,
            ),
        ],
    );
    let mut board = Board {
        roster: vec![card("Nisugi")],
        pages: Some(("GS3:Nisugi".to_owned(), pages())),
        widgets: vec![mana],
        ..Board::default()
    };
    board
        .menu
        .open_at(Some("GS3:Nisugi".to_owned()), Some("widget:4"));
    let mut harness = Harness::builder()
        .with_size((760.0, 360.0))
        .wgpu()
        .build_ui_state(|ui, board: &mut Board| board.draw(ui), board);
    harness.run();
    harness.snapshot("settings_bar");
}
