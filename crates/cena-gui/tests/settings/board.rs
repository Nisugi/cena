//! The settings menu's board, card and pages (`mod.rs` says why they are
//! shared).

use cena_gui::{KeysView, Menu, MenuAsked, MenuView};
use cena_ui::settings::{Page, Row, RowKind, Value};
use cena_ui::{HubRequest, RosterCard};

/// What the window would give the menu, and what it asked.
#[derive(Default)]
pub struct Board {
    pub menu: Menu,
    pub roster: Vec<RosterCard>,
    pub pages: Option<(String, Vec<Page>)>,
    pub said: Option<String>,
    pub own: Vec<Page>,
    pub bound: Vec<(String, String)>,
    pub numpad_always: bool,
    pub numpad: Option<String>,
    pub widgets: Vec<Page>,
    pub asked: Vec<MenuAsked>,
}

impl Board {
    pub fn draw(&mut self, ui: &mut egui::Ui) {
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
    pub fn of_binary(&self) -> Vec<HubRequest> {
        self.asked
            .iter()
            .filter_map(|asked| match asked {
                MenuAsked::Binary(request) => Some(request.clone()),
                _ => None,
            })
            .collect()
    }
}

pub fn card(character: &str) -> RosterCard {
    RosterCard {
        character: character.to_owned(),
        account: format!("{}01", character.to_lowercase()),
        game: "GS3".to_owned(),
        kept: true,
        favourite: false,
    }
}

pub fn row(key: &str, label: &str, kind: RowKind, value: Value, here: bool) -> Row {
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

pub fn page(id: &str, title: &str, rows: Vec<Row>) -> Page {
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
pub fn pages() -> Vec<Page> {
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
