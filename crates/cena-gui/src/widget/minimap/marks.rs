//! What the minimap says beyond lines and rooms (`plan/53` §7 step 7): each
//! place marked in Despana's colours (`preferences.mjs:2-16`), the buildings'
//! names once there is room for them (Despana's 12 pixels a cell), put where
//! they cover no room, and a card for the room under the pointer (the Genie
//! card of §1e: its title, number, where it is, its ways out, what it is).

use cena_ui::MapScene;
use egui::{FontId, Pos2, Rect, Vec2, vec2};

use super::cell;
use crate::theme::{self, T};

/// Pixels a cell before the buildings' names are drawn (`map_view.rs:253`).
const LABELS_FROM: f32 = 12.0;

/// A kind of place: the palette's token for its colour, and what the card
/// calls it.
fn kind(mark: &str) -> Option<(T, &'static str)> {
    Some(match mark {
        "bank" => (T::MarkBank, "Bank"),
        "furrier" => (T::MarkFurrier, "Furrier"),
        "gemshop" => (T::MarkGemshop, "Gemshop"),
        "pawnshop" => (T::MarkPawnshop, "Pawnshop"),
        "advguild" => (T::MarkGuild, "Adventurer's Guild"),
        "locksmith" => (T::MarkLocksmith, "Locksmith"),
        "healer" => (T::MarkHealer, "Healer"),
        "herbalist" => (T::MarkHerbalist, "Herbalist"),
        "alchemist" => (T::MarkAlchemist, "Alchemist"),
        _ => return None,
    })
}

/// Each marked room and way-in dot of `scene`, a small dot of its kind's
/// colour at its top right, as dim as `fade` says for its building.
pub(super) fn draw_marks(
    painter: &egui::Painter,
    scene: &MapScene,
    to_screen: &dyn Fn(Vec2) -> Pos2,
    zoom: f32,
    fade: &dyn Fn(Option<usize>) -> f32,
) {
    let square = (zoom * 0.7).clamp(3.0, 10.0);
    let r = (square * 0.35).max(2.0);
    let marked = scene
        .rooms
        .iter()
        .map(|room| {
            (
                cell(room.cell),
                &room.marks,
                square * 0.5,
                fade(room.building),
            )
        })
        .chain(
            scene
                .doors
                .iter()
                .map(|d| (vec2(d.at.0, d.at.1), &d.marks, 2.5, fade(None))),
        );
    for (at, marks, beside, fade) in marked {
        for (n, (colour, _)) in marks.iter().filter_map(|m| kind(m)).enumerate() {
            #[allow(clippy::cast_precision_loss)]
            let step = n as f32 * r * 2.2;
            let spot = to_screen(at) + vec2(beside + r + step, -beside - r);
            let colour = theme::color(painter.ctx(), colour);
            painter.circle_filled(spot, r, colour.gamma_multiply(fade));
        }
    }
}

/// The buildings' names on `scene`, once a cell is `LABELS_FROM` pixels:
/// above its top-left, else below, right or left of it, wherever it covers
/// no room drawn; none at all where every place would.
pub(super) fn draw_labels(
    painter: &egui::Painter,
    scene: &MapScene,
    to_screen: &dyn Fn(Vec2) -> Pos2,
    zoom: f32,
    fade: &dyn Fn(Option<usize>) -> f32,
) {
    if zoom < LABELS_FROM {
        return;
    }
    let square = (zoom * 0.7).clamp(3.0, 10.0);
    let rooms: Vec<Rect> = scene
        .rooms
        .iter()
        .map(|r| Rect::from_center_size(to_screen(cell(r.cell)), Vec2::splat(square)))
        .collect();
    let font = FontId::proportional(11.0);
    for label in &scene.labels {
        let colour = theme::color(painter.ctx(), T::MapMuted).gamma_multiply(fade(label.building));
        let galley = painter.layout_no_wrap(label.text.clone(), font.clone(), colour);
        let size = galley.size();
        let anchor = to_screen(vec2(label.at.0, label.at.1));
        let gap = square * 0.5 + 2.0;
        let tries = [
            anchor + vec2(-size.x * 0.5, -gap - size.y),
            anchor + vec2(-size.x * 0.5, gap),
            anchor + vec2(gap, -size.y * 0.5),
            anchor + vec2(-gap - size.x, -size.y * 0.5),
        ];
        let free = tries.into_iter().find(|&at| {
            let text = Rect::from_min_size(at, size);
            !rooms.iter().any(|room| room.intersects(text))
        });
        if let Some(at) = free {
            painter.galley(at, galley, colour);
        }
    }
}

