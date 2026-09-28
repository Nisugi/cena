//! The settings menu (`plan/50` §7 step 1), the one every way in opens. The
//! author, 2026-09-27: *"one main settings button to get to the main
//! settings menu. if we add right click context menu options to open
//! settings for widgets/windows then they would open the same main settings
//! menu to the correct spot for that setting."*
//!
//! It opens on Hydra's own pages (step 2): the *Window* page, drawn as any
//! other, and the *Keys* page ([`KeysView`]); the window applies their
//! changes itself. A character is picked from the roster, running or not.
//! Its pages come from the binary as [`Page`]s, which this draws knowing no
//! behavior. Each change goes back as a [`HubRequest::Change`], which the
//! binary applies through the writer the behavior's `;` command uses. So the
//! menu and the command cannot disagree about a value.
//!
//! A text setting is typed and committed on Enter, or when the field is
//! left; a value that is not of its kind is refused here, before anything is
//! sent. *Use default* is offered where the file sets a value.

use std::collections::HashMap;

use cena_ui::settings::{Change, Page, Row, RowKind, Value};
use cena_ui::{HubRequest, RosterCard};

use crate::keys::page::{KeyChange, KeysPage, KeysView};

/// The *Keys* page's id.
const KEYS: &str = "keys";

/// What the settings menu asks for.
#[derive(Clone, Debug, PartialEq)]
pub enum MenuAsked {
    /// Of the binary: a character's pages, or a change to one of them.
    Binary(HubRequest),
    /// A change to Hydra's own *Window* page, which the window makes.
    Own {
        /// The row's key.
        key: String,
        /// Its value, written as the menu writes it; `None` puts it back
        /// to its default.
        to: Option<String>,
    },
    /// A change to the keybinds, which the window writes.
    Key(KeyChange),
}

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
    /// The character whose settings are shown, as the roster names it;
    /// `None` shows Hydra's own.
    character: Option<String>,
    /// The page showing, by its id.
    page: Option<String>,
    /// What is typed into a field, by page and key, while it has the focus.
    typed: HashMap<(String, String), String>,
    /// The character whose pages were last asked for, since the menu opened.
    asked: Option<String>,
    /// What the menu itself last said: why a value was not sent, or what a
    /// change to Hydra's own pages did.
    note: Option<String>,
    /// The *Keys* page.
    keys: KeysPage,
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
    /// Hydra's own pages but the keys: the *Window* page.
    pub own: &'a [Page],
    /// The keybinds, for the *Keys* page.
    pub keys: KeysView<'a>,
}

/// A roster character's name for the binary: `GAME:Name`.
#[must_use]
pub fn roster_name(card: &RosterCard) -> String {
    format!("{}:{}", card.game, card.character)
}

impl Menu {
    /// Open the menu on `character`'s settings, as the roster names it: a
    /// play window's button names its own. `None` opens Hydra's own, as the
    /// hub's button does. A character's pages are asked for afresh.
    pub fn open_for(&mut self, character: Option<String>) {
        self.open_at(character, None);
    }

    /// Open the menu as [`Self::open_for`] does, at `page` (`plan/50` §7
    /// step 8: every other way in opens the one menu at its place). A page
    /// named by the start of its id, `hunt:`, is the first of its kind.
    pub fn open_at(&mut self, character: Option<String>, page: Option<&str>) {
        self.open = true;
        self.asked = None;
        self.character = character;
        if let Some(page) = page {
            self.page = Some(page.to_owned());
        }
    }

    /// The character whose settings are shown, as the roster names it;
    /// `None` while Hydra's own are.
    #[must_use]
    pub fn character(&self) -> Option<&str> {
        self.character.as_deref()
    }

    /// The page showing, by its id; `None` until one is chosen.
    #[must_use]
    pub fn page(&self) -> Option<&str> {
        self.page.as_deref()
    }

    /// Whether the *Keys* page waits for a key to be pressed: the window
    /// then hands it every numpad key ([`KeysView::numpad`]).
    #[must_use]
    pub fn waiting_for_key(&self) -> bool {
        self.open
            && self.character.is_none()
            && self.page.as_deref() == Some(KEYS)
            && self.keys.waiting()
    }

    /// Say `said` at the top of the menu: what a change to Hydra's own
    /// pages did.
    pub fn tell(&mut self, said: String) {
        self.note = Some(said);
    }

