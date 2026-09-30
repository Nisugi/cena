//! The theme editor (`plan/57` step 6): a window of its own beside
//! *Settings* and *Triggers*, where a theme is made. Generate first: a seed
//! and a background, a scheme, the dials, and the whole palette shown as it
//! comes out with a sample of story text; then each token with its pin;
//! then the shape and the type. *Wear while editing* puts the draft on the
//! whole window as it changes. *Save* writes the theme to the `themes`
//! folder, holding only what differs from its base; a built-in is never
//! written over.
//!
//! The editor holds the draft whole, resolved: the recipe with its pins,
//! the shape and the type, over the base it names. What is saved is the
//! difference ([`Editor::draft`]), so a file says only what it changes.

use std::collections::BTreeSet;

use cena_ui::theme::{Outfit, Recipe, RecipeFile, Shape, ShapeFile, Theme, Themes, Type, TypeFile};

mod generate;
mod look;
mod pins;
mod preview;
#[cfg(test)]
mod tests;

/// What the editor asks of the app.
#[derive(Clone, Debug, PartialEq)]
pub(crate) enum Asked {
    /// Write this theme to the themes folder.
    Save(Theme),
    /// Wear this outfit on the whole window while editing; `None`, Hydra's
    /// own again.
    Preview(Option<Outfit>),
}

/// The theme editor.
#[derive(Debug)]
pub(crate) struct Editor {
    /// Whether it is showing.
    pub(crate) open: bool,
    /// The theme the draft began as, for the *Start from* choice.
    from: String,
    /// The draft's name.
    name: String,
    /// The theme the draft takes what it does not say from.
    base: Option<String>,
    /// The draft's recipe, whole, its own pins in it.
    recipe: Recipe,
    /// The draft's shape, whole.
    shape: Shape,
    /// The draft's type, whole.
    kind: Type,
    /// Whether the draft is worn on the whole window as it changes.
    wearing: bool,
    /// What the editor has to say: why a save did nothing, or that it did.
    notice: Option<String>,
    /// The outfit last sent to be worn, so it is sent again only when the
    /// draft changed.
    sent: Option<Outfit>,
}

impl Default for Editor {
    /// A draft over Despana, named for it.
    fn default() -> Self {
        let mut editor = Self {
            open: false,
            from: Theme::DEFAULT.to_owned(),
            name: String::new(),
            base: None,
            recipe: Recipe::default(),
            shape: Shape::default(),
            kind: Type::default(),
            wearing: false,
            notice: None,
            sent: None,
        };
        editor.start_from(&Themes::built_in(), Theme::DEFAULT);
        editor
    }
}

impl Editor {
    /// Open it; whether it was closed before.
    pub(crate) fn open(&mut self) -> bool {
        let was_closed = !self.open;
        self.open = true;
        was_closed
    }

    /// Begin a draft from the theme named `name`: a built-in becomes the
    /// base of a new theme named after it; a file's theme is edited as it
    /// is, over its own base.
    pub(crate) fn start_from(&mut self, themes: &Themes, name: &str) {
        let Some(theme) = themes.get(name) else {
            return;
        };
        let built_in = Themes::built_in().get(name).is_some();
        let (base, own_name) = if built_in {
            (Some(theme.name.clone()), format!("My {}", theme.name))
        } else {
            (theme.base.clone(), theme.name.clone())
        };
        let Ok((mut recipe, shape, kind)) = themes.resolve(name) else {
            return;
        };
        if built_in {
            // Its pins are the base's; the draft starts with none of its own.
            recipe.pins.clear();
        } else {
            recipe.pins = theme.pins.clone();
        }
        self.from.clone_from(&theme.name);
        self.name = own_name;
        self.base = base;
        self.recipe = recipe;
        self.shape = shape;
        self.kind = kind;
        self.notice = None;
        self.sent = None;
    }

    /// What is said under the top bar.
    pub(crate) fn said(&mut self, notice: String) {
        self.notice = Some(notice);
    }

    /// The draft as a theme: over its base, saying only what differs.
    pub(crate) fn draft(&self, themes: &Themes) -> Theme {
        let (base_recipe, base_shape, base_kind) = self
            .base
            .as_deref()
            .and_then(|base| themes.resolve(base).ok())
            .unwrap_or_default();
        Theme {
            name: self.name.trim().to_owned(),
            base: self.base.clone(),
            recipe: RecipeFile::differing(&self.recipe, &base_recipe),
            pins: self.recipe.pins.clone(),
            shape: ShapeFile::differing(&self.shape, &base_shape),
            kind: TypeFile::differing(&self.kind, &base_kind),
        }
    }

