//! The theme editor's window (`plan/57` step 6): opened by *Theme* beside
//! *Settings* and *Triggers*; a theme it saves is written to the `themes`
//! folder and the themes read again; the draft worn while editing is worn
//! on the whole window until the editor closes.

use super::{App, TITLE};
use crate::theme_editor::Asked;

/// The editor's key among the windows' places.
const THEME: &str = "theme";

/// What `worn` holds while the theme editor's draft is worn: no theme's name.
pub(super) const PREVIEW: &str = "\u{1}preview";

impl App {
    /// Wear the theme Hydra's own settings choose, once, and again after a
    /// change: the one chosen, or the light one while the computer is in
    /// light mode and the setting follows it (`plan/57` step 2). A theme
    /// that cannot be worn is said on the hub, and Despana is worn.
    pub(super) fn wear_theme(&mut self, context: &egui::Context) {
        if self.fonts.is_none() {
            let data = self.keys_file.as_deref().and_then(std::path::Path::parent);
            self.fonts = Some(data.map_or_else(Vec::new, |data| {
                crate::fonts::load(context, &data.join("fonts")).1
            }));
        }
        // The editor's draft, while it is worn: over whatever is chosen.
        if let Some(outfit) = &self.preview {
            if self.worn.as_deref() != Some(PREVIEW) {
                crate::theme::wear(context, outfit);
                self.worn = Some(PREVIEW.to_owned());
                self.looks.wear_again();
            }
            return;
        }
        let light =
            self.own.follow_computer() && context.system_theme() == Some(egui::Theme::Light);
        let wanted = if light {
            self.own.light_theme()
        } else {
            self.own.theme()
        };
        if self.worn.as_deref() == Some(wanted) {
            return;
        }
        let outfit = match self.themes.outfit(wanted) {
            Ok(outfit) => {
                self.theme_problem = None;
                outfit
            }
            Err(why) => {
                self.theme_problem = Some(format!("{wanted} is not worn: {why}"));
                self.themes
                    .outfit(cena_ui::theme::Theme::DEFAULT)
                    .unwrap_or_default()
            }
        };
        crate::theme::wear(context, &outfit);
        self.worn = Some(wanted.to_owned());
        self.looks.wear_again();
    }

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
