//! Hydra's own settings (`plan/50` §7 step 2), one set for every character
//! (§4.4's Hydra scope), kept in the window's own file, `window.toml` in the
//! data folder. The settings menu shows them as its *Window* page, drawn as
//! any other page; no behavior is behind them, so the window applies a
//! change itself.
//!
//! - **The card width** (`plan/49` Stage C's revision): every hub card's,
//!   kept when a drag of a card's side lets go, or typed on the page.
//! - **Closing a play window when its session closes** (§6 item 11): off,
//!   so the window stays open with the character's last state.
//! - **The key held to drag an object** from a link (`carry.rs`): Alt, or
//!   Ctrl or Shift, as `VellumFE`'s `drag_modifier_key` is. Alt by default
//!   since 2026-09-29 (the author), so Ctrl is left to copying.

use std::path::{Path, PathBuf};

use cena_ui::settings::{Page, Row, RowKind, Value};
use egui::Modifiers;

use crate::hub::CardWidth;

/// The window's own file, in the data folder.
pub(crate) const FILE: &str = "window.toml";
/// The page's id, which a change names.
pub(crate) const PAGE: &str = "window";
/// The widest a card is typed to.
const WIDEST: f32 = 2000.0;

/// The file as written: a key it leaves out is at its default.
#[derive(Clone, Debug, Default, PartialEq, serde::Deserialize, serde::Serialize)]
#[serde(deny_unknown_fields)]
struct File {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    card_width: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    close_with_session: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    drag_with: Option<String>,
}

/// The keys an object may be dragged with: each as the file writes it, as
/// the page names it, and as egui holds it.
const DRAG_KEYS: [(&str, &str, Modifiers); 3] = [
    ("ctrl", "Ctrl", Modifiers::CTRL),
    ("alt", "Alt", Modifiers::ALT),
    ("shift", "Shift", Modifiers::SHIFT),
];

/// Hydra's own settings, and where they are kept.
#[derive(Debug, Default)]
pub(crate) struct Own {
    file: File,
    /// The data folder; `None`, and nothing is kept.
    data: Option<PathBuf>,
    /// Why the file does not read: nothing is written over it.
    problem: Option<String>,
}

impl Own {
    /// Read from `data`, the data folder; no file there is every default.
    pub(crate) fn load(data: &Path) -> Self {
        let (file, problem) = match std::fs::read_to_string(data.join(FILE)) {
            Ok(text) => match toml::from_str::<File>(&text) {
                Ok(file) => (file, None),
                Err(why) => (File::default(), Some(format!("it does not read: {why}"))),
            },
            Err(why) if why.kind() == std::io::ErrorKind::NotFound => (File::default(), None),
            Err(why) => (File::default(), Some(format!("it cannot be read: {why}"))),
        };
        Self {
            file,
            data: Some(data.to_owned()),
            problem,
        }
    }

    /// Every hub card's width.
    pub(crate) fn card_width(&self) -> CardWidth {
        self.file.card_width.map_or(CardWidth::FOUR_BARS, |width| {
            CardWidth(width.clamp(CardWidth::NARROWEST, WIDEST))
        })
    }

    /// The key held to drag an object from a link: Alt unless chosen.
    pub(crate) fn drag_with(&self) -> Modifiers {
        let chosen = self.file.drag_with.as_deref().unwrap_or("alt");
        DRAG_KEYS
            .iter()
            .find(|(written, ..)| *written == chosen)
            .map_or(Modifiers::ALT, |(.., key)| *key)
    }

    /// Whether a play window closes when its session does.
    pub(crate) fn close_with_session(&self) -> bool {
        self.file.close_with_session.unwrap_or(false)
    }

    /// Keep `width`, which a drag set, when it is not what is kept already.
    ///
    /// # Errors
    ///
    /// Why it was not saved.
    pub(crate) fn keep_width(&mut self, width: CardWidth) -> Result<(), String> {
        if self.problem.is_some() || (width.0 - self.card_width().0).abs() < 0.5 {
            return Ok(());
        }
        let file = File {
            card_width: Some(width.0.round()),
            ..self.file.clone()
        };
        self.save(file)
    }

