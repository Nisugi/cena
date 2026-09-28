//! `plan/50` §7 step 1: the settings menu, the one every way in opens. It
//! draws the pages it is given, knowing no behavior, and asks the binary
//! for each change, written as the behavior's `;` command would type it.

use cena_gui::{Menu, MenuView, typed};
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
    asked: Vec<HubRequest>,
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
        };
        if let Some(request) = self.menu.show(ui, &view) {
            self.asked.push(request);
        }
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

/// The menu open, with Nisugi's pages given.
fn menu<'a>() -> Harness<'a, Board> {
    let board = Board {
        roster: vec![card("Nisugi"), card("Dicate")],
        pages: Some(("GS3:Nisugi".to_owned(), pages())),
        ..Board::default()
    };
    let mut harness = Harness::builder()
        .with_size((760.0, 520.0))
        .build_ui_state(|ui, board: &mut Board| board.draw(ui), board);
    harness.run();
    harness.state_mut().asked.clear();
    harness
}

fn change(key: &str, to: Option<&str>) -> HubRequest {
    HubRequest::Change(Change {
        character: "GS3:Nisugi".to_owned(),
        page: "heal".to_owned(),
        key: key.to_owned(),
        to: to.map(str::to_owned),
    })
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

/// Opened, it asks for the first roster character's pages once, and says
/// it is reading them until they come; picking another character asks for
/// that one's.
#[test]
fn it_asks_for_a_characters_pages_once() {
    let mut harness = Harness::builder().with_size((760.0, 520.0)).build_ui_state(
        |ui, board: &mut Board| board.draw(ui),
        Board {
            roster: vec![card("Nisugi"), card("Dicate")],
            ..Board::default()
        },
    );
    harness.run();
    assert_eq!(
        harness.state().asked,
        [HubRequest::Settings("GS3:Nisugi".to_owned())]
    );
    assert!(harness.query_by_label("Reading the settings...").is_some());
    harness.state_mut().pages = Some(("GS3:Nisugi".to_owned(), pages()));
    harness.run();
    assert!(harness.query_by_label("Herb container").is_some());
    assert_eq!(harness.state().asked.len(), 1, "asked once");

    harness
        .get_by_role_and_label(Role::ComboBox, "Character")
        .click();
    harness.run();
    harness.get_by_label("Dicate (Prime)").click();
    harness.run();
    assert_eq!(
        harness.state().asked.last(),
        Some(&HubRequest::Settings("GS3:Dicate".to_owned()))
    );
    assert!(
        harness.query_by_label("Herb container").is_none(),
        "Nisugi's pages are not Dicate's"
    );

    // Opened again, it asks again: a `;` command may have changed a file.
    let asked = harness.state().asked.len();
    harness.state_mut().menu.open_for(None);
    harness.run();
    assert_eq!(harness.state().asked.len(), asked + 1);
    assert_eq!(
        harness.state().asked.last(),
        Some(&HubRequest::Settings("GS3:Dicate".to_owned())),
        "on the character it was on"
    );
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
    assert_eq!(harness.query_all_by_label("Use default").count(), 1);
    assert_eq!(harness.query_all_by_label("default").count(), 2);
    harness.get_by_label("Use default").click();
    harness.run();
    assert_eq!(harness.state().asked, [change("container", None)]);
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
    let mut harness = Harness::builder()
        .with_size((760.0, 360.0))
        .wgpu()
        .build_ui_state(|ui, board: &mut Board| board.draw(ui), board);
    harness.run();
    harness.snapshot("settings_menu");
}
