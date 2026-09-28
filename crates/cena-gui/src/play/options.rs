//! A widget's own page in the settings menu (the author, 2026-09-28: a
//! health bar's right-click, *Settings...*, *"should take you to settings to
//! edit that bar, is it horizontal or vertical, is the text inside it or
//! outside it, is it just a percent, or just current/max, or text label plus
//! numbers, or any combination, what color is it? what overlay am I
//! using"*). Every other way in opens the one menu at its place
//! (`plan/50` §6 item 9); for a widget, its place is its own page.
//!
//! The pages are the play window's, built from its layout, and a change is
//! made to the layout and saved with it: no behavior is behind them. A bar
//! widget's page holds its [`Look`].

use std::path::PathBuf;

use cena_ui::settings::{Page, Row, RowKind, Value};

use super::Play;
use crate::bar::{Fills, Look, Place};
use crate::layout::Holds;
use crate::widget::Widget;

/// What a widget page's id begins with; the widget's id follows.
pub(crate) const PREFIX: &str = "widget:";

/// The page id of widget `placed`.
pub(crate) fn page_id(placed: u32) -> String {
    format!("{PREFIX}{placed}")
}

/// Whether `widget` has a page of its own.
pub(crate) fn has_page(widget: &Widget) -> bool {
    widget.bar_look().is_some()
}

/// Which way a bar fills, as the page names each.
const FILLS: [(Fills, &str, &str); 4] = [
    (Fills::Right, "right", "Across, from the left"),
    (Fills::Left, "left", "Across, from the right"),
    (Fills::Up, "up", "Upright, from the bottom"),
    (Fills::Down, "down", "Upright, from the top"),
];

/// Where a bar's text goes, as the page names each.
const PLACES: [(Place, &str, &str); 6] = [
    (Place::Inside, "inside", "Inside"),
    (Place::Above, "above", "Above"),
    (Place::Below, "below", "Below"),
    (Place::Left, "left", "Left"),
    (Place::Right, "right", "Right"),
    (Place::Hidden, "hidden", "None"),
];

impl Play {
    /// A page for each widget in this window that has one, in the order its
    /// windows are drawn; `overlays` the images a bar may lay over itself.
    pub(crate) fn widget_pages(&self, overlays: &[PathBuf]) -> Vec<Page> {
        let Some(layout) = &self.layout else {
            return Vec::new();
        };
        let file = self
            .layouts
            .as_ref()
            .map_or_else(String::new, |dir| dir.display().to_string());
        let mut pages = Vec::new();
        for holder in &layout.holders {
            let (window, placed): (Option<&str>, Vec<_>) = match &holder.holds {
                Holds::One(one) => (None, vec![one]),
                Holds::Custom(custom) => (
                    Some(custom.title.as_str()),
                    custom
                        .cells
                        .iter()
                        .flat_map(|cell| cell.tabs.iter())
                        .collect(),
                ),
            };
            for one in placed {
                let Some(default) = one.widget.bar_look() else {
                    continue;
                };
                let name = one.widget.name();
                pages.push(Page {
                    id: page_id(one.id),
                    title: window
                        .map_or_else(|| name.to_string(), |window| format!("{name} ({window})")),
                    file: file.clone(),
                    takes: "at once".to_owned(),
                    problem: None,
                    rows: bar_rows(layout.looks.get(&one.id), &default, overlays),
                });
            }
        }
        pages
    }

    /// Set `key` on the widget page `page` to `to`, as the menu writes it, or
    /// back to the kind's own (`None`); the layout saved. What was done.
    ///
    /// # Errors
    ///
    /// Why nothing was: no such widget, or not a value it takes.
    pub(crate) fn widget_change(
        &mut self,
        page: &str,
        key: &str,
        to: Option<&str>,
    ) -> Result<String, String> {
        let placed = page
            .strip_prefix(PREFIX)
            .and_then(|id| id.parse::<u32>().ok())
            .ok_or_else(|| format!("There is no {page} page."))?;
        let layout = self
            .layout
            .as_mut()
            .ok_or_else(|| "The window has no layout yet.".to_owned())?;
        let widget = layout
            .holders
            .iter()
            .flat_map(|holder| match &holder.holds {
                Holds::One(one) => vec![one],
                Holds::Custom(custom) => custom
                    .cells
                    .iter()
                    .flat_map(|cell| cell.tabs.iter())
                    .collect(),
            })
            .find(|one| one.id == placed)
            .map(|one| one.widget.clone())
            .ok_or_else(|| "That widget is no longer in the window.".to_owned())?;
        let default = widget
            .bar_look()
            .ok_or_else(|| format!("{} has no settings of its own.", widget.name()))?;
        let mut look = layout
            .looks
            .get(&placed)
            .cloned()
            .unwrap_or_else(|| default.clone());
        set(&mut look, &default, key, to)?;
        if look == default {
            layout.looks.remove(&placed);
        } else {
            layout.looks.insert(placed, look);
        }
        self.save();
        Ok(format!("{}: changed.", widget.name()))
    }
}

