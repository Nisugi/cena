//! The settings menu (`plan/50` §7 step 1), the one every way in opens. The
//! author, 2026-09-27: *"one main settings button to get to the main
//! settings menu. if we add right click context menu options to open
//! settings for widgets/windows then they would open the same main settings
//! menu to the correct spot for that setting."*
//!
//! A character is picked from the roster, running or not. Its pages come
//! from the binary as [`Page`]s, which this draws knowing no behavior. Each
//! change goes back as a [`HubRequest::Change`], which the binary applies
//! through the writer the behavior's `;` command uses. So the menu and the
//! command cannot disagree about a value.
//!
//! A text setting is typed and committed on Enter, or when the field is
//! left; a value that is not of its kind is refused here, before anything is
//! sent. *Use default* is offered where the file sets a value.

use std::collections::HashMap;

use cena_ui::settings::{Change, Page, Row, RowKind, Value};
use cena_ui::{HubRequest, RosterCard};

/// What a row asks for.
enum Wanted {
    /// This value, as the command would type it.
    Set(String),
    /// The setting's default: taken out of the file.
    Default,
}

/// The settings menu's state, which outlives a frame.
#[derive(Debug, Default)]
pub struct Menu {
    /// Whether its window is open.
    pub open: bool,
    /// The character whose settings are shown, as the roster names it.
    character: Option<String>,
    /// The page showing, by its id.
    page: Option<String>,
    /// What is typed into a field, by page and key, while it has the focus.
    typed: HashMap<(String, String), String>,
    /// The character whose pages were last asked for, since the menu opened.
    asked: Option<String>,
    /// Why the last typed value was not sent.
    refused: Option<String>,
}

/// What the menu draws from this frame.
#[derive(Clone, Copy, Debug, Default)]
pub struct MenuView<'a> {
    /// Every character on the roster.
    pub roster: &'a [RosterCard],
    /// The pages the binary last gave, and for whom.
    pub pages: Option<(&'a str, &'a [Page])>,
    /// What the binary answered the last request.
    pub said: Option<&'a str>,
}

/// A roster character's name for the binary: `GAME:Name`.
#[must_use]
pub fn roster_name(card: &RosterCard) -> String {
    format!("{}:{}", card.game, card.character)
}

impl Menu {
    /// Open the menu, on `character`'s settings when one is named: a play
    /// window's button names its own. Its pages are asked for afresh.
    pub fn open_for(&mut self, character: Option<String>) {
        self.open = true;
        self.asked = None;
        if character.is_some() {
            self.character = character;
        }
    }

    /// The character whose settings are shown, as the roster names it.
    #[must_use]
    pub fn character(&self) -> Option<&str> {
        self.character.as_deref()
    }

    /// Draw the menu over `view`, and return what it asks of the binary.
    pub fn show(&mut self, ui: &mut egui::Ui, view: &MenuView<'_>) -> Option<HubRequest> {
        let mut asked = None;
        if self.character.is_none() {
            self.character = view.roster.first().map(roster_name);
        }
        let Some(character) = self.character.clone() else {
            ui.weak("No character is on the roster yet: log one in from Not launched.");
            return None;
        };
        ui.horizontal(|ui| {
            let label = ui.label("Character");
            let called = |name: &str| {
                view.roster
                    .iter()
                    .find(|card| roster_name(card) == name)
                    .map_or_else(
                        || name.to_owned(),
                        |card| {
                            format!(
                                "{} ({})",
                                card.character,
                                crate::launch::game_name(&card.game)
                            )
                        },
                    )
            };
            egui::ComboBox::from_id_salt("settings-character")
                .selected_text(called(&character))
                .show_ui(ui, |ui| {
                    for card in view.roster {
                        let name = roster_name(card);
                        let shown = called(&name);
                        ui.selectable_value(&mut self.character, Some(name), shown);
                    }
                })
                .response
                .labelled_by(label.id);
        });
        let character = self.character.clone().unwrap_or(character);
        if self.asked.as_deref() != Some(character.as_str()) {
            self.asked = Some(character.clone());
            self.typed.clear();
            asked = Some(HubRequest::Settings(character.clone()));
        }
        if let Some(said) = self.refused.as_deref().or(view.said) {
            ui.weak(said);
        }
        ui.separator();
        let pages = match view.pages {
            Some((whose, pages)) if whose == character => pages,
            _ => {
                ui.weak("Reading the settings...");
                return asked;
            }
        };
        if self
            .page
            .as_deref()
            .is_none_or(|id| pages.iter().all(|page| page.id != id))
        {
            self.page = pages.first().map(|page| page.id.clone());
        }
        ui.horizontal_top(|ui| {
            ui.vertical(|ui| {
                ui.set_width(140.0);
                for page in pages {
                    let showing = self.page.as_deref() == Some(page.id.as_str());
                    if ui.selectable_label(showing, &page.title).clicked() {
                        self.page = Some(page.id.clone());
                    }
                }
            });
            ui.separator();
            ui.vertical(|ui| {
                let page = pages
                    .iter()
                    .find(|page| self.page.as_deref() == Some(page.id.as_str()));
                if let Some(page) = page
                    && let Some(change) = self.page_drawn(ui, &character, page)
                {
                    asked = Some(HubRequest::Change(change));
                }
            });
        });
        asked
    }

