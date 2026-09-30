//! A character's own theme and accent (`plan/57` §3c, step 3): the *Theme*
//! page among the character's settings file's pages, over its `theme`
//! section ([`Chosen`]). The theme named is worn by that character's play
//! window in place of Hydra's; the accent is pinned over whichever theme
//! the window wears. Nothing chosen, the window wears Hydra's.
//!
//! The GUI reads the section itself (`cena_gui`'s `app/looks.rs`); this is
//! its one writer, through the settings menu, as `general` is for the
//! sections beside it.

use std::path::Path;

use cena_session::settings_store::{self, SettingsFile};
use cena_ui::settings::{Page, Row, RowKind, Value};
use cena_ui::theme::{Chosen, Themes};

/// The page's id.
pub(crate) const PAGE: &str = "theme";

/// The character's settings file, trusted, or why not.
fn load(dir: &Path, instance: &str, name: &str) -> Result<SettingsFile, String> {
    settings_store::load(dir, instance, name).map_err(|why| why.to_string())
}

/// The *Theme* page for `name` on `instance`, as its file holds it; the
/// themes to choose from are the built-ins and the data folder's.
pub(crate) fn page(dir: &Path, instance: &str, name: &str) -> Page {
    let file_name = settings_store::settings_path(dir, instance, name).map_or_else(
        || format!("{instance}_{name}.settings.json"),
        |path| path.display().to_string(),
    );
    let mut page = Page {
        id: PAGE.to_owned(),
        title: "Theme".to_owned(),
        file: file_name,
        takes: "at once".to_owned(),
        problem: None,
        rows: Vec::new(),
    };
    let chosen = load(dir, instance, name).and_then(|file| {
        file.section::<Chosen>(Chosen::SECTION)
            .map_err(|why| format!("its theme section does not read: {why}"))
    });
    match chosen {
        Ok(chosen) => page.rows = rows(&chosen, &Themes::load(&dir.join("themes")).names()),
        Err(why) => page.problem = Some(format!("Nothing here is changed while {why}")),
    }
    page
}

/// The page's two rows: the theme, Hydra's own unless chosen, and the
/// accent.
fn rows(chosen: &Chosen, themes: &[String]) -> Vec<Row> {
    let mut choice = vec![(String::new(), "Hydra's".to_owned())];
    choice.extend(themes.iter().map(|t| (t.clone(), t.clone())));
    vec![
        Row {
            key: "theme".to_owned(),
            label: "Theme".to_owned(),
            help: "The theme this character's window wears; Hydra's, on the Window page, unless chosen.".to_owned(),
            kind: RowKind::Choice(choice),
            value: Value::Text(chosen.theme.clone().unwrap_or_default()),
            here: chosen.theme.is_some(),
            from: None,
        },
        Row {
            key: "accent".to_owned(),
            label: "Accent".to_owned(),
            help: "This character's own accent colour, over whichever theme its window wears: what is chosen, a guide, a find's hit.".to_owned(),
            kind: RowKind::Color,
            value: Value::Text(chosen.accent.clone().unwrap_or_default()),
            here: chosen.accent.is_some(),
            from: None,
        },
    ]
}

/// Set `key` to `to`, as the menu writes it, or back to its default
/// (`None`), and save. What was done, in words for the player.
///
/// # Errors
///
/// Why nothing was changed.
pub(crate) fn change(
    dir: &Path,
    (instance, name): (&str, &str),
    key: &str,
    to: Option<&str>,
) -> Result<String, String> {
    let path = settings_store::settings_path(dir, instance, name).unwrap_or_default();
    cena_session::store::changing(&path, || changed(dir, (instance, name), key, to))
}

/// [`change`], with the file's lock held.
fn changed(
    dir: &Path,
    (instance, name): (&str, &str),
    key: &str,
    to: Option<&str>,
) -> Result<String, String> {
    let mut file =
        load(dir, instance, name).map_err(|why| format!("Nothing was changed: {why}"))?;
    let mut chosen: Chosen = file
        .section(Chosen::SECTION)
        .map_err(|why| format!("Nothing was changed: its theme section does not read: {why}"))?;
    let done = match (key, to.map(str::trim)) {
        ("theme", Some("") | None) => {
            chosen.theme = None;
            format!("{name}'s window wears Hydra's theme")
        }
        ("theme", Some(to)) => {
            chosen.theme = Some(to.to_owned());
            format!("{name}'s window wears {to}")
        }
        ("accent", Some("") | None) => {
            chosen.accent = None;
            format!("{name} has no accent of its own")
        }
        ("accent", Some(to)) => {
            let rgb = cena_ui::theme::parse_hex(to)
                .ok_or_else(|| format!("Theme: `{to}` is not a colour."))?;
            chosen.accent = Some(cena_ui::theme::hex(rgb));
            format!("{name}'s accent is {}", cena_ui::theme::hex(rgb))
        }
        (other, _) => return Err(format!("Theme has no setting `{other}`.")),
    };
    file.set_section(Chosen::SECTION, &chosen)
        .map_err(|why| format!("Nothing was changed: {why}"))?;
    settings_store::save(dir, &file).map_err(|why| format!("Nothing was saved: {why}"))?;
    Ok(format!("Theme: {done}."))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_theme_and_an_accent_are_kept_and_put_back() {
        let dir = std::env::temp_dir().join(format!("cena-theme-page-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("made");
        let who = ("prime", "Ashryn");
        let shown = page(&dir, who.0, who.1);
        assert_eq!(shown.rows.len(), 2);
        assert!(shown.rows.iter().all(|row| !row.here));

        assert_eq!(
            change(&dir, who, "theme", Some("Light")).as_deref(),
            Ok("Theme: Ashryn's window wears Light.")
        );
        assert_eq!(
            change(&dir, who, "accent", Some("#C9733A")).as_deref(),
            Ok("Theme: Ashryn's accent is #c9733a.")
        );
        let shown = page(&dir, who.0, who.1);
        assert!(shown.rows.iter().all(|row| row.here));
        assert_eq!(shown.rows[0].value, Value::Text("Light".to_owned()));
        assert_eq!(shown.rows[1].value, Value::Text("#c9733a".to_owned()));

        assert!(change(&dir, who, "accent", Some("orange")).is_err());
        assert!(change(&dir, who, "size", Some("3")).is_err());
        change(&dir, who, "theme", None).expect("put back");
        change(&dir, who, "accent", Some("")).expect("put back");
        let chosen: Chosen = settings_store::load(&dir, who.0, who.1)
            .expect("read")
            .section(Chosen::SECTION)
            .expect("its section");
        assert_eq!(chosen, Chosen::default());
        let _ = std::fs::remove_dir_all(&dir);
    }
}