/// `look`'s `key` set to `to`, or to `default`'s.
fn set(look: &mut Look, default: &Look, key: &str, to: Option<&str>) -> Result<(), String> {
    let on = |to: &str| match to {
        "on" => Ok(true),
        "off" => Ok(false),
        other => Err(format!("`{other}` is not on or off.")),
    };
    match (key, to) {
        ("fills", Some(to)) => {
            look.fills = FILLS
                .iter()
                .find(|(_, value, _)| *value == to)
                .map(|(fills, ..)| *fills)
                .ok_or_else(|| format!("A bar does not fill `{to}`."))?;
        }
        ("fills", None) => look.fills = default.fills,
        ("text", Some(to)) => {
            look.place = PLACES
                .iter()
                .find(|(_, value, _)| *value == to)
                .map(|(place, ..)| *place)
                .ok_or_else(|| format!("A bar's text does not go `{to}`."))?;
        }
        ("text", None) => look.place = default.place,
        ("label", to) => look.says.label = to.map_or(Ok(default.says.label), on)?,
        ("numbers", to) => look.says.numbers = to.map_or(Ok(default.says.numbers), on)?,
        ("percent", to) => look.says.percent = to.map_or(Ok(default.says.percent), on)?,
        ("color", Some(to)) => {
            look.color = crate::menu::rgb(to).ok_or_else(|| format!("`{to}` is not a colour."))?;
        }
        ("color", None) => look.color = default.color,
        ("overlay", Some("") | None) => look.overlay = None,
        ("overlay", Some(path)) => look.overlay = Some(path.to_owned()),
        (key, _) => return Err(format!("A bar has no setting {key}.")),
    }
    Ok(())
}

/// A choice among `named`, each its value and what a player calls it.
fn choice<'a>(named: impl Iterator<Item = (&'a str, &'a str)>) -> RowKind {
    RowKind::Choice(
        named
            .map(|(value, called)| (value.to_owned(), called.to_owned()))
            .collect(),
    )
}

/// How `fills` is written on the page.
fn fills_value(fills: Fills) -> String {
    FILLS
        .iter()
        .find(|(each, ..)| *each == fills)
        .map_or("", |(_, value, _)| value)
        .to_owned()
}

/// How `place` is written on the page.
fn place_value(place: Place) -> String {
    PLACES
        .iter()
        .find(|(each, ..)| *each == place)
        .map_or("", |(_, value, _)| value)
        .to_owned()
}

/// The overlays to choose from: none, then each image by its file's name.
fn overlay_choice(overlays: &[PathBuf]) -> RowKind {
    let mut images: Vec<(String, String)> = vec![(String::new(), "None".to_owned())];
    images.extend(overlays.iter().map(|path| {
        let called = path.file_name().map_or_else(
            || path.display().to_string(),
            |name| name.to_string_lossy().into_owned(),
        );
        (path.display().to_string(), called)
    }));
    RowKind::Choice(images)
}

/// A bar widget's rows: how it draws, as `look` says, or its kind's own.
fn bar_rows(look: Option<&Look>, default: &Look, overlays: &[PathBuf]) -> Vec<Row> {
    let now = look.unwrap_or(default);
    let row = |key: &str, label: &str, help: &str, kind: RowKind, value: Value, here: bool| Row {
        key: key.to_owned(),
        label: label.to_owned(),
        help: help.to_owned(),
        kind,
        value,
        here,
        from: None,
    };
    let says = |key: &str, label: &str, help: &str, now: bool, default: bool| {
        row(
            key,
            label,
            help,
            RowKind::Toggle,
            Value::On(now),
            now != default,
        )
    };
    vec![
        row(
            "fills",
            "Fills",
            "Across or upright, and from which edge. An upright bar takes its whole space: size it by its window or cell.",
            choice(FILLS.iter().map(|(_, value, called)| (*value, *called))),
            Value::Text(fills_value(now.fills)),
            now.fills != default.fills,
        ),
        row(
            "text",
            "Text",
            "Where its words go: inside, beside it, or none.",
            choice(PLACES.iter().map(|(_, value, called)| (*value, *called))),
            Value::Text(place_value(now.place)),
            now.place != default.place,
        ),
        says(
            "label",
            "Says its label",
            "HP, MP, SP, Sp.",
            now.says.label,
            default.says.label,
        ),
        says(
            "numbers",
            "Says current/max",
            "350/400, when the game has said both.",
            now.says.numbers,
            default.says.numbers,
        ),
        says(
            "percent",
            "Says its percent",
            "87%.",
            now.says.percent,
            default.says.percent,
        ),
        row(
            "color",
            "Colour",
            "Its fill's colour.",
            RowKind::Color,
            Value::Text(crate::menu::hex(now.color)),
            now.color != default.color,
        ),
        row(
            "overlay",
            "Overlay",
            "An image laid over the bar, stretched: any PNG in the data folder's overlays folder.",
            overlay_choice(overlays),
            Value::Text(now.overlay.clone().unwrap_or_default()),
            now.overlay.is_some(),
        ),
    ]
}