    /// One page: where it is kept, when a change takes effect, and its rows.
    fn page_drawn(&mut self, ui: &mut egui::Ui, character: &str, page: &Page) -> Option<Change> {
        ui.strong(&page.title);
        ui.weak(format!(
            "Saved in {}; a change takes effect {}.",
            page.file, page.takes
        ));
        if let Some(problem) = &page.problem {
            ui.colored_label(ui.visuals().error_fg_color, problem);
            return None;
        }
        let mut wanted = None;
        egui::ScrollArea::vertical()
            .id_salt(("settings-page", &page.id))
            .auto_shrink(false)
            .show(ui, |ui| {
                egui::Grid::new(("settings-rows", &page.id))
                    .num_columns(3)
                    .striped(true)
                    .show(ui, |ui| {
                        for row in &page.rows {
                            if let Some(asked) = self.row(ui, page, row) {
                                wanted = Some(Change {
                                    character: character.to_owned(),
                                    page: page.id.clone(),
                                    key: row.key.clone(),
                                    to: match asked {
                                        Wanted::Set(value) => Some(value),
                                        Wanted::Default => None,
                                    },
                                });
                            }
                            ui.end_row();
                        }
                    });
            });
        wanted
    }

    /// One row: its name, its value to change, and *Use default* when the
    /// file sets it. What it asks, when anything.
    fn row(&mut self, ui: &mut egui::Ui, page: &Page, row: &Row) -> Option<Wanted> {
        let label = ui.label(&row.label).on_hover_text(&row.help);
        let mut wanted = None;
        match (&row.kind, &row.value) {
            (RowKind::Toggle, value) => {
                let mut on = matches!(value, Value::On(true));
                let toggled = ui.add(egui::Checkbox::without_text(&mut on));
                // Its own label is empty; a screen reader hears the row's.
                toggled.widget_info(|| {
                    egui::WidgetInfo::selected(egui::WidgetType::Checkbox, true, on, &row.label)
                });
                if toggled.changed() {
                    wanted = Some(Wanted::Set(if on { "on" } else { "off" }.to_owned()));
                }
            }
            (RowKind::Map, value) => {
                let shown = match value {
                    Value::Map(pairs) if !pairs.is_empty() => pairs
                        .iter()
                        .map(|(name, value)| format!("{name} = {value}"))
                        .collect::<Vec<_>>()
                        .join(", "),
                    _ => "none".to_owned(),
                };
                ui.label(shown).on_hover_text(&row.help);
            }
            (kind, value) => {
                let shown = shown(value);
                let key = (page.id.clone(), row.key.clone());
                let mut text = self
                    .typed
                    .get(&key)
                    .cloned()
                    .unwrap_or_else(|| shown.clone());
                // Sized outright: a grid gives a field only its column's
                // width from the last frame, which would never grow.
                let size = egui::vec2(220.0, ui.spacing().interact_size.y);
                let field = ui
                    .add_sized(size, egui::TextEdit::singleline(&mut text))
                    .labelled_by(label.id);
                if field.has_focus() {
                    self.typed.insert(key, text);
                } else {
                    self.typed.remove(&key);
                    if field.lost_focus() && text != shown {
                        match typed(kind, &text) {
                            Ok(to) => {
                                self.refused = None;
                                wanted = Some(Wanted::Set(to));
                            }
                            Err(why) => self.refused = Some(format!("{}: {why}", row.label)),
                        }
                    }
                }
            }
        }
        if row.here {
            if ui
                .small_button("Use default")
                .on_hover_text("Take this setting out of the file")
                .clicked()
            {
                wanted = Some(Wanted::Default);
            }
        } else {
            ui.weak("default");
        }
        wanted
    }
}

/// A value as a field shows it: a list's items joined by commas.
fn shown(value: &Value) -> String {
    match value {
        Value::Unset => String::new(),
        Value::On(on) => on.to_string(),
        Value::Text(text) => text.clone(),
        Value::List(items) => items.join(", "),
        Value::Map(pairs) => pairs
            .iter()
            .map(|(name, value)| format!("{name} = {value}"))
            .collect::<Vec<_>>()
            .join(", "),
    }
}

/// What was typed for a setting of `kind`, written as it would be typed after
/// `;heal set <key>`: a number as it is, words quoted, a list in brackets.
///
/// # Errors
///
/// What was typed is not of that kind, in words for the player.
pub fn typed(kind: &RowKind, text: &str) -> Result<String, String> {
    let text = text.trim();
    let items = || {
        text.split([',', ' '])
            .map(str::trim)
            .filter(|item| !item.is_empty())
    };
    match kind {
        RowKind::Whole { min, max } => match text.parse::<u32>() {
            Ok(n) if (*min..=*max).contains(&n) => Ok(n.to_string()),
            _ => Err(format!("a whole number from {min} to {max}")),
        },
        RowKind::Number { min, max } => match text.parse::<f64>() {
            Ok(n) if (*min..=*max).contains(&n) => Ok(text.to_owned()),
            _ => Err(format!("a number from {min} to {max}")),
        },
        RowKind::Text => Ok(quoted(text)),
        RowKind::Numbers => {
            let numbers: Result<Vec<u32>, _> = items().map(str::parse::<u32>).collect();
            numbers
                .map(|numbers| {
                    let listed: Vec<String> = numbers.iter().map(u32::to_string).collect();
                    format!("[{}]", listed.join(", "))
                })
                .map_err(|_| "whole numbers, separated by commas".to_owned())
        }
        RowKind::Words => {
            let words: Vec<String> = text
                .split(',')
                .map(str::trim)
                .filter(|word| !word.is_empty())
                .map(quoted)
                .collect();
            Ok(format!("[{}]", words.join(", ")))
        }
        RowKind::Toggle | RowKind::Map => Err("changed another way".to_owned()),
    }
}

/// `text` as a TOML string: in quotes, a quote or a backslash in it escaped.
fn quoted(text: &str) -> String {
    format!("\"{}\"", text.replace('\\', "\\\\").replace('"', "\\\""))
}