    /// The draft worn: its palette, shape and type, over its base.
    ///
    /// # Errors
    ///
    /// The base cannot be resolved.
    pub(crate) fn outfit(&self, themes: &Themes) -> Result<Outfit, String> {
        let mut themes = themes.clone();
        let mut draft = self.draft(themes_ref(&themes));
        if draft.name.is_empty() {
            "\u{1}draft".clone_into(&mut draft.name);
        }
        let name = draft.name.clone();
        themes.add(draft);
        themes.outfit(&name)
    }

    /// Draw the editor into `ui`; `fonts` are the families loaded, for the
    /// type's choices. What it asks of the app.
    pub(crate) fn show(
        &mut self,
        ui: &mut egui::Ui,
        themes: &Themes,
        fonts: &BTreeSet<String>,
    ) -> Vec<Asked> {
        let mut asked = Vec::new();
        let names = themes.names();
        ui.horizontal(|ui| {
            ui.label("Start from");
            let mut from = self.from.clone();
            egui::ComboBox::from_id_salt("theme-from")
                .selected_text(&from)
                .show_ui(ui, |ui| {
                    for name in &names {
                        ui.selectable_value(&mut from, name.clone(), name);
                    }
                });
            if from != self.from {
                self.start_from(themes, &from);
            }
            ui.separator();
            ui.label("Name");
            ui.add(egui::TextEdit::singleline(&mut self.name).desired_width(160.0));
            ui.label("over");
            let shown = self.base.clone().unwrap_or_else(|| "nothing".to_owned());
            egui::ComboBox::from_id_salt("theme-base")
                .selected_text(shown)
                .show_ui(ui, |ui| {
                    ui.selectable_value(&mut self.base, None, "nothing");
                    for name in names.iter().filter(|n| !n.eq_ignore_ascii_case(&self.name)) {
                        ui.selectable_value(&mut self.base, Some(name.clone()), name);
                    }
                });
            ui.separator();
            if ui
                .checkbox(&mut self.wearing, "Wear while editing")
                .on_hover_text("The whole window in the draft's colours as it changes")
                .changed()
                && !self.wearing
            {
                self.sent = None;
                asked.push(Asked::Preview(None));
            }
            if ui.button("Save").clicked() {
                match self.savable(themes) {
                    Ok(theme) => asked.push(Asked::Save(theme)),
                    Err(why) => self.notice = Some(why),
                }
            }
        });
        if let Some(notice) = &self.notice {
            ui.label(notice.as_str());
        }
        ui.separator();
        let outfit = self.outfit(themes);
        let height = (ui.available_height() - 8.0).max(200.0);
        ui.horizontal_top(|ui| {
            ui.vertical(|ui| {
                ui.set_width(430.0);
                egui::ScrollArea::vertical()
                    .id_salt("theme-controls")
                    .max_height(height)
                    .auto_shrink(false)
                    .show(ui, |ui| {
                        egui::CollapsingHeader::new("Generate")
                            .default_open(true)
                            .show(ui, |ui| {
                                generate::show(ui, &mut self.recipe, themes, &self.from);
                            });
                        if let Ok(outfit) = &outfit {
                            egui::CollapsingHeader::new("Pins")
                                .show(ui, |ui| pins::show(ui, &mut self.recipe, &outfit.palette));
                        }
                        egui::CollapsingHeader::new("Shape")
                            .show(ui, |ui| look::shape(ui, &mut self.shape));
                        egui::CollapsingHeader::new("Type")
                            .show(ui, |ui| look::kind(ui, &mut self.kind, fonts));
                    });
            });
            ui.separator();
            ui.vertical(|ui| {
                egui::ScrollArea::vertical()
                    .id_salt("theme-preview")
                    .max_height(height)
                    .auto_shrink(false)
                    .show(ui, |ui| match &outfit {
                        Ok(outfit) => preview::show(ui, outfit, fonts),
                        Err(why) => {
                            ui.colored_label(
                                ui.visuals().warn_fg_color,
                                format!("Not worn: {why}"),
                            );
                        }
                    });
            });
        });
        if self.wearing
            && let Ok(outfit) = outfit
            && self.sent.as_ref() != Some(&outfit)
        {
            self.sent = Some(outfit.clone());
            asked.push(Asked::Preview(Some(outfit)));
        }
        asked
    }

    /// The draft as it would be saved, or why it cannot be: no name, a
    /// built-in's name, or a base that goes round.
    fn savable(&self, themes: &Themes) -> Result<Theme, String> {
        let name = self.name.trim();
        if name.is_empty() {
            return Err("Give the theme a name first.".to_owned());
        }
        if Themes::built_in().get(name).is_some() {
            return Err(format!("{name} is built in; save it under another name."));
        }
        if self
            .base
            .as_deref()
            .is_some_and(|base| base.eq_ignore_ascii_case(name))
        {
            return Err("A theme cannot be its own base.".to_owned());
        }
        self.outfit(themes)?;
        Ok(self.draft(themes))
    }
}

/// `themes` as a reference: for the draft, which resolves its base in it.
fn themes_ref(themes: &Themes) -> &Themes {
    themes
}
