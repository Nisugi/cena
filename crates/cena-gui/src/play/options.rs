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
//! widget's page holds its [`Look`]; the Room widget's, which of its parts
//! it shows ([`RoomParts`]): *"it should take you to pick which streams show
//! in the room window"*.

use std::path::PathBuf;

use cena_ui::settings::{Page, Row, RowKind, Value};

use super::Play;
use crate::bar::{Fills, Look, Place};
use crate::layout::Holds;
use crate::widget::{RoomParts, Widget};

/// What a widget page's id begins with; the widget's id follows.
pub(crate) const PREFIX: &str = "widget:";

/// The page id of widget `placed`.
pub(crate) fn page_id(placed: u32) -> String {
    format!("{PREFIX}{placed}")
}

/// Whether `widget` has a page of its own.
pub(crate) fn has_page(widget: &Widget) -> bool {
    widget.bar_look().is_some() || *widget == Widget::Room
}

/// The Room widget's parts, as its page names each: its key, what a player
/// calls it, and a line of what it is.
const PARTS: [(&str, &str, &str); 7] = [
    (
        "title",
        "Title",
        "The room's name, and the game's number for it.",
    ),
    ("description", "Description", "What the room looks like."),
    (
        "objects",
        "Objects",
        "What else is here: the game's \"You also see\".",
    ),
    (
        "creatures",
        "Creatures",
        "The creatures here, on their own line when they stand apart.",
    ),
    ("players", "Players", "Who else is here, when anyone is."),
    ("exits", "Exits", "The ways out."),
    (
        "apart",
        "Creatures apart",
        "Objects and creatures each on a line of their own, split by the game's bold, rather than run on after the description.",
    ),
];

/// The part of `parts` a page's `key` names.
fn part<'a>(parts: &'a mut RoomParts, key: &str) -> Option<&'a mut bool> {
    Some(match key {
        "title" => &mut parts.title,
        "description" => &mut parts.description,
        "objects" => &mut parts.objects,
        "creatures" => &mut parts.creatures,
        "players" => &mut parts.players,
        "exits" => &mut parts.exits,
        "apart" => &mut parts.apart,
        _ => return None,
    })
}

/// Whether the part a page's `key` names is on in `parts`.
fn shows(mut parts: RoomParts, key: &str) -> bool {
    part(&mut parts, key).is_some_and(|on| *on)
}

/// Which way a bar fills, as the page names each.
const FILLS: [(Fills, &str, &str); 6] = [
    (Fills::Right, "right", "Across, from the left"),
    (Fills::Left, "left", "Across, from the right"),
    (Fills::Up, "up", "Upright, from the bottom"),
    (Fills::Down, "down", "Upright, from the top"),
    (Fills::Orb, "orb", "Orb, filling from the bottom"),
    (Fills::Ring, "ring", "Ring, clockwise from the top"),
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
                let rows = match one.widget.bar_look() {
                    Some(default) => bar_rows(layout.looks.get(&one.id), &default, overlays),
                    None if one.widget == Widget::Room => room_rows(layout.rooms.get(&one.id)),
                    None => continue,
                };
                let name = one.widget.name();
                pages.push(Page {
                    id: page_id(one.id),
                    title: window
                        .map_or_else(|| name.to_string(), |window| format!("{name} ({window})")),
                    file: file.clone(),
                    takes: "at once".to_owned(),
                    problem: None,
                    rows,
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
        if widget == Widget::Room {
            let mut parts = layout.rooms.get(&placed).copied().unwrap_or_default();
            let default = RoomParts::default();
            let asked = to.map(|to| match to {
                "on" => Ok(true),
                "off" => Ok(false),
                other => Err(format!("`{other}` is not on or off.")),
            });
            let now =
                part(&mut parts, key).ok_or_else(|| format!("The room has no part {key}."))?;
            *now = match asked {
                Some(asked) => asked?,
                None => shows(default, key),
            };
            if parts == default {
                layout.rooms.remove(&placed);
            } else {
                layout.rooms.insert(placed, parts);
            }
            self.save();
            return Ok("Room: changed.".to_owned());
        }
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
        ("ring", Some(to)) => {
            look.ring = to
                .parse::<u8>()
                .ok()
                .filter(|width| (RING_LEAST..=100).contains(width))
                .ok_or_else(|| format!("`{to}` is not a thickness from {RING_LEAST} to 100."))?;
        }
        ("ring", None) => look.ring = default.ring,
        ("overlay", Some("") | None) => look.overlay = None,
        ("overlay", Some(path)) => look.overlay = Some(path.to_owned()),
        ("background", Some("") | None) => look.background = None,
        ("background", Some(path)) => look.background = Some(path.to_owned()),
        ("fill_image", Some("") | None) => look.fill_image = None,
        ("fill_image", Some(path)) => look.fill_image = Some(path.to_owned()),
        (key, _) => return Err(format!("A bar has no setting {key}.")),
    }
    Ok(())
}

/// The Room widget's rows: each part, on or off, as `parts` says or all.
fn room_rows(parts: Option<&RoomParts>) -> Vec<Row> {
    let default = RoomParts::default();
    let now = parts.copied().unwrap_or_default();
    PARTS
        .iter()
        .map(|(key, label, help)| {
            let on = shows(now, key);
            let was = shows(default, key);
            Row {
                key: (*key).to_owned(),
                label: (*label).to_owned(),
                help: (*help).to_owned(),
                kind: RowKind::Toggle,
                value: Value::On(on),
                here: on != was,
                from: None,
            }
        })
        .collect()
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
            "Across or upright and from which edge, or round: an orb or a ring. A bar takes its whole space, a round one the square in its middle: size it by its window or cell.",
            choice(FILLS.iter().map(|(_, value, called)| (*value, *called))),
            Value::Text(fills_value(now.fills)),
            now.fills != default.fills,
        ),
        row(
            "ring",
            "Ring thickness",
            "A ring's thickness, in percent of its radius: 100 is a disc.",
            RowKind::Whole {
                min: u32::from(RING_LEAST),
                max: 100,
            },
            Value::Text(now.ring.to_string()),
            now.ring != default.ring,
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
    ]
    .into_iter()
    .chain(image_rows(now, overlays))
    .collect()
}

/// A bar widget's images, as `now` has them: the fill's, the one under it
/// and the one over it, each any PNG in the data folder's overlays folder. An
/// orb's or a ring's is laid over the square it sits in.
fn image_rows(now: &Look, overlays: &[PathBuf]) -> Vec<Row> {
    let row = |key: &str, label: &str, help: &str, set: &Option<String>| Row {
        key: key.to_owned(),
        label: label.to_owned(),
        help: help.to_owned(),
        kind: overlay_choice(overlays),
        value: Value::Text(set.clone().unwrap_or_default()),
        here: set.is_some(),
        from: None,
    };
    vec![
        row(
            "fill_image",
            "Fill image",
            "An image the fill uncovers as it fills, in place of its colour: a liquid.",
            &now.fill_image,
        ),
        row(
            "background",
            "Background",
            "An image under the fill, in place of the empty part: an orb's glass.",
            &now.background,
        ),
        row(
            "overlay",
            "Overlay",
            "An image laid over the bar and its fill, stretched: a frame, a gloss.",
            &now.overlay,
        ),
    ]
}

/// The thinnest a ring may be, in percent of its radius.
const RING_LEAST: u8 = 5;
