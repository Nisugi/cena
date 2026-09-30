//! The editor's *Generate*: the recipe's seed and background, its scheme
//! and its dials, and seeds offered from the theme the draft began as.

use cena_ui::theme::{Recipe, Scheme, Themes, seed_swatches};

/// Draw the recipe's controls into `ui`, changing `recipe` as the player
/// does.
pub(super) fn show(ui: &mut egui::Ui, recipe: &mut Recipe, themes: &Themes, from: &str) {
    ui.horizontal(|ui| {
        ui.label("Seed");
        ui.color_edit_button_srgb(&mut recipe.seed)
            .on_hover_text("Its hue anchors the scheme; its lightness and chroma, every colour");
        ui.label("Background");
        ui.color_edit_button_srgb(&mut recipe.background)
            .on_hover_text("What every colour must read against; a light one makes a light theme");
    });
    // Seeds worth choosing from the theme the draft began as: its vivid
    // colours, the greys and what is lost in the background left out.
    let raw: Vec<_> = themes
        .outfit(from)
        .map(|outfit| {
            cena_ui::theme::Token::ALL
                .into_iter()
                .map(|token| outfit.palette.get(token))
                .collect()
        })
        .unwrap_or_default();
    let swatches = seed_swatches(&raw, recipe.background, 12);
    if !swatches.is_empty() {
        ui.horizontal_wrapped(|ui| {
            ui.label(format!("Seeds from {from}:"));
            for swatch in swatches {
                let (rect, response) =
                    ui.allocate_exact_size(egui::vec2(18.0, 18.0), egui::Sense::click());
                ui.painter()
                    .rect_filled(rect, 3.0, crate::theme::rgb(swatch));
                if response
                    .on_hover_text(cena_ui::theme::hex(swatch))
                    .clicked()
                {
                    recipe.seed = swatch;
                }
            }
        });
    }
    ui.horizontal(|ui| {
        ui.label("Scheme");
        egui::ComboBox::from_id_salt("theme-scheme")
            .selected_text(recipe.scheme.name())
            .show_ui(ui, |ui| {
                for scheme in Scheme::ALL {
                    ui.selectable_value(&mut recipe.scheme, scheme, scheme.name())
                        .on_hover_text(scheme.description());
                }
            });
        ui.label(recipe.scheme.description());
    });
    ui.add(
        egui::Slider::new(&mut recipe.variance, 0.5..=2.0)
            .text("Spread")
            .show_value(true),
    )
    .on_hover_text("How far the scheme's hues sit from the seed's: 0.7 close, 1.4 far");
    ui.add(egui::Slider::new(&mut recipe.contrast, 1.5..=10.0).text("Contrast"))
        .on_hover_text("The least contrast against the background: 3 low, 4.5 medium, 7 high");
    ui.add(egui::Slider::new(&mut recipe.separation, 0.0..=0.3).text("Separation"))
        .on_hover_text(
            "The least distance between two colours of one group: 0.04 low, 0.09 medium, 0.15 high",
        );
    ui.add(egui::Slider::new(&mut recipe.room_spread, 1.0..=10.0).text("Room plate"))
        .on_hover_text("The room's name's contrast on its plate: 2.5 subtle, 7 hard");
}
