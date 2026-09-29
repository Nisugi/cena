//! The minimap (`plan/53` §7 step 6, Stage 3's first cut): the area the
//! character is in, laid out by the binary, drawn in Despana's colours
//! (§1c) and following you.
//!
//! A pure painter over a [`MapScene`] and a camera kept in egui's memory
//! for the widget. The camera follows you, moving only when you leave the
//! middle of the view (Despana's dead zone); the wheel zooms about the
//! pointer, a drag pans and stops the following, and a double-click comes
//! back to you. In a building, only the building is drawn (`VellumFE`'s rule,
//! §1d).

use cena_ui::{EdgeKind, MapScene, MinimapView, SceneEdge};
use egui::{Color32, FontId, Id, Pos2, Rect, Sense, Shape, Stroke, Vec2, vec2};

/// The minimap's inset background (`assets/style.css:141-154`).
const BACKGROUND: Color32 = Color32::from_rgb(0x11, 0x16, 0x1b);
/// A room: fill and edge (`atlas-view.mjs:275-276`).
const ROOM_FILL: Color32 = Color32::from_rgb(0x49, 0x7f, 0xa3);
const ROOM_STROKE: Color32 = Color32::from_rgb(0xa2, 0xc8, 0xdf);
/// A line with a direction (`atlas-view.mjs:233`).
const LINE: Color32 = Color32::from_rgb(0x6e, 0x99, 0xb5);
/// A line without one, dashed.
const CONNECTOR: Color32 = Color32::from_rgb(0x38, 0x51, 0x64);
/// A way in, and its place's name: the transition gold (`atlas-view.mjs:303`).
const DOOR: Color32 = Color32::from_rgb(0xff, 0xc7, 0x78);
/// You (`minimap.mjs:132-147`).
const YOU: Color32 = Color32::from_rgb(0xdd, 0xdc, 0xd7);
/// Muted text, for what is waiting.
const MUTED: Color32 = Color32::from_rgb(0x8e, 0x9f, 0xad);

/// Pixels a cell is at first, and the least and most it may be zoomed to.
const ZOOM: f32 = 9.0;
const ZOOM_RANGE: (f32, f32) = (2.0, 40.0);
/// How far from the middle you may go, as a share of the view each way,
/// before the camera moves (Despana's dead zone, 24 x 16 of 80 x 56).
const DEAD_ZONE: f32 = 0.3;

/// The camera, kept per widget between frames.
#[derive(Clone, Copy, Debug)]
struct Camera {
    /// The cell in the middle of the view.
    centre: Vec2,
    /// Pixels a cell.
    zoom: f32,
    /// Whether it follows you; a drag stops it until a double-click.
    follow: bool,
    /// The area it was last on: another area recentres.
    area: u64,
}

/// Draw `view` into all the space `ui` gives.
pub(super) fn minimap(ui: &mut egui::Ui, view: Option<&MinimapView>, id: Id) {
    let size = ui.available_size().max(vec2(80.0, 60.0));
    let (rect, response) = ui.allocate_exact_size(size, Sense::click_and_drag());
    let painter = ui.painter_at(rect);
    painter.rect_filled(rect, 4.0, BACKGROUND);
    let (scene, you) = match view {
        Some(MinimapView::Here { scene, room }) => (scene.as_ref(), *room),
        Some(MinimapView::Waiting(why)) => return waiting(&painter, rect, why),
        None => return waiting(&painter, rect, "No map."),
    };
    let Some(here) = scene.room(you) else {
        return waiting(&painter, rect, "You are not on this area's map.");
    };
    let here_cell = cell(here.cell);
    let area = egui::util::hash(&scene.area);
    let mut camera = ui.data(|d| d.get_temp::<Camera>(id)).unwrap_or(Camera {
        centre: here_cell,
        zoom: ZOOM,
        follow: true,
        area,
    });
    if camera.area != area {
        camera = Camera {
            centre: here_cell,
            area,
            follow: true,
            ..camera
        };
    }
    steer(ui, &response, rect, &mut camera);
    if camera.follow {
        keep_in_view(&mut camera, here_cell, rect.size());
    }
    ui.data_mut(|d| d.insert_temp(id, camera));

    let to_screen = |p: Vec2| rect.center() + (p - camera.centre) * camera.zoom;
    // In a building, only the building; outdoors, everything.
    let inside = here.building;
    let shown = |building: Option<usize>| inside.is_none() || building == inside;
    for edge in scene.edges.iter().filter(|e| shown(e.building)) {
        draw_edge(&painter, edge, &to_screen, camera.zoom);
    }
    let square = (camera.zoom * 0.7).clamp(3.0, 10.0);
    for room in scene.rooms.iter().filter(|r| shown(r.building)) {
        let at = to_screen(cell(room.cell));
        if rect.expand(square).contains(at) {
            painter.rect(
                Rect::from_center_size(at, Vec2::splat(square)),
                1.4,
                ROOM_FILL,
                Stroke::new(1.0, ROOM_STROKE),
                egui::StrokeKind::Inside,
            );
        }
    }
    if inside.is_none() {
        draw_doors(&painter, scene, &to_screen, camera.zoom);
    }
    painter.circle_stroke(
        to_screen(here_cell),
        square * 0.9 + 3.0,
        Stroke::new(2.5, YOU),
    );
}

