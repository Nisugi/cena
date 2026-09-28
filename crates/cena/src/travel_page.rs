//! The settings menu's *Travel* page (`plan/50` §7 step 4): one character's
//! travel settings, in its spot in the travel file
//! ([`cena_session::travel_store`]), until now a hand edit only.
//!
//! First the settings travel's code reads, each with its kind
//! ([`cena_behavior::travel::settings::TABLE`]). Then the map's own, which its
//! crossings read (`ice_mode`, the sack a house key is kept in), and any other
//! the file holds, as words: the map, not Hydra, says what those mean. The
//! author, asked whether travel's settings should be *"a typed list of the
//! keys the code reads, with the map file's own keys kept as a free list"*:
//! *"checked"* (§6 item 2).
//!
//! A change is written one setting at a time ([`travel_store::set_setting`]),
//! so a trip under way cannot put the old value back when it ends. The next
//! trip reads it.

use std::collections::BTreeSet;
use std::path::Path;

use cena_behavior::settings::{self, KeyKind};
use cena_behavior::travel::settings::TABLE;
use cena_session::travel_store;
use cena_ui::settings::{Page, Row, RowKind, Value};

/// The page's id.
pub(crate) const PAGE: &str = "travel";

/// The *Travel* page for `name` on `instance`, as the travel file holds it,
/// with `map`'s own settings after travel's.
pub(crate) fn page(dir: &Path, instance: &str, name: &str, map: &BTreeSet<String>) -> Page {
    let mut page = Page {
        id: PAGE.to_owned(),
        title: "Travel".to_owned(),
        file: travel_store::travel_path(dir)
            .strip_prefix(dir)
            .map_or_else(
                |_| "travel.json".to_owned(),
                |path| path.display().to_string(),
            ),
        takes: "at the next trip".to_owned(),
        problem: None,
        rows: Vec::new(),
    };
    let file = match travel_store::load(dir, instance, name) {
        Ok(file) => file,
        Err(why) => {
            page.problem = Some(format!("Nothing here is changed while {why}"));
            return page;
        }
    };
    let held = |key: &str| file.settings.get(key);
    let typed = TABLE.iter().map(|key| {
        let (kind, value) = match key.kind {
            KeyKind::Toggle => (
                RowKind::Toggle,
                Value::On(held(key.name).is_some_and(|is| is == "true")),
            ),
            KeyKind::Whole { min, max } => (RowKind::Whole { min, max }, text(held(key.name))),
            _ => (RowKind::Text, text(held(key.name))),
        };
        Row {
            key: key.name.to_owned(),
            label: key.label.to_owned(),
            help: key.help.to_owned(),
            kind,
            value,
            here: held(key.name).is_some(),
        }
    });
    let free = map
        .iter()
        .chain(file.settings.keys())
        .filter(|key| TABLE.iter().all(|typed| typed.name != key.as_str()))
        .collect::<BTreeSet<_>>()
        .into_iter()
        .map(|key| Row {
            key: key.clone(),
            label: key.clone(),
            help: if map.contains(key) {
                "A setting the map's crossings read, in the map's own words.".to_owned()
            } else {
                "Kept in the travel file; the map this Hydra has loaded reads no such setting."
                    .to_owned()
            },
            kind: RowKind::Text,
            value: text(held(key)),
            here: held(key).is_some(),
        });
    page.rows = typed.chain(free).collect();
    page
}

/// A value kept as text, or none.
fn text(held: Option<&String>) -> Value {
    held.map_or(Value::Unset, |text| Value::Text(text.clone()))
}