    /// The *Window* page, as the menu draws it.
    pub(crate) fn page(&self) -> Page {
        let file = self.data.as_ref().map_or_else(
            || FILE.to_owned(),
            |data| data.join(FILE).display().to_string(),
        );
        let mut page = Page {
            id: PAGE.to_owned(),
            title: "Window".to_owned(),
            file,
            takes: "at once".to_owned(),
            problem: None,
            rows: Vec::new(),
        };
        if let Some(why) = &self.problem {
            page.problem = Some(format!("Nothing here is changed while {why}"));
            return page;
        }
        page.rows = vec![
            Row {
                key: "card_width".to_owned(),
                label: "Card width".to_owned(),
                help: "How wide every card on the hub is. Dragging a card's side sets it too."
                    .to_owned(),
                kind: RowKind::Number {
                    min: f64::from(CardWidth::NARROWEST),
                    max: f64::from(WIDEST),
                },
                value: Value::Text(format!("{:.0}", self.card_width().0)),
                here: self.file.card_width.is_some(),
                from: None,
            },
            Row {
                key: "close_with_session".to_owned(),
                label: "Close a play window when its session closes".to_owned(),
                help: "Off, the window stays open with the character's last state.".to_owned(),
                kind: RowKind::Toggle,
                value: Value::On(self.close_with_session()),
                here: self.file.close_with_session.is_some(),
                from: None,
            },
            Row {
                key: "drag_with".to_owned(),
                label: "Drag an item with".to_owned(),
                help: "The key held while dragging an object's link to a hand, a container or the floor."
                    .to_owned(),
                kind: RowKind::Choice(
                    DRAG_KEYS
                        .iter()
                        .map(|(written, called, _)| ((*written).to_owned(), (*called).to_owned()))
                        .collect(),
                ),
                value: Value::Text(self.file.drag_with.clone().unwrap_or_else(|| "alt".to_owned())),
                here: self.file.drag_with.is_some(),
                from: None,
            },
        ];
        page
    }

    /// Set `key` to `to`, as the menu writes it, or back to its default
    /// (`None`), and save. What was done, in words for the player.
    ///
    /// # Errors
    ///
    /// Why nothing was changed.
    pub(crate) fn change(&mut self, key: &str, to: Option<&str>) -> Result<String, String> {
        if let Some(why) = &self.problem {
            return Err(format!("Window: nothing was changed: {FILE}: {why}"));
        }
        let mut file = self.file.clone();
        let done = match (key, to) {
            ("card_width", Some(to)) => {
                let width = to
                    .trim()
                    .parse::<f32>()
                    .ok()
                    .filter(|width| (CardWidth::NARROWEST..=WIDEST).contains(width))
                    .ok_or_else(|| {
                        format!(
                            "Window: the card width is a number from {} to {WIDEST}.",
                            CardWidth::NARROWEST
                        )
                    })?;
                file.card_width = Some(width.round());
                format!("the card width is {:.0}", width.round())
            }
            ("close_with_session", Some(to)) => {
                let on = match to.trim() {
                    "on" => true,
                    "off" => false,
                    other => return Err(format!("Window: `{other}` is not on or off.")),
                };
                file.close_with_session = Some(on);
                format!(
                    "a play window {} when its session closes",
                    if on { "closes" } else { "stays open" }
                )
            }
            ("drag_with", Some(to)) => {
                let (written, called, _) = DRAG_KEYS
                    .iter()
                    .find(|(written, ..)| *written == to.trim())
                    .ok_or_else(|| format!("Window: an item is not dragged with `{to}`."))?;
                file.drag_with = Some((*written).to_owned());
                format!("an item is dragged with {called}")
            }
            ("drag_with", None) => {
                file.drag_with = None;
                "an item is dragged with Alt, its default".to_owned()
            }
            ("card_width", None) => {
                file.card_width = None;
                "the card width is back to its default".to_owned()
            }
            ("close_with_session", None) => {
                file.close_with_session = None;
                "closing a play window is back to its default, off".to_owned()
            }
            (other, _) => return Err(format!("Window has no setting `{other}`.")),
        };
        self.save(file)?;
        Ok(format!("Window: {done}."))
    }

