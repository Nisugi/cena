//! A bar widget's page: its rows and a change to its look. Split out of
//! `options.rs` at the file cap (`plan/05` Rule 4.1).

use std::path::PathBuf;

use cena_ui::settings::{Row, RowKind, Value};

use super::{FILLS, PLACES, choice, on_off, overlay_choice};
use crate::bar::{Fills, Look, Place};

/// `look`'s `key` set to `to`, or to `default`'s.
pub(super) fn set(
    look: &mut Look,
    default: &Look,
    key: &str,
    to: Option<&str>,
) -> Result<(), String> {
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
        ("clock", to) => look.clock = to.map_or(Ok(default.clock), on)?,
        ("color", Some(to)) => {
            look.color =
                Some(crate::menu::rgb(to).ok_or_else(|| format!("`{to}` is not a colour."))?);
        }
        ("color", None) => look.color = None,
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

/// A bar widget's rows: how it draws, as `look` says, or its kind's own.
pub(super) fn bar_rows(
    look: Option<&Look>,
    default: &Look,
    overlays: &[PathBuf],
    clock: bool,
) -> Vec<Row> {
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
            "Its fill's colour; empty, the theme's.",
            RowKind::Color,
            Value::Text(now.color.map(crate::menu::hex).unwrap_or_default()),
            now.color.is_some(),
        ),
    ]
    .into_iter()
    .chain(clock.then(|| clock_row(now, default)))
    .chain(image_rows(now, overlays))
    .collect()
}

/// The pulse's Clock row: just a clock, in place of its bar.
fn clock_row(now: &Look, default: &Look) -> Row {
    Row {
        key: "clock".to_owned(),
        label: "Clock".to_owned(),
        help: "Just a clock: seconds to the earliest pulse, then below zero until it comes."
            .to_owned(),
        kind: RowKind::Toggle,
        value: Value::On(now.clock),
        here: now.clock != default.clock,
        from: None,
    }
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
