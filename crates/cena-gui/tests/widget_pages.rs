//! A widget's own page in the settings menu (`plan/50` §7 step 8, as the
//! author corrected it): it sits after the character's other pages, its
//! choices and colour drawn as such, and a change on it is asked of the play
//! window, not the binary.

use cena_gui::MenuAsked;
use cena_ui::HubRequest;
use cena_ui::settings::{RowKind, Value};
use egui::accesskit::Role;
use egui_kittest::Harness;
use egui_kittest::kittest::Queryable as _;

mod settings;

use settings::{Board, card, page, pages, row};

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
