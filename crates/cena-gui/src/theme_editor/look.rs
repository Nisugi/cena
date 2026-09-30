//! The editor's *Shape* and *Type*: the corners, edges, density, scrollbar
//! and shadows; the fonts, from those loaded, and their sizes.

use std::collections::BTreeSet;

use cena_ui::theme::{Density, Shape, Type};

/// Draw the shape's controls into `ui`.
pub(super) fn shape(ui: &mut egui::Ui, shape: &mut Shape) {
    ui.add(egui::Slider::new(&mut shape.corner, 0..=32).text("Corners"))
        .on_hover_text("A control's corner radius; a window's is twice it");
    ui.add(egui::Slider::new(&mut shape.stroke, 0.0..=8.0).text("Edges"))
        .on_hover_text("An edge's width");
    ui.horizontal(|ui| {
        ui.label("Density");
        for density in Density::ALL {
            ui.selectable_value(&mut shape.density, density, density.name());
        }
    });
    ui.add(egui::Slider::new(&mut shape.scrollbar, 2.0..=32.0).text("Scrollbar"));
    ui.checkbox(&mut shape.shadows, "Windows and menus cast shadows");
}

/// Draw the type's controls into `ui`; `fonts` are the families loaded
/// from the fonts folder.
pub(super) fn kind(ui: &mut egui::Ui, kind: &mut Type, fonts: &BTreeSet<String>) {
    font(ui, "UI font", &mut kind.ui_font, fonts, "egui's own");
    font(ui, "Story font", &mut kind.story_font, fonts, "the UI's");
    ui.add(egui::Slider::new(&mut kind.ui_size, 6.0..=48.0).text("UI size"));
    ui.add(egui::Slider::new(&mut kind.story_size, 6.0..=48.0).text("Story size"));
    if fonts.is_empty() {
        ui.weak("Put .ttf or .otf files in the data folder's fonts folder to choose from them.");
    }
}

/// One font's choice: `none` for no font of the theme's own, else each
/// loaded; a font named that is not loaded is shown as named.
fn font(
    ui: &mut egui::Ui,
    label: &str,
    chosen: &mut Option<String>,
    fonts: &BTreeSet<String>,
    none: &str,
) {
    ui.horizontal(|ui| {
        ui.label(label);
        let shown = chosen.clone().unwrap_or_else(|| none.to_owned());
        egui::ComboBox::from_id_salt(("theme-font", label))
            .selected_text(shown)
            .show_ui(ui, |ui| {
                ui.selectable_value(chosen, None, none);
                for stem in fonts {
                    ui.selectable_value(chosen, Some(stem.clone()), stem);
                }
            });
        if let Some(name) = chosen
            && !fonts.contains(name)
        {
            ui.weak(format!(
                "{name} is not in the fonts folder; egui's own is drawn"
            ));
        }
    });
}