/// Wheel zoom about the pointer, drag to pan, double-click back to you.
fn steer(ui: &egui::Ui, response: &egui::Response, rect: Rect, camera: &mut Camera) {
    if response.dragged() {
        camera.centre -= response.drag_delta() / camera.zoom;
        camera.follow = false;
    }
    if response.double_clicked() {
        camera.follow = true;
    }
    if response.hovered() {
        let scroll = ui.input(|i| i.smooth_scroll_delta.y);
        if scroll != 0.0 {
            let pointer = ui.input(|i| i.pointer.hover_pos()).unwrap_or(rect.center());
            let before = camera.centre + (pointer - rect.center()) / camera.zoom;
            camera.zoom = (camera.zoom * (scroll / 200.0).exp()).clamp(ZOOM_RANGE.0, ZOOM_RANGE.1);
            camera.centre = before - (pointer - rect.center()) / camera.zoom;
        }
    }
}

/// Move the camera only as far as keeps `you` within the middle of a view
/// of `size`.
fn keep_in_view(camera: &mut Camera, you: Vec2, size: Vec2) {
    let half = size / camera.zoom * DEAD_ZONE;
    let off = you - camera.centre;
    if off.x.abs() > half.x {
        camera.centre.x += off.x - half.x * off.x.signum();
    }
    if off.y.abs() > half.y {
        camera.centre.y += off.y - half.y * off.y.signum();
    }
}

fn draw_edge(
    painter: &egui::Painter,
    edge: &SceneEdge,
    to_screen: &dyn Fn(Vec2) -> Pos2,
    zoom: f32,
) {
    let points: Vec<Pos2> = edge
        .path
        .iter()
        .map(|&(x, y)| to_screen(vec2(x, y)))
        .collect();
    match edge.kind {
        EdgeKind::Directional => {
            painter.add(Shape::line(points, Stroke::new(1.35, LINE)));
        }
        EdgeKind::Connector => {
            painter.extend(Shape::dashed_line(
                &points,
                Stroke::new(1.0, LINE.gamma_multiply(0.7)),
                5.0,
                4.0,
            ));
        }
        EdgeKind::Stub => {
            // A short mark out of each end toward the other.
            let (Some(&a), Some(&b)) = (points.first(), points.last()) else {
                return;
            };
            let length = (a.distance(b) * 0.2).min(15.0).min(zoom * 2.0);
            let toward = (b - a).normalized() * length;
            let stroke = Stroke::new(1.0, CONNECTOR.gamma_multiply(1.6));
            painter.extend(Shape::dashed_line(&[a, a + toward], stroke, 3.0, 2.0));
            painter.extend(Shape::dashed_line(&[b, b - toward], stroke, 3.0, 2.0));
        }
    }
}

fn draw_doors(
    painter: &egui::Painter,
    scene: &MapScene,
    to_screen: &dyn Fn(Vec2) -> Pos2,
    zoom: f32,
) {
    for door in &scene.doors {
        let at = to_screen(vec2(door.at.0, door.at.1));
        let r = if door.named { 3.5 } else { 2.2 } * (zoom / ZOOM).clamp(0.6, 1.6);
        painter.circle_filled(at, r, DOOR);
        if door.named {
            painter.text(
                at + vec2(r + 3.0, 0.0),
                egui::Align2::LEFT_CENTER,
                &door.place,
                FontId::proportional(11.0),
                DOOR,
            );
        }
    }
}

fn waiting(painter: &egui::Painter, rect: Rect, why: &str) {
    painter.text(
        rect.center(),
        egui::Align2::CENTER_CENTER,
        why,
        FontId::proportional(12.0),
        MUTED,
    );
}

