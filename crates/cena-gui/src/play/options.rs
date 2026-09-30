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

use std::collections::BTreeMap;
use std::path::PathBuf;

use cena_ui::settings::{Page, Row, RowKind, Value};

use super::Play;
use crate::bar::{Fills, Look, Place};
use crate::story::Hours;
use crate::widget::doll::{DollLook, Style};
#[cfg(feature = "doll-infinite")]
use crate::widget::infinite::BARE;
use crate::widget::{Lines, RoomParts, Stamps, Widget};

/// What a widget page's id begins with; the widget's id follows.
pub(crate) const PREFIX: &str = "widget:";

/// The page id of widget `placed`.
pub(crate) fn page_id(placed: u32) -> String {
    format!("{PREFIX}{placed}")
}

/// Whether `widget` has a page of its own.
pub(crate) fn has_page(widget: &Widget) -> bool {
    widget.bar_look().is_some()
        || matches!(
            widget,
            Widget::Room | Widget::Story | Widget::Stream(_) | Widget::Injuries
        )
}

/// The pictures a widget's page offers, from the data folder: a bar's
/// images in `overlays`, a doll's in `dolls` (`plan/55`).
#[derive(Clone, Debug, Default)]
pub(crate) struct Pictures {
    /// Each PNG in `overlays`.
    pub(crate) overlays: Vec<PathBuf>,
    /// Each doll picture in `dolls`, its overlays left out.
    pub(crate) dolls: Vec<PathBuf>,
}

/// Where a line's time goes, as the page names each.
const STAMPS: [(Stamps, &str, &str); 3] = [
    (Stamps::None, "none", "None"),
    (Stamps::Start, "start", "At the start"),
    (Stamps::End, "end", "At the end"),
];