/// Set `key` of `name`'s travel settings to `to`, as the menu writes it, or
/// back to its default (`None`). `map`'s own settings may be set as words,
/// as may any the file already holds. What was done.
///
/// # Errors
///
/// Why nothing was changed.
pub(crate) fn change(
    dir: &Path,
    (instance, name): (&str, &str),
    key: &str,
    to: Option<&str>,
    map: &BTreeSet<String>,
) -> Result<String, String> {
    let file = travel_store::load(dir, instance, name)
        .map_err(|why| format!("Travel: nothing was changed: {why}"))?;
    let kind = TABLE
        .iter()
        .find(|typed| typed.name == key)
        .map(|typed| typed.kind);
    if kind.is_none() && !map.contains(key) && !file.settings.contains_key(key) {
        return Err(format!("Travel has no setting {key}."));
    }
    let value = match to {
        None => None,
        Some(to) => {
            let value = settings::typed(to);
            Some(match kind {
                Some(KeyKind::Toggle) => value
                    .as_bool()
                    .ok_or_else(|| format!("Travel: {key} is on or off."))?
                    .to_string(),
                Some(KeyKind::Whole { min, max }) => value
                    .as_integer()
                    .and_then(|whole| u32::try_from(whole).ok())
                    .filter(|whole| (min..=max).contains(whole))
                    .ok_or_else(|| format!("Travel: {key} is a whole number from {min} to {max}."))?
                    .to_string(),
                _ => value
                    .as_str()
                    .map_or_else(|| value.to_string(), str::to_owned),
            })
        }
    };
    travel_store::set_setting(dir, instance, name, key, value.as_deref())
        .map_err(|why| format!("Travel: not saved: {why}"))?;
    Ok(match value {
        Some(value) => format!("Travel: {key} is {value}, from the next trip."),
        None => format!("Travel: {key} is back to its default."),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use cena_behavior::travel::settings::{GET_SILVERS, GIGAS_MIN_NUMBER, USE_URCHINS};

    fn scratch(test: &str) -> std::path::PathBuf {
        let dir =
            std::env::temp_dir().join(format!("cena-travel-page-{test}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        dir
    }

    fn map() -> BTreeSet<String> {
        ["ice_mode", "key_sack", USE_URCHINS]
            .into_iter()
            .map(str::to_owned)
            .collect()
    }

    fn row<'a>(page: &'a Page, key: &str) -> Option<&'a Row> {
        page.rows.iter().find(|row| row.key == key)
    }

    /// Travel's own settings come first, each with its kind; then the map's
    /// own and the file's others, as words, each once.
    #[test]
    fn the_codes_settings_are_typed_and_the_maps_are_words() {
        let dir = scratch("rows");
        travel_store::set_setting(&dir, "Prime", "Nisugi", "old_thing", Some("x")).expect("set");
        let page = super::page(&dir, "Prime", "Nisugi", &map());
        let keys: Vec<&str> = page.rows.iter().map(|row| row.key.as_str()).collect();
        assert_eq!(keys.len(), TABLE.len() + 3, "{keys:?}");
        assert_eq!(keys[..TABLE.len()], settings::names(TABLE)[..]);
        assert_eq!(keys[TABLE.len()..], ["ice_mode", "key_sack", "old_thing"]);
        assert_eq!(
            row(&page, USE_URCHINS).map(|row| &row.kind),
            Some(&RowKind::Toggle)
        );
        assert_eq!(
            row(&page, GIGAS_MIN_NUMBER).map(|row| &row.kind),
            Some(&RowKind::Whole { min: 0, max: 1000 })
        );
        assert!(row(&page, "ice_mode").is_some_and(|row| row.kind == RowKind::Text && !row.here));
        assert!(row(&page, "old_thing").is_some_and(|row| row.here));
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// A change is written as travel reads it -- a switch as `true` -- and
    /// the next trip finds it; `None` takes it out.
    #[test]
    fn a_change_is_written_as_travel_reads_it() {
        let dir = scratch("change");
        let who = ("Prime", "Nisugi");
        assert_eq!(
            change(&dir, who, GET_SILVERS, Some("on"), &map()).as_deref(),
            Ok("Travel: get_silvers is true, from the next trip.")
        );
        change(&dir, who, GIGAS_MIN_NUMBER, Some("6"), &map()).expect("a number");
        change(&dir, who, "ice_mode", Some("\"wait\""), &map()).expect("the map's");
        let file = travel_store::load(&dir, "Prime", "Nisugi").expect("it reads");
        let setting = |key: &str| file.settings.get(key).map(String::as_str);
        assert_eq!(setting(GET_SILVERS), Some("true"));
        assert_eq!(setting(GIGAS_MIN_NUMBER), Some("6"));
        assert_eq!(setting("ice_mode"), Some("wait"));
        let page = super::page(&dir, "Prime", "Nisugi", &map());
        assert_eq!(
            row(&page, GET_SILVERS).map(|row| &row.value),
            Some(&Value::On(true))
        );
        assert!(row(&page, GET_SILVERS).is_some_and(|row| row.here));
        assert!(row(&page, USE_URCHINS).is_some_and(|row| !row.here));

        change(&dir, who, GET_SILVERS, None, &map()).expect("put back");
        let file = travel_store::load(&dir, "Prime", "Nisugi").expect("it reads");
        assert!(!file.settings.contains_key(GET_SILVERS));
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// What is not of a setting's kind is refused, as is a setting nothing
    /// reads; a travel file that cannot be trusted is shown with why and
    /// never written over.
    #[test]
    fn a_bad_value_or_a_broken_file_changes_nothing() {
        let dir = scratch("broken");
        let who = ("Prime", "Nisugi");
        assert!(change(&dir, who, USE_URCHINS, Some("7"), &map()).is_err());
        assert!(change(&dir, who, GIGAS_MIN_NUMBER, Some("-1"), &map()).is_err());
        assert!(change(&dir, who, GIGAS_MIN_NUMBER, Some("1001"), &map()).is_err());
        assert!(change(&dir, who, GIGAS_MIN_NUMBER, Some("\"four\""), &map()).is_err());
        assert!(change(&dir, who, "made_up", Some("\"x\""), &map()).is_err());
        std::fs::create_dir_all(&dir).expect("made");
        let path = travel_store::travel_path(&dir);
        std::fs::write(&path, "{ not json").expect("written");
        let page = super::page(&dir, "Prime", "Nisugi", &map());
        assert!(page.problem.is_some() && page.rows.is_empty());
        assert!(change(&dir, who, GET_SILVERS, Some("on"), &map()).is_err());
        assert_eq!(
            std::fs::read_to_string(&path).ok().as_deref(),
            Some("{ not json")
        );
        let _ = std::fs::remove_dir_all(&dir);
    }
}