/// The card for the room `pointed` finds under the pointer, if any; not
/// while a button is down or let go, or the click would land on it.
pub(super) fn hover(
    ui: &egui::Ui,
    response: &egui::Response,
    pointed: &dyn Fn(Pos2) -> Option<u32>,
    layers: &[(&MapScene, Vec2)],
) {
    let pressing = ui.input(|i| i.pointer.any_down() || i.pointer.any_released());
    let Some(lines) = response
        .hover_pos()
        .filter(|_| !pressing)
        .and_then(pointed)
        .and_then(|room| card(layers, room))
    else {
        return;
    };
    response.clone().on_hover_ui_at_pointer(|ui| {
        // Not selectable: a tooltip holding anything it can take is one a
        // click lands on, and the click is the map's.
        for line in lines {
            ui.add(egui::Label::new(line).selectable(false));
        }
    });
}

/// What the card for room `id` says, from the first of `layers` to draw it:
/// its title (or the place behind a dot), its number, the building or map
/// it is in, its ways out, and what kind of place it is.
pub(super) fn card(layers: &[(&MapScene, Vec2)], id: u32) -> Option<Vec<String>> {
    let area = |scene: &MapScene| {
        scene
            .area
            .split('@')
            .next()
            .unwrap_or(&scene.area)
            .replace('-', " ")
    };
    let kinds = |marks: &[String]| {
        marks
            .iter()
            .filter_map(|m| kind(m).map(|(_, name)| name))
            .collect::<Vec<_>>()
            .join(", ")
    };
    for &(scene, _) in layers {
        if let Some(room) = scene.room(id) {
            let within = room
                .building
                .and_then(|b| scene.buildings.get(b).cloned())
                .unwrap_or_else(|| area(scene));
            let mut lines = vec![room.title.clone(), format!("#{id} · {within}")];
            lines.extend((!room.paths.is_empty()).then(|| room.paths.clone()));
            lines.extend(Some(kinds(&room.marks)).filter(|k| !k.is_empty()));
            return Some(lines);
        }
        if let Some(door) = scene.doors.iter().find(|d| d.inside == id) {
            let mut lines = vec![
                door.place.clone(),
                format!("#{id} · a way in, {}", area(scene)),
            ];
            lines.extend(Some(kinds(&door.marks)).filter(|k| !k.is_empty()));
            return Some(lines);
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    fn town() -> MapScene {
        MapScene {
            area: "cold-river".to_owned(),
            rooms: vec![cena_ui::SceneRoom {
                id: 7,
                cell: (0, 0),
                title: "[Cold River, Market]".to_owned(),
                building: None,
                paths: "Obvious paths: north, east".to_owned(),
                marks: vec!["healer".to_owned(), "bank".to_owned()],
            }],
            doors: vec![cena_ui::SceneDoor {
                street: 7,
                inside: 8,
                at: (0.0, 0.45),
                place: "Rawknuckle's".to_owned(),
                named: false,
                marks: Vec::new(),
            }],
            ..MapScene::default()
        }
    }

    /// A room's card: its title, number and map, its ways out, what it is;
    /// a dot's: the place behind it.
    #[test]
    fn a_card_says_what_the_room_is() {
        let town = town();
        let layers = [(&town, Vec2::ZERO)];
        assert_eq!(
            card(&layers, 7),
            Some(vec![
                "[Cold River, Market]".to_owned(),
                "#7 · cold river".to_owned(),
                "Obvious paths: north, east".to_owned(),
                "Healer, Bank".to_owned(),
            ])
        );
        assert_eq!(
            card(&layers, 8),
            Some(vec![
                "Rawknuckle's".to_owned(),
                "#8 · a way in, cold river".to_owned()
            ])
        );
        assert_eq!(card(&layers, 9), None);
    }
}
