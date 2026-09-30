//! The Injuries widget's page: its rows and a change to its look. Split
//! out of `options.rs` at its cap (`plan/05` Rule 4.1).

use std::path::PathBuf;

use cena_ui::settings::{Row, RowKind, Value};

use super::overlay_choice;
use crate::widget::doll::{Backdrop, DollLook, Style};
#[cfg(feature = "doll-infinite")]
use crate::widget::infinite::BARE;

/// The Injuries widget's rows: its picture, any in the data folder's
/// `dolls`, or none for the body drawn in code.
pub(super) fn doll_rows(look: Option<&DollLook>, dolls: &[PathBuf]) -> Vec<Row> {
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
        let backdrop = look.map_or(Backdrop::Day, |look| look.backdrop);
        rows.push(Row {
            key: "backdrop".to_owned(),
            label: "Backdrop".to_owned(),
            help: "What the puppet stands in front of: a day sky, gs_studio's dark display, \
                   black, or a colour of your own."
                .to_owned(),
            kind: RowKind::Choice(
                BACKDROPS
                    .iter()
                    .map(|(_, value, called)| ((*value).to_owned(), (*called).to_owned()))
                    .collect(),
            ),
            value: Value::Text(backdrop_value(backdrop)),
            here: backdrop != Backdrop::Day,
            from: None,
        });
        if backdrop == Backdrop::Colour {
            let colour = look.and_then(|look| look.colour.clone());
            rows.push(Row {
                key: "colour".to_owned(),
                label: "Colour".to_owned(),
                help: "The colour behind the puppet.".to_owned(),
                kind: RowKind::Color,
                value: Value::Text(colour.clone().unwrap_or_else(|| "#000000".to_owned())),
                here: colour.is_some(),
                from: None,
            });
        }
    }
    rows
}

/// The Injuries widget's backdrops: each, how it is written, its name.
const BACKDROPS: [(Backdrop, &str, &str); 4] = [
    (Backdrop::Day, "day", "Day sky"),
    (Backdrop::Display, "display", "Display"),
    (Backdrop::Black, "black", "Black"),
    (Backdrop::Colour, "colour", "Solid colour"),
];

/// How `backdrop` is written on the page.
#[cfg_attr(
    not(feature = "doll-infinite"),
    expect(dead_code, reason = "the doll's page")
)]
fn backdrop_value(backdrop: Backdrop) -> String {
    BACKDROPS
        .iter()
        .find(|(each, ..)| *each == backdrop)
        .map_or("", |(_, value, _)| value)
        .to_owned()
}

/// Set the Injuries widget's `key` to `to`, or back to its own.
pub(super) fn doll_set(look: &mut DollLook, key: &str, to: Option<&str>) -> Result<(), String> {
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
        "facing" => {
            look.facing = match to.map(str::trim).filter(|to| !to.is_empty()) {
                None => None,
                Some(to) => {
                    let deg: f32 = to
                        .trim_end_matches('\u{b0}')
                        .parse()
                        .ok()
                        .filter(|deg: &f32| deg.is_finite())
                        .ok_or_else(|| format!("`{to}` is not a facing in degrees."))?;
                    #[expect(
                        clippy::cast_possible_truncation,
                        reason = "wrapped to -180..180 first"
                    )]
                    Some(((deg + 180.0).rem_euclid(360.0) - 180.0).round() as i16)
                }
            };
            Ok(())
        }
        "backdrop" => {
            look.backdrop = match to {
                None => Backdrop::Day,
                Some(to) => BACKDROPS
                    .iter()
                    .find(|(_, value, _)| *value == to)
                    .map(|(each, ..)| *each)
                    .ok_or_else(|| format!("Injuries has no backdrop {to}."))?,
            };
            Ok(())
        }
        "colour" => {
            look.colour = match to.filter(|colour| !colour.is_empty()) {
                None => None,
                Some(to) => Some(
                    crate::menu::rgb(to)
                        .map(crate::menu::hex)
                        .ok_or_else(|| format!("`{to}` is not a colour."))?,
                ),
            };
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