    /// Draw the menu over `view`, and return what it asks for.
    pub fn show(&mut self, ui: &mut egui::Ui, view: &MenuView<'_>) -> Option<MenuAsked> {
        self.picker(ui, view);
        if let Some(said) = self.note.as_deref().or(view.said) {
            ui.weak(said);
        }
        ui.separator();
        let Some(character) = self.character.clone() else {
            return self.hydra(ui, view);
        };
        let mut asked = None;
        if self.asked.as_deref() != Some(character.as_str()) {
            self.asked = Some(character.clone());
            self.typed.clear();
            asked = Some(MenuAsked::Binary(HubRequest::Settings(character.clone())));
        }
        let pages = match view.pages {
            Some((whose, pages)) if whose == character => pages,
            _ => {
                ui.weak("Reading the settings...");
                return asked;
            }
        };
        let titles: Vec<(&str, &str)> = pages
            .iter()
            .map(|page| (page.id.as_str(), page.title.as_str()))
            .collect();
        ui.horizontal_top(|ui| {
            self.contents(ui, &titles);
            ui.separator();
            ui.vertical(|ui| {
                let page = pages
                    .iter()
                    .find(|page| self.page.as_deref() == Some(page.id.as_str()));
                if let Some(page) = page
                    && let Some(change) = self.page_drawn(ui, &character, page)
                {
                    self.note = None;
                    asked = Some(MenuAsked::Binary(HubRequest::Change(change)));
                }
            });
        });
        asked
    }

    /// Whose settings: Hydra's own, or a roster character's.
    fn picker(&mut self, ui: &mut egui::Ui, view: &MenuView<'_>) {
        let called = |name: Option<&str>| {
            let Some(name) = name else {
                return "Hydra (every character)".to_owned();
            };
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
        ui.horizontal(|ui| {
            let label = ui.label("Settings for");
            egui::ComboBox::from_id_salt("settings-character")
                .selected_text(called(self.character.as_deref()))
                .show_ui(ui, |ui| {
                    ui.selectable_value(&mut self.character, None, called(None));
                    for card in view.roster {
                        let name = roster_name(card);
                        let shown = called(Some(&name));
                        ui.selectable_value(&mut self.character, Some(name), shown);
                    }
                })
                .response
                .labelled_by(label.id);
        });
    }

    /// The pages' titles down the side, one of them showing: the first,
    /// until another is picked.
    fn contents(&mut self, ui: &mut egui::Ui, titles: &[(&str, &str)]) {
        if self
            .page
            .as_deref()
            .is_none_or(|id| titles.iter().all(|(page, _)| *page != id))
        {
            // A page asked for by the start of its id, or the first.
            let started = self.page.as_deref().and_then(|start| {
                titles
                    .iter()
                    .find(|(page, _)| page.starts_with(start))
                    .map(|(page, _)| *page)
            });
            self.page = started
                .or_else(|| titles.first().map(|(id, _)| *id))
                .map(str::to_owned);
        }
        ui.vertical(|ui| {
            ui.set_width(140.0);
            for (id, title) in titles {
                let showing = self.page.as_deref() == Some(*id);
                if ui.selectable_label(showing, *title).clicked() {
                    self.page = Some((*id).to_owned());
                }
            }
        });
    }

    /// Hydra's own pages: *Window*, drawn as any other page, and *Keys*.
    fn hydra(&mut self, ui: &mut egui::Ui, view: &MenuView<'_>) -> Option<MenuAsked> {
        let mut titles: Vec<(&str, &str)> = view
            .own
            .iter()
            .map(|page| (page.id.as_str(), page.title.as_str()))
            .collect();
        titles.push((KEYS, "Keys"));
        let mut asked = None;
        ui.horizontal_top(|ui| {
            self.contents(ui, &titles);
            ui.separator();
            ui.vertical(|ui| {
                if self.page.as_deref() == Some(KEYS) {
                    asked = self
                        .keys
                        .show(ui, &view.keys, &mut self.note)
                        .map(MenuAsked::Key);
                    return;
                }
                let page = view
                    .own
                    .iter()
                    .find(|page| self.page.as_deref() == Some(page.id.as_str()));
                if let Some(page) = page
                    && let Some(change) = self.page_drawn(ui, "", page)
                {
                    asked = Some(MenuAsked::Own {
                        key: change.key,
                        to: change.to,
                    });
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
        let tables = page
            .rows
            .iter()
            .filter_map(|row| row.key.split_once('.').map(|(table, _)| table))
            .collect::<std::collections::BTreeSet<_>>()
            .len();
        egui::ScrollArea::vertical()
            .id_salt(("settings-page", &page.id))
            .auto_shrink(false)
            .show(ui, |ui| {
                egui::Grid::new(("settings-rows", &page.id))
                    .num_columns(3)
                    .striped(true)
                    .show(ui, |ui| {
                        let mut table = None;
                        for row in &page.rows {
                            // A heading where a page's settings are in
                            // several tables: a hunt profile's.
                            let this = row.key.split_once('.').map(|(table, _)| table);
                            if tables > 1 && this.is_some() && this != table {
                                ui.strong(this.unwrap_or_default());
                                ui.end_row();
                            }
                            table = this;
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
                                self.note = None;
                                wanted = Some(Wanted::Set(to));
                            }
                            Err(why) => self.note = Some(format!("{}: {why}", row.label)),
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
            ui.weak(row.from.as_deref().unwrap_or("default"));
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