#[allow(clippy::cast_precision_loss)]
fn cell((x, y): (i32, i32)) -> Vec2 {
    vec2(x as f32, y as f32)
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use cena_ui::{SceneDoor, SceneRoom};
    use egui_kittest::Harness;

    use super::*;

    /// A town of four street rooms, a shop off the middle one, a dock by
    /// a line with no direction, a far link drawn as marks, and a named
    /// place's way in.
    ///
    /// ```text
    /// 1 --- 2 --- 3        (you at 2)
    ///       |\
    ///       4  5 shop      6 dock, by `go dock` from 3; 7 far, marks from 1
    /// ```
    fn town() -> MapScene {
        let room = |id: u32, cell: (i32, i32), building: Option<usize>| SceneRoom {
            id,
            cell,
            title: format!("[Room {id}]"),
            building,
        };
        let line = |a: u32, b: u32, kind: EdgeKind, path: &[(f32, f32)], building| SceneEdge {
            a,
            b,
            kind,
            path: path.to_vec(),
            label: None,
            building,
        };
        MapScene {
            area: "test-town".to_owned(),
            rooms: vec![
                room(1, (0, 0), None),
                room(2, (4, 0), None),
                room(3, (8, 0), None),
                room(4, (4, 4), None),
                room(5, (5, 1), Some(0)),
                room(6, (8, 4), None),
                room(7, (30, 0), None),
            ],
            edges: vec![
                line(1, 2, EdgeKind::Directional, &[(0.0, 0.0), (4.0, 0.0)], None),
                line(2, 3, EdgeKind::Directional, &[(4.0, 0.0), (8.0, 0.0)], None),
                line(2, 4, EdgeKind::Directional, &[(4.0, 0.0), (4.0, 4.0)], None),
                line(2, 5, EdgeKind::Connector, &[(4.0, 0.0), (5.0, 1.0)], None),
                line(
                    3,
                    6,
                    EdgeKind::Connector,
                    &[(8.0, 0.0), (9.0, 2.0), (8.0, 4.0)],
                    None,
                ),
                line(1, 7, EdgeKind::Stub, &[(0.0, 0.0), (30.0, 0.0)], None),
            ],
            doors: vec![SceneDoor {
                street: 4,
                inside: 40,
                at: (4.0, 4.45),
                place: "Angargreft".to_owned(),
                named: true,
            }],
            buildings: vec!["Shop".to_owned()],
            min: (0, 0),
            max: (30, 4),
        }
    }

    fn drawn(view: MinimapView) -> Harness<'static, ()> {
        Harness::builder()
            .with_size((260.0, 200.0))
            .build_ui(move |ui| minimap(ui, Some(&view), Id::new("minimap")))
    }

    /// The town, you at its middle room.
    #[test]
    fn the_minimap_draws_the_area_and_you() {
        let mut harness = drawn(MinimapView::Here {
            scene: Arc::new(town()),
            room: 2,
        });
        harness.run();
        harness.snapshot("minimap");
    }

    /// In the shop, only the shop.
    #[test]
    fn in_a_building_only_the_building() {
        let mut harness = drawn(MinimapView::Here {
            scene: Arc::new(town()),
            room: 5,
        });
        harness.run();
        harness.snapshot("minimap_inside");
    }

    /// Nothing to draw says why, where the map would be.
    #[test]
    fn waiting_says_why() {
        let mut harness = drawn(MinimapView::Waiting(
            "Laying out the map: 3 of 207 areas.".to_owned(),
        ));
        harness.run();
        harness.snapshot("minimap_waiting");
    }

    /// The camera stays put while you are in the middle of the view, and
    /// moves only as far as brings you back to its edge.
    #[test]
    fn the_camera_moves_only_past_the_dead_zone() {
        let mut camera = Camera {
            centre: vec2(0.0, 0.0),
            zoom: ZOOM,
            follow: true,
            area: 0,
        };
        // A view 180 by 180 pixels at 9 a cell is 20 cells; the dead zone
        // 6 either way.
        let size = vec2(180.0, 180.0);
        keep_in_view(&mut camera, vec2(5.0, -5.0), size);
        assert_eq!(camera.centre, vec2(0.0, 0.0), "moved inside the dead zone");
        keep_in_view(&mut camera, vec2(10.0, 0.0), size);
        assert_eq!(
            camera.centre,
            vec2(4.0, 0.0),
            "moved past what brings you back"
        );
    }
}
