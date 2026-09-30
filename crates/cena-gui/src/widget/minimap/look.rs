//! The minimap's own page (`plan/53` §8c step 4): its two default zooms,
//! outside and in a place, each the one a door resets to (the author:
//! *"Maybe have a setting where they can set the default zoom levels for
//! each"*), and how many maps out it draws next door (*"Maybe make it a
//! setting?"*). Kept with the layout, by the widget.

use cena_ui::settings::{Row, RowKind, Value};
use serde::{Deserialize, Serialize};

/// How a minimap draws, as the player chose on its page.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub(crate) struct MinimapLook {
    /// Pixels a cell outdoors, the zoom a door out resets to.
    pub(crate) outside: f32,
    /// Pixels a cell in a place, the zoom a door in resets to.
    pub(crate) inside: f32,
    /// How many maps out from yours are drawn next door: 0, 1 or 2.
    pub(crate) maps_out: u8,
}

impl Default for MinimapLook {
    fn default() -> Self {
        Self {
            outside: super::ZOOM,
            inside: super::ZOOM_INSIDE,
            maps_out: 1,
        }
    }
}

/// The most maps out a minimap may draw: as many rings as the binary works
/// out (`crates/cena/src/atlas/follow.rs`).
pub(crate) const MOST_MAPS_OUT: u8 = 2;

/// The page's rows, from `look` or the defaults.
pub(crate) fn rows(look: Option<&MinimapLook>) -> Vec<Row> {
    let now = look.copied().unwrap_or_default();
    let default = MinimapLook::default();
    let (least, most) = super::ZOOM_RANGE;
    let zoom = |key: &str, label: &str, help: &str, value: f32, default: f32| Row {
        key: key.to_owned(),
        label: label.to_owned(),
        help: help.to_owned(),
        kind: RowKind::Number {
            min: f64::from(least),
            max: f64::from(most),
        },
        value: Value::Text(format!("{value}")),
        here: (value - default).abs() > f32::EPSILON,
        from: None,
    };
    vec![
        zoom(
            "outside",
            "Zoom outside",
            "Pixels a cell on the streets and in the wilds: what the minimap comes back to \
             when you step outside. The wheel changes it until then.",
            now.outside,
            default.outside,
        ),
        zoom(
            "inside",
            "Zoom inside",
            "Pixels a cell in a shop, a tavern, a bank: its rooms are drawn at half the \
             streets' size, so this is larger. What the minimap goes to when you step in.",
            now.inside,
            default.inside,
        ),
        Row {
            key: "maps_out".to_owned(),
            label: "Maps next door".to_owned(),
            help: "How many maps out from yours are drawn beside it, dimmed: 0 for yours \
                   alone, 1 for those a walk joins to it, 2 for theirs too."
                .to_owned(),
            kind: RowKind::Whole {
                min: 0,
                max: u32::from(MOST_MAPS_OUT),
            },
            value: Value::Text(now.maps_out.to_string()),
            here: now.maps_out != default.maps_out,
            from: None,
        },
    ]
}

/// Set `key` to `to`, or back to its default.
pub(crate) fn set(look: &mut MinimapLook, key: &str, to: Option<&str>) -> Result<(), String> {
    let default = MinimapLook::default();
    let (least, most) = super::ZOOM_RANGE;
    let zoom = |to: Option<&str>, default: f32| -> Result<f32, String> {
        let Some(to) = to else { return Ok(default) };
        to.trim()
            .parse::<f32>()
            .ok()
            .filter(|z| (least..=most).contains(z))
            .ok_or_else(|| format!("A zoom is a number from {least} to {most}."))
    };
    match key {
        "outside" => look.outside = zoom(to, default.outside)?,
        "inside" => look.inside = zoom(to, default.inside)?,
        "maps_out" => {
            look.maps_out = match to {
                None => default.maps_out,
                Some(to) => to
                    .trim()
                    .parse::<u8>()
                    .ok()
                    .filter(|n| *n <= MOST_MAPS_OUT)
                    .ok_or_else(|| format!("Maps next door is 0 to {MOST_MAPS_OUT}."))?,
            };
        }
        _ => return Err(format!("The minimap has no setting {key}.")),
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_setting_is_checked_and_goes_back_to_its_default() {
        let mut look = MinimapLook::default();
        set(&mut look, "inside", Some("20")).expect("a zoom");
        set(&mut look, "maps_out", Some("2")).expect("two out");
        assert!((look.inside - 20.0).abs() < f32::EPSILON);
        assert_eq!(look.maps_out, 2);
        assert!(
            set(&mut look, "outside", Some("99")).is_err(),
            "past the most"
        );
        assert!(set(&mut look, "maps_out", Some("3")).is_err());
        set(&mut look, "inside", None).expect("back");
        assert_eq!(
            look,
            MinimapLook {
                maps_out: 2,
                ..MinimapLook::default()
            }
        );
        let rows = rows(Some(&look));
        assert!(rows.iter().any(|r| r.key == "maps_out" && r.here));
        assert!(rows.iter().all(|r| r.key == "maps_out" || !r.here));
    }
}