/// The clock a time is said on, as the page names each.
const HOURS: [(Hours, &str, &str); 2] = [
    (Hours::Twelve, "12", "12-hour, AM/PM"),
    (Hours::TwentyFour, "24", "24-hour"),
];

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
    pub(crate) fn widget_pages(&self, pictures: &Pictures) -> Vec<Page> {
        let Some(layout) = &self.layout else {
            return Vec::new();
        };
        let file = self
            .layouts
            .as_ref()
            .map_or_else(String::new, |dir| dir.display().to_string());
        let mut pages = Vec::new();
        for holder in &layout.holders {
            let window = holder.holds.title();
            for one in holder.holds.placed() {
                let rows = match (&one.widget, one.widget.bar_look()) {
                    (_, Some(default)) => {
                        bar_rows(layout.looks.get(&one.id), &default, &pictures.overlays)
                    }
                    (Widget::Injuries, None) => {
                        doll_rows(layout.dolls.get(&one.id), &pictures.dolls)
                    }
                    (Widget::Room, None) => room_rows(layout.rooms.get(&one.id)),
                    (Widget::Minimap, None) => {
                        crate::widget::minimap::look::rows(layout.minimaps.get(&one.id))
                    }
                    (Widget::Story | Widget::Stream(_), None) => {
                        lines_rows(layout.lines.get(&one.id), one.widget == Widget::Story)
                    }
                    _ => continue,
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
            .placed()
            .into_iter()
            .find(|one| one.id == placed)
            .map(|one| one.widget.clone())
            .ok_or_else(|| "That widget is no longer in the window.".to_owned())?;
        match &widget {
            Widget::Room => keep(&mut layout.rooms, placed, &RoomParts::default(), |parts| {
                room_set(parts, key, to)
            })?,
            Widget::Injuries => keep(&mut layout.dolls, placed, &DollLook::default(), |look| {
                doll_set(look, key, to)
            })?,
            Widget::Minimap => keep(
                &mut layout.minimaps,
                placed,
                &crate::widget::minimap::MinimapLook::default(),
                |look| crate::widget::minimap::look::set(look, key, to),
            )?,
            Widget::Story | Widget::Stream(_) => {
                let story = widget == Widget::Story;
                keep(&mut layout.lines, placed, &Lines::default(), |lines| {
                    lines_set(lines, key, to, story)
                })?;
            }
            _ => {
                let default = widget
                    .bar_look()
                    .ok_or_else(|| format!("{} has no settings of its own.", widget.name()))?;
                keep(&mut layout.looks, placed, &default, |look| {
                    set(look, &default, key, to)
                })?;
            }
        }
        self.save();
        Ok(format!("{}: changed.", widget.name()))
    }
}

/// Widget `placed`'s entry in `kept` changed by `change`: kept only while it
/// differs from its kind's own, `default`.
fn keep<T: Clone + PartialEq>(
    kept: &mut BTreeMap<u32, T>,
    placed: u32,
    default: &T,
    change: impl FnOnce(&mut T) -> Result<(), String>,
) -> Result<(), String> {
    let mut now = kept
        .get(&placed)
        .cloned()
        .unwrap_or_else(|| default.clone());
    change(&mut now)?;
    if now == *default {
        kept.remove(&placed);
    } else {
        kept.insert(placed, now);
    }
    Ok(())
}

/// `on` or `off`, as the menu writes a switch.
fn on_off(to: &str) -> Result<bool, String> {
    match to {
        "on" => Ok(true),
        "off" => Ok(false),
        other => Err(format!("`{other}` is not on or off.")),
    }
}

/// The Room's part `key` set to `to`, or to all shown.
fn room_set(parts: &mut RoomParts, key: &str, to: Option<&str>) -> Result<(), String> {
    let shown = match to {
        Some(to) => on_off(to)?,
        None => shows(RoomParts::default(), key),
    };
    *part(parts, key).ok_or_else(|| format!("The room has no part {key}."))? = shown;
    Ok(())
}

/// How lines are drawn, `key` set to `to` or to the kind's own; the
/// prompts and what was typed only on the `story`.
fn lines_set(lines: &mut Lines, key: &str, to: Option<&str>, story: bool) -> Result<(), String> {
    let default = Lines::default();
    let switch = |to: Option<&str>, default: bool| to.map_or(Ok(default), on_off);
    match (key, to) {
        ("stamps", Some(to)) => {
            lines.stamps = STAMPS
                .iter()
                .find(|(_, value, _)| *value == to)
                .map(|(stamps, ..)| *stamps)
                .ok_or_else(|| format!("A line's time does not go `{to}`."))?;
        }
        ("stamps", None) => lines.stamps = default.stamps,
        ("hours", Some(to)) => {
            lines.hours = HOURS
                .iter()
                .find(|(_, value, _)| *value == to)
                .map(|(hours, ..)| *hours)
                .ok_or_else(|| format!("There is no `{to}`-hour clock."))?;
        }
        ("hours", None) => lines.hours = default.hours,
        ("seconds", to) => lines.seconds = switch(to, default.seconds)?,
        ("wrap", to) => lines.wrap = switch(to, default.wrap)?,
        ("prompts", to) if story => lines.prompts = switch(to, default.prompts)?,
        ("echo", to) if story => lines.echo = switch(to, default.echo)?,
        (key, _) => return Err(format!("It has no setting {key}.")),
    }
    Ok(())
}

/// `look`'s `key` set to `to`, or to `default`'s.
fn set(look: &mut Look, default: &Look, key: &str, to: Option<&str>) -> Result<(), String> {
    let on = on_off;
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
        ("words", to) => look.says.words = to.map_or(Ok(default.says.words), on)?,
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

/// A story's or a stream's rows: how it draws its lines, as `lines` says or
/// as it always has; the prompts and what was typed only on the `story`.
fn lines_rows(lines: Option<&Lines>, story: bool) -> Vec<Row> {
    let default = Lines::default();
    let now = lines.copied().unwrap_or_default();
    let row = |key: &str, label: &str, help: &str, kind: RowKind, value: Value, here: bool| Row {
        key: key.to_owned(),
        label: label.to_owned(),
        help: help.to_owned(),
        kind,
        value,
        here,
        from: None,
    };
    let switch = |key: &str, label: &str, help: &str, now: bool, was: bool| {
        row(
            key,
            label,
            help,
            RowKind::Toggle,
            Value::On(now),
            now != was,
        )
    };
    let stamps = STAMPS
        .iter()
        .find(|(each, ..)| *each == now.stamps)
        .map_or("", |(_, value, _)| value);
    let hours = HOURS
        .iter()
        .find(|(each, ..)| *each == now.hours)
        .map_or("", |(_, value, _)| value);
    let mut rows = vec![
        row(
            "stamps",
            "Timestamps",
            "When each line arrived, on your clock: before it, after it, or not at all.",
            choice(STAMPS.iter().map(|(_, value, called)| (*value, *called))),
            Value::Text(stamps.to_owned()),
            now.stamps != default.stamps,
        ),
        switch(
            "seconds",
            "With seconds",
            "7:08:05 rather than 7:08.",
            now.seconds,
            default.seconds,
        ),
        row(
            "hours",
            "Clock",
            "7:08 PM, or 19:08.",
            choice(HOURS.iter().map(|(_, value, called)| (*value, *called))),
            Value::Text(hours.to_owned()),
            now.hours != default.hours,
        ),
        switch(
            "wrap",
            "Word wrap",
            "A long line wraps; off, the window scrolls sideways.",
            now.wrap,
            default.wrap,
        ),
    ];
    if story {
        rows.push(switch(
            "prompts",
            "Prompts",
            "The game's prompt, >, after what it says.",
            now.prompts,
            default.prompts,
        ));
        rows.push(switch(
            "echo",
            "What you type",
            "Each line you send, after the prompt it followed.",
            now.echo,
            default.echo,
        ));
    }
    rows
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

/// The Injuries widget's rows: its picture, any in the data folder's
/// `dolls`, or none for the body drawn in code.
fn doll_rows(look: Option<&DollLook>, dolls: &[PathBuf]) -> Vec<Row> {
    let picture = look.and_then(|look| look.picture.clone());
    let style = look.map(|look| look.style).unwrap_or_default();
    let mut styles = vec![
        ("doll".to_owned(), "Doll".to_owned()),
        ("text".to_owned(), "Text".to_owned()),
    ];
    if cfg!(feature = "doll-infinite") {
        styles.push(("infinite".to_owned(), "Infinite".to_owned()));
    }
    let mut rows = vec![Row {
        key: "style".to_owned(),
        label: "Style".to_owned(),
        help: "The Doll: dots on a picture or a body, or a picture's own art. Text: a line \
               for each part hurt or scarred. Infinite: a puppet that moves as your \
               character does."
            .to_owned(),
        kind: RowKind::Choice(styles),
        value: Value::Text(
            match style {
                Style::Doll => "doll",
                Style::Text => "text",
                Style::Infinite => "infinite",
            }
            .to_owned(),
        ),
        here: style != Style::Doll,
        from: None,
    }];
    rows.push(Row {
        key: "picture".to_owned(),
        label: "Picture".to_owned(),
        help: "A picture of your own for the doll, from the dolls folder in Hydra's data \
               folder; None draws a body. Calibrate it from the doll's right-click menu."
            .to_owned(),
        kind: overlay_choice(dolls),
        value: Value::Text(picture.clone().unwrap_or_default()),
        here: picture.is_some(),
        from: None,
    });
    #[cfg(feature = "doll-infinite")]
    if style == Style::Infinite {
        let skin = look.and_then(|look| look.skin.clone());
        let mut skins = vec![
            (String::new(), "Lay figure".to_owned()),
            (BARE.to_owned(), "The form's own".to_owned()),
        ];
        skins.extend(
            crate::widget::infinite::skins()
                .into_iter()
                .map(|skin| (skin.clone(), skin.replace('_', " "))),
        );
        rows.push(Row {
            key: "skin".to_owned(),
            label: "Skin".to_owned(),
            help: "What the Infinite puppet wears: the lay figure, the form's own \
                   texture, or any skin gs_studio has for it."
                .to_owned(),
            kind: RowKind::Choice(skins),
            value: Value::Text(skin.clone().unwrap_or_default()),
            here: skin.is_some(),
            from: None,
        });
    }
    rows
}

/// Set the Injuries widget's `key` to `to`, or back to its own.
fn doll_set(look: &mut DollLook, key: &str, to: Option<&str>) -> Result<(), String> {
    match key {
        "picture" => {
            look.picture = to.filter(|path| !path.is_empty()).map(str::to_owned);
            Ok(())
        }
        #[cfg(feature = "doll-infinite")]
        "skin" => {
            let skin = to.filter(|skin| !skin.is_empty());
            if let Some(skin) = skin
                && skin != BARE
                && !crate::widget::infinite::skins()
                    .iter()
                    .any(|one| one == skin)
            {
                return Err(format!("Injuries has no skin {skin}."));
            }
            look.skin = skin.map(str::to_owned);
            Ok(())
        }
        "style" => {
            look.style = match to {
                None | Some("doll") => Style::Doll,
                Some("infinite") => Style::Infinite,
                Some("text") => Style::Text,
                Some(other) => return Err(format!("Injuries has no style {other}.")),
            };
            Ok(())
        }
        _ => Err(format!("Injuries has no setting {key}.")),
    }
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
            "Its name: HP, MP, Stance, Mind.",
            now.says.label,
            default.says.label,
        ),
        // A bar the game words has its word to say, and no current/max.
        if default.says.words {
            says(
                "words",
                "Says the game's word",
                "What the game calls it: offensive, Light, clear as a bell.",
                now.says.words,
                default.says.words,
            )
        } else {
            says(
                "numbers",
                "Says current/max",
                "350/400, when the game has said both.",
                now.says.numbers,
                default.says.numbers,
            )
        },
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
