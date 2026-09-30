//! The theme editor's window (`plan/57` step 6): opened by *Theme* beside
//! *Settings* and *Triggers*; a theme it saves is written to the `themes`
//! folder and the themes read again; the draft worn while editing is worn
//! on the whole window until the editor closes.

use super::{App, TITLE};
use crate::theme_editor::Asked;

/// The editor's key among the windows' places.
const THEME: &str = "theme";

impl App {
    /// Open the theme editor, starting its draft from the theme worn.
    pub(super) fn open_theme_editor(&mut self) {
        if self.themer.open() {
            let worn = self
                .worn
                .clone()
                .unwrap_or_else(|| self.own.theme().to_owned());
            self.themer.start_from(&self.themes, &worn);
        }
    }

    /// The theme editor, in a window of its own, while it is open.
    pub(super) fn theme_window(&mut self, context: &egui::Context) {
        if !self.themer.open {
            if self.preview.take().is_some() {
                self.worn = None;
            }
            return;
        }
        let fonts = crate::fonts::loaded(context);
        let editor = &mut self.themer;
        let themes = &self.themes;
        let mut asked = Vec::new();
        let closed = self.placements.show(
            context,
            THEME,
            egui::ViewportId::from_hash_of(THEME),
            ([1000.0, 700.0], format!("Theme — {TITLE}")),
            |ui| asked.extend(editor.show(ui, themes, &fonts)),
        );
        if closed {
            self.themer.open = false;
            asked.push(Asked::Preview(None));
        }
        for ask in asked {
            match ask {
                Asked::Save(theme) => {
                    let said = self.save_theme(&theme);
                    self.themer.said(said);
                }
                Asked::Preview(outfit) => {
                    self.preview = outfit;
                    self.worn = None;
                }
            }
        }
    }

    /// Write `theme` to the themes folder and read the themes again; what
    /// was done, for the editor.
    fn save_theme(&mut self, theme: &cena_ui::theme::Theme) -> String {
        let Some(data) = self
            .keys_file
            .as_deref()
            .and_then(std::path::Path::parent)
            .map(std::path::Path::to_owned)
        else {
            return "Nothing is kept: Hydra was started with no data folder.".to_owned();
        };
        let folder = data.join("themes");
        let stem: String = theme
            .name
            .chars()
            .map(|c| {
                if c.is_alphanumeric() {
                    c.to_ascii_lowercase()
                } else {
                    '-'
                }
            })
            .collect();
        let path = folder.join(format!("{stem}.toml"));
        if let Err(why) = std::fs::create_dir_all(&folder) {
            return format!("The themes folder cannot be made: {why}.");
        }
        match cena_session::store::save_text(&folder, &path, &theme.to_toml()) {
            Ok(()) => {
                self.themes = cena_ui::theme::Themes::load(&folder);
                self.looks.wear_again();
                self.worn = None;
                format!("{} written. Choose it on the Window page.", path.display())
            }
            Err(why) => format!("{} was not written: {why}.", path.display()),
        }
    }
}