    /// Write `file`, and hold it once written. With no data folder, it is
    /// held and not written.
    fn save(&mut self, file: File) -> Result<(), String> {
        if let Some(data) = &self.data {
            let text = toml::to_string(&file).map_err(|why| format!("{FILE}: {why}"))?;
            cena_session::store::save_text(data, &data.join(FILE), &text)
                .map_err(|why| format!("{FILE} was not saved: {why}"))?;
        }
        self.file = file;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn folder(name: &str) -> PathBuf {
        let data = std::env::temp_dir().join(format!("cena-own-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&data);
        data
    }

    /// With no file, every setting is at its default; a change is saved and
    /// read back, and *Use default* takes it out of the file again.
    #[test]
    fn a_change_is_kept_and_put_back() {
        let data = folder("change");
        let mut own = Own::load(&data);
        assert_eq!(own.card_width(), CardWidth::FOUR_BARS);
        assert!(!own.close_with_session(), "off by default");
        assert_eq!(own.drag_with(), Modifiers::ALT, "Alt by default");
        assert!(own.page().rows.iter().all(|row| !row.here));

        assert_eq!(
            own.change("close_with_session", Some("on")).as_deref(),
            Ok("Window: a play window closes when its session closes.")
        );
        own.change("card_width", Some("420.4")).expect("changed");
        own.change("drag_with", Some("alt")).expect("changed");
        let read = Own::load(&data);
        assert!(read.close_with_session());
        assert_eq!(read.drag_with(), Modifiers::ALT);
        assert_eq!(read.card_width(), CardWidth(420.0));
        assert!(read.page().rows.iter().all(|row| row.here));

        own.change("card_width", None).expect("put back");
        assert_eq!(Own::load(&data).card_width(), CardWidth::FOUR_BARS);
        assert!(own.change("card_width", Some("90")).is_err(), "too narrow");
        assert!(own.change("close_with_session", Some("yes")).is_err());
        assert!(own.change("drag_with", Some("meta")).is_err());
        assert!(own.change("volume", Some("3")).is_err());
        let _ = std::fs::remove_dir_all(&data);
    }

    /// A width a drag set is kept; the same width again writes nothing.
    #[test]
    fn a_dragged_width_is_kept() {
        let data = folder("drag");
        let mut own = Own::load(&data);
        own.keep_width(CardWidth::FOUR_BARS)
            .expect("nothing to keep");
        assert!(!data.join(FILE).exists(), "the default is not written");
        own.keep_width(CardWidth(351.3)).expect("kept");
        assert_eq!(Own::load(&data).card_width(), CardWidth(351.0));
        let _ = std::fs::remove_dir_all(&data);
    }

    /// A file that does not read shows why, has no rows, and is never
    /// written over, by a change or by a drag.
    #[test]
    fn a_broken_file_is_shown_and_left_alone() {
        let data = folder("broken");
        std::fs::create_dir_all(&data).expect("made");
        std::fs::write(data.join(FILE), "card_width = \"wide\"\n").expect("written");
        let mut own = Own::load(&data);
        let page = own.page();
        assert!(page.rows.is_empty());
        assert!(
            page.problem.as_deref().is_some_and(
                |problem| problem.starts_with("Nothing here is changed while it does not read")
            ),
            "{page:?}"
        );
        assert!(own.change("card_width", Some("400")).is_err());
        own.keep_width(CardWidth(500.0)).expect("quietly not kept");
        assert_eq!(
            std::fs::read_to_string(data.join(FILE)).ok().as_deref(),
            Some("card_width = \"wide\"\n")
        );
        let _ = std::fs::remove_dir_all(&data);
    }
}
