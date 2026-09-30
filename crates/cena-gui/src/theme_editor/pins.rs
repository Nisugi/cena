//! The editor's *Pins*: every token by its group, each as it comes out of
//! the harmony, pinned to a colour of the player's own with a click, and
//! for a free token the other hues it could take.

use cena_ui::theme::{Group, Palette, Recipe, Role, Token, hue_variants};

/// The groups in the order shown, each with what to call it.
const GROUPS: [(Group, &str); 8] = [
    (Group::Surfaces, "Surfaces and text"),
    (Group::Text, "The story and the room"),
    (Group::Vitals, "Vitals"),
    (Group::Status, "Status"),
    (Group::Injuries, "Injuries"),
    (Group::Map, "The map"),
    (Group::Marks, "The map's marks"),
    (Group::Chrome, "Chrome"),
];

/// Draw every token's row into `ui`: its colour in `palette` now, its pin
/// in `recipe` made, moved or taken off.
pub(super) fn show(ui: &mut egui::Ui, recipe: &mut Recipe, palette: &Palette) {
    for (group, called) in GROUPS {
        egui::CollapsingHeader::new(called)
            .id_salt(("theme-group", called))
            .show(ui, |ui| {
                for token in Token::ALL.into_iter().filter(|t| t.group() == group) {
                    row(ui, recipe, palette, token);
                }
            });
    }
}

/// One token's row: its name, its pin's switch, its colour, and its hues.
fn row(ui: &mut egui::Ui, recipe: &mut Recipe, palette: &Palette, token: Token) {
    ui.horizontal(|ui| {
        let mut pinned = recipe.pins.contains_key(&token);
        if ui
            .checkbox(&mut pinned, "")
            .on_hover_text("Pinned: kept as written; the rest harmonise round it")
            .changed()
        {
            if pinned {
                recipe.pins.insert(token, palette.get(token));
            } else {
                recipe.pins.remove(&token);
            }
        }
        let mut rgb = recipe
            .pins
            .get(&token)
            .copied()
            .unwrap_or(palette.get(token));
        if ui.color_edit_button_srgb(&mut rgb).changed() {
            recipe.pins.insert(token, rgb);
        }
        ui.label(token.name());
        if let Role::Free { .. } = token.role() {
            for hue in hue_variants(token, recipe).into_iter().skip(1).take(6) {
                let (rect, response) =
                    ui.allocate_exact_size(egui::vec2(12.0, 12.0), egui::Sense::click());
                ui.painter().rect_filled(rect, 2.0, crate::theme::rgb(hue));
                if response
                    .on_hover_text(format!(
                        "Pin {} to {}",
                        token.name(),
                        cena_ui::theme::hex(hue)
                    ))
                    .clicked()
                {
                    recipe.pins.insert(token, hue);
                }
            }
        }
    });
}
