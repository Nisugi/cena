//! The minimap (`plan/53` §7 step 6, Stage 3's first cut): the area the
//! character is in, laid out by the binary, drawn in Despana's colours
//! (§1c) and following you.
//!
//! A pure painter over a [`MapScene`] and a camera kept in egui's memory
//! for the widget. The camera follows you, moving only when you leave the
//! middle of the view (Despana's dead zone); the wheel zooms about the
//! pointer, a drag pans and stops the following, and a double-click comes
//! back to you. In a place, the place is drawn opened on its area's sheet
//! where its dot was, the rest dimmed (§8a, the author: *"I also want the
//! outside rooms to continue to show in some capacity"*), and the camera
//! zooms in: two zooms, outside and in, each back to its default at a door
//! (*"I think it resetting to the default is a better design"*), and a
//! glide between (`VellumFE`'s, §1d).
//!
//! The clicks (§6 item 6, and the author after the first run, 2026-09-29:
//! *"click previewing the route on the minimap, and then a second click or
//! something initiating the travel"*): a room clicked is the target, its
//! route drawn as Despana draws one; clicked again, or right-clicked, it is
//! walked to (`go2`). Shift+click shows the room in the story as `;map`'s
//! does, and Ctrl+click its number (`room`). A way-in dot stands for the
//! place behind it.

use cena_ui::{EdgeKind, MapScene, MinimapView, SceneEdge};

use super::Clicked;
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
/// A route (`preferences.mjs:18`, `routeColor`).
const ROUTE: Color32 = Color32::from_rgb(0x57, 0xf3, 0xcb);

/// Pixels a cell outdoors at first, and in a place, whose rooms are drawn
/// at half the streets' scale: zoomed in so they are no more crowded than
/// the streets are; and the least and most either may be zoomed to.
const ZOOM: f32 = 7.0;
const ZOOM_INSIDE: f32 = 16.0;
const ZOOM_RANGE: (f32, f32) = (2.0, 40.0);
/// Seconds a glide takes (`VellumFE`'s `map_compass.rs:102-112`).
const GLIDE: f64 = 0.25;
/// How much of its colour what is outside the place you are in keeps.
const FADED: f32 = 0.35;
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
    /// Whether you were in a place: a door crossed zooms to the other
    /// side's default.
    inside: bool,
    /// A glide under way to `centre` and `zoom`: where it started, and
    /// when.
    glide: Option<(Vec2, f32, f64)>,
}

impl Camera {
    /// Where the view is this frame: part way along a glide, or where it
    /// is going.
    fn drawn(&self, now: f64) -> (Vec2, f32) {
        let Some((from, zoom, start)) = self.glide else {
            return (self.centre, self.zoom);
        };
        #[allow(clippy::cast_possible_truncation)]
        let t = (((now - start) / GLIDE).clamp(0.0, 1.0)) as f32;
        let eased = t * t * 2.0f32.mul_add(-t, 3.0);
        (
            from + (self.centre - from) * eased,
            zoom + (self.zoom - zoom) * eased,
        )
    }

    /// Glide from where the view is now to where it is going.
    fn glide_from(&mut self, drawn: (Vec2, f32), now: f64) {
        self.glide = Some((drawn.0, drawn.1, now));
    }
}

/// Draw `view` into all the space `ui` gives, and what a click on it asked
/// for; `own` is false on another character's minimap, which only shows.
pub(super) fn minimap(
    ui: &mut egui::Ui,
    view: Option<&MinimapView>,
    id: Id,
    own: bool,
) -> Option<Clicked> {
    let size = ui.available_size().max(vec2(80.0, 60.0));
    let (rect, response) = ui.allocate_exact_size(size, Sense::click_and_drag());
    let painter = ui.painter_at(rect);
    painter.rect_filled(rect, 4.0, BACKGROUND);
    let (scene, you, target, route) = match view {
        Some(MinimapView::Here {
            scene,
            room,
            target,
            route,
        }) => (scene.as_ref(), *room, *target, route.as_slice()),
        Some(MinimapView::Waiting(why)) => {
            waiting(&painter, rect, why);
            return None;
        }
        None => {
            waiting(&painter, rect, "No map.");
            return None;
        }
    };
    let Some((here_cell, inside)) = whereabouts(scene, you) else {
        waiting(&painter, rect, "You are not on this area's map.");
        return None;
    };
    let camera = aimed(ui, id, &response, rect, scene, here_cell, inside.is_some());
    let now = ui.input(|i| i.time);
    let (centre, zoom) = camera.drawn(now);
    if camera.glide.is_some_and(|(.., start)| now - start < GLIDE) {
        ui.ctx().request_repaint();
    }

    let to_screen = |p: Vec2| rect.center() + (p - centre) * zoom;
    // In a place, the rest of the sheet dimmed; outdoors, everything.
    let fade = |building: Option<usize>| {
        if inside.is_none() || building == inside {
            1.0
        } else {
            FADED
        }
    };
    for edge in &scene.edges {
        draw_edge(&painter, edge, &to_screen, zoom, fade(edge.building));
    }
    draw_route(&painter, scene, route, &to_screen);
    let square = (zoom * 0.7).clamp(3.0, 10.0);
    for room in &scene.rooms {
        let at = to_screen(cell(room.cell));
        if rect.expand(square).contains(at) {
            let f = fade(room.building);
            painter.rect(
                Rect::from_center_size(at, Vec2::splat(square)),
                1.4,
                ROOM_FILL.gamma_multiply(f),
                Stroke::new(1.0, ROOM_STROKE.gamma_multiply(f)),
                egui::StrokeKind::Inside,
            );
        }
    }
    draw_doors(&painter, scene, &to_screen, zoom, fade(None));
    if let Some(at) = target.and_then(|t| spot(scene, t)) {
        painter.circle_stroke(to_screen(at), square * 0.9 + 3.0, Stroke::new(2.0, ROUTE));
    }
    painter.circle_stroke(
        to_screen(here_cell),
        square * 0.9 + 3.0,
        Stroke::new(2.5, YOU),
    );
    if !own {
        return None;
    }
    let from_screen = |p: Pos2| centre + (p - rect.center()) / zoom;
    let reach = (square * 0.9 + 3.0) / zoom;
    let pointed = response
        .interact_pointer_pos()
        .and_then(|at| hit(scene, from_screen(at), reach, &|_| true, true));
    clicked(ui, &response, pointed, target)
}

/// The camera for this frame, kept for the next: a new area recentres at
/// once; a door crossed, into a place or out, glides to that side's default
/// zoom; following you glides along; the wheel and a drag steer it.
fn aimed(
    ui: &egui::Ui,
    id: Id,
    response: &egui::Response,
    rect: Rect,
    scene: &MapScene,
    here: Vec2,
    inside: bool,
) -> Camera {
    let now = ui.input(|i| i.time);
    let default = if inside { ZOOM_INSIDE } else { ZOOM };
    // A place is opened on its area's sheet: its sheet is named for the
    // area and the place (`area@room`), and is the same area.
    let area = egui::util::hash(scene.area.split('@').next().unwrap_or(&scene.area));
    let fresh = Camera {
        centre: here,
        zoom: default,
        follow: true,
        area,
        inside,
        glide: None,
    };
    let mut camera = ui.data(|d| d.get_temp::<Camera>(id)).unwrap_or(fresh);
    if camera.area != area {
        camera = fresh;
    } else if camera.inside != inside {
        let was = camera.drawn(now);
        camera = Camera {
            glide: None,
            ..fresh
        };
        camera.glide_from(was, now);
    }
    let before = (camera.centre, camera.zoom);
    steer(ui, response, rect, &mut camera);
    if (camera.centre, camera.zoom) != before {
        // Steered by hand: no glide to fight it.
        camera.glide = None;
    } else if camera.follow {
        let was = camera.drawn(now);
        keep_in_view(&mut camera, here, rect.size());
        if camera.centre != before.0 {
            camera.glide_from(was, now);
        }
    }
    ui.data_mut(|d| d.insert_temp(id, camera));
    camera
}

/// What a click on the room `pointed` at asks, as the modifiers held say.
fn clicked(
    ui: &egui::Ui,
    response: &egui::Response,
    pointed: Option<u32>,
    target: Option<u32>,
) -> Option<Clicked> {
    let hydra = |word: String, echo| Some(Clicked::Hydra { word, echo });
    if response.secondary_clicked() {
        return pointed.and_then(|room| hydra(format!("go2 {room}"), true));
    }
    if !response.clicked() {
        return None;
    }
    let modifiers = ui.input(|i| i.modifiers);
    match pointed {
        Some(room) if modifiers.shift => hydra(format!("room {room}"), false),
        Some(room) if modifiers.command => hydra(format!("room {room} number"), false),
        Some(room) if target == Some(room) => hydra(format!("go2 {room}"), true),
        Some(room) => Some(Clicked::Aim(Some(room))),
        None => target.map(|_| Clicked::Aim(None)),
    }
}

/// The room drawn nearest `at` within `reach` cells, or the room behind the
/// way-in dot there: only what is shown, and dots only outdoors.
fn hit(
    scene: &MapScene,
    at: Vec2,
    reach: f32,
    shown: &dyn Fn(Option<usize>) -> bool,
    outdoors: bool,
) -> Option<u32> {
    let rooms = scene
        .rooms
        .iter()
        .filter(|r| shown(r.building))
        .map(|r| (r.id, cell(r.cell)));
    let doors = scene
        .doors
        .iter()
        .filter(|_| outdoors)
        .map(|d| (d.inside, vec2(d.at.0, d.at.1)));
    rooms
        .chain(doors)
        .map(|(id, p)| (id, (p - at).length()))
        .filter(|&(_, d)| d <= reach)
        .min_by(|a, b| a.1.total_cmp(&b.1))
        .map(|(id, _)| id)
}

/// Where `room` is drawn: its cell, or its first way-in dot.
fn spot(scene: &MapScene, room: u32) -> Option<Vec2> {
    whereabouts(scene, room).map(|(at, _)| at)
}

/// The route over the rooms of it this sheet draws, each step along its
/// own line where the sheet has one (`atlas-view.mjs:247-252`).
fn draw_route(
    painter: &egui::Painter,
    scene: &MapScene,
    route: &[u32],
    to_screen: &dyn Fn(Vec2) -> Pos2,
) {
    for step in route.windows(2) {
        let (a, b) = (step[0], step[1]);
        let along = scene.edges.iter().find_map(|e| {
            if (e.a, e.b) == (a, b) {
                Some(e.path.clone())
            } else if (e.a, e.b) == (b, a) {
                Some(e.path.iter().rev().copied().collect())
            } else {
                None
            }
        });
        let points: Vec<Pos2> = match along {
            Some(path) => path.iter().map(|&(x, y)| to_screen(vec2(x, y))).collect(),
            None => match (spot(scene, a), spot(scene, b)) {
                (Some(from), Some(to)) => vec![to_screen(from), to_screen(to)],
                _ => continue,
            },
        };
        painter.add(Shape::line(
            points.clone(),
            Stroke::new(5.0, ROUTE.gamma_multiply(0.3)),
        ));
        painter.extend(Shape::dashed_line(
            &points,
            Stroke::new(2.0, ROUTE),
            6.0,
            4.0,
        ));
    }
}

/// Where you are drawn, and the building you are in: your room, or, for a
/// room the layout draws only as dots at the rooms it is entered from (the
/// Issenflow's current), the first of those dots.
fn whereabouts(scene: &MapScene, you: u32) -> Option<(Vec2, Option<usize>)> {
    if let Some(here) = scene.room(you) {
        return Some((cell(here.cell), here.building));
    }
    let door = scene.doors.iter().find(|d| d.inside == you)?;
    Some((vec2(door.at.0, door.at.1), None))
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
    fade: f32,
) {
    let (line, connector) = (LINE.gamma_multiply(fade), CONNECTOR.gamma_multiply(fade));
    let points: Vec<Pos2> = edge
        .path
        .iter()
        .map(|&(x, y)| to_screen(vec2(x, y)))
        .collect();
    match edge.kind {
        EdgeKind::Directional => {
            painter.add(Shape::line(points, Stroke::new(1.35, line)));
        }
        EdgeKind::Connector => {
            painter.extend(Shape::dashed_line(
                &points,
                Stroke::new(1.0, line.gamma_multiply(0.7)),
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
            let stroke = Stroke::new(1.0, connector.gamma_multiply(1.6));
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
    fade: f32,
) {
    let colour = DOOR.gamma_multiply(fade);
    for door in &scene.doors {
        let at = to_screen(vec2(door.at.0, door.at.1));
        let r = if door.named { 3.5 } else { 2.2 } * (zoom / ZOOM).clamp(0.6, 1.6);
        painter.circle_filled(at, r, colour);
        if door.named {
            painter.text(
                at + vec2(r + 3.0, 0.0),
                egui::Align2::LEFT_CENTER,
                &door.place,
                FontId::proportional(11.0),
                colour,
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
            .build_ui(move |ui| {
                minimap(ui, Some(&view), Id::new("minimap"), true);
            })
    }

    /// You in the town at `room`, routed to `target` along `route`.
    fn here(room: u32, target: Option<u32>, route: &[u32]) -> MinimapView {
        MinimapView::Here {
            scene: Arc::new(town()),
            room,
            target,
            route: route.to_vec(),
        }
    }

    /// The town, you at its middle room.
    #[test]
    fn the_minimap_draws_the_area_and_you() {
        let mut harness = drawn(here(2, None, &[]));
        harness.run();
        harness.snapshot("minimap");
    }

    /// In the shop, the shop, and the rest of the town dimmed round it,
    /// zoomed in.
    #[test]
    fn in_a_place_the_rest_is_dimmed() {
        let mut harness = drawn(here(5, None, &[]));
        harness.run();
        harness.snapshot("minimap_inside");
    }

    /// Into the shop, the camera glides to the inside zoom; back out, to
    /// the outside one, whatever the wheel did in between.
    #[test]
    fn a_door_crossed_zooms_to_that_sides_default() {
        let view = Arc::new(std::sync::Mutex::new(here(2, None, &[])));
        let shown = Arc::clone(&view);
        let mut harness = Harness::builder()
            .with_size((260.0, 200.0))
            .build_ui(move |ui| {
                let view = shown.lock().expect("unpoisoned").clone();
                minimap(ui, Some(&view), Id::new("minimap"), true);
            });
        let camera = |harness: &Harness<'_, ()>| {
            harness
                .ctx
                .data(|d| d.get_temp::<Camera>(Id::new("minimap")))
                .expect("kept")
        };
        harness.run();
        assert!((camera(&harness).zoom - ZOOM).abs() < 1e-3);
        *view.lock().expect("unpoisoned") = here(5, None, &[]);
        harness.run();
        let inside = camera(&harness);
        assert!((inside.zoom - ZOOM_INSIDE).abs() < 1e-3);
        assert!(inside.glide.is_some(), "it glides in");
        *view.lock().expect("unpoisoned") = here(2, None, &[]);
        harness.run();
        assert!((camera(&harness).zoom - ZOOM).abs() < 1e-3);
    }

    /// A glide eases from where it was to where it goes over `GLIDE`.
    #[test]
    fn a_glide_eases_there() {
        let camera = Camera {
            centre: vec2(10.0, 0.0),
            zoom: 20.0,
            follow: true,
            area: 0,
            inside: true,
            glide: Some((vec2(0.0, 0.0), 10.0, 1.0)),
        };
        assert_eq!(camera.drawn(1.0), (vec2(0.0, 0.0), 10.0));
        assert_eq!(camera.drawn(1.0 + GLIDE / 2.0), (vec2(5.0, 0.0), 15.0));
        assert_eq!(camera.drawn(2.0), (vec2(10.0, 0.0), 20.0));
    }

    /// The dock clicked: its route from you, along the town's own lines,
    /// and the dock ringed.
    #[test]
    fn a_target_is_ringed_and_its_route_drawn() {
        let mut harness = drawn(here(2, Some(6), &[2, 3, 6]));
        harness.run();
        harness.snapshot("minimap_route");
    }

    /// What one click at `at` with `modifiers`, by `button`, asks of the
    /// minimap showing `view`; `own` false for another character's.
    fn click(
        view: MinimapView,
        own: bool,
        at: Pos2,
        button: egui::PointerButton,
        modifiers: egui::Modifiers,
    ) -> Option<Clicked> {
        let asked = Arc::new(std::sync::Mutex::new(None));
        let out = Arc::clone(&asked);
        let mut harness = Harness::builder()
            .with_size((260.0, 200.0))
            .build_ui(move |ui| {
                if let Some(clicked) = minimap(ui, Some(&view), Id::new("minimap"), own) {
                    *out.lock().expect("unpoisoned") = Some(clicked);
                }
            });
        harness.run();
        for pressed in [true, false] {
            harness
                .input_mut()
                .events
                .push(egui::Event::PointerMoved(at));
            harness.input_mut().events.push(egui::Event::PointerButton {
                pos: at,
                button,
                pressed,
                modifiers,
            });
            harness.input_mut().modifiers = modifiers;
            harness.run();
        }
        asked.lock().expect("unpoisoned").clone()
    }

    /// The clicks the author asked for (`plan/53` §6 item 6, and after the
    /// first run): a room aimed at, walked to when clicked again or
    /// right-clicked, said in the story with Shift, its number with Ctrl;
    /// a click on nothing forgets the target; another character's minimap
    /// does nothing. You are at room 2, the view's middle (130, 100); room
    /// 3 is four cells east, `ZOOM` pixels a cell.
    #[test]
    fn the_clicks_aim_walk_and_tell() {
        use egui::{
            Modifiers,
            PointerButton::{Primary, Secondary},
        };
        let three = Pos2::new(4.0f32.mul_add(ZOOM, 130.0), 100.0);
        let nothing = Pos2::new(40.0, 170.0);
        let hydra = |word: &str, echo| {
            Some(Clicked::Hydra {
                word: word.to_owned(),
                echo,
            })
        };
        let none = Modifiers::NONE;
        assert_eq!(
            click(here(2, None, &[]), true, three, Primary, none),
            Some(Clicked::Aim(Some(3)))
        );
        assert_eq!(
            click(here(2, Some(3), &[2, 3]), true, three, Primary, none),
            hydra("go2 3", true)
        );
        assert_eq!(
            click(here(2, None, &[]), true, three, Secondary, none),
            hydra("go2 3", true)
        );
        assert_eq!(
            click(here(2, None, &[]), true, three, Primary, Modifiers::SHIFT),
            hydra("room 3", false)
        );
        assert_eq!(
            click(here(2, None, &[]), true, three, Primary, Modifiers::COMMAND),
            hydra("room 3 number", false)
        );
        assert_eq!(
            click(here(2, Some(3), &[2, 3]), true, nothing, Primary, none),
            Some(Clicked::Aim(None))
        );
        assert_eq!(click(here(2, None, &[]), false, three, Primary, none), None);
    }

    /// A click finds the room drawn nearest it within reach, or the place
    /// behind a way-in dot; nothing out of reach.
    #[test]
    fn a_click_finds_the_room_or_the_dot() {
        let town = town();
        let all = |_: Option<usize>| true;
        assert_eq!(hit(&town, vec2(8.2, 0.1), 0.5, &all, true), Some(3));
        assert_eq!(hit(&town, vec2(4.0, 4.4), 0.2, &all, true), Some(40));
        assert_eq!(
            hit(&town, vec2(4.0, 4.4), 0.2, &all, false),
            None,
            "no dots indoors"
        );
        assert_eq!(hit(&town, vec2(20.0, 2.0), 0.5, &all, true), None);
    }

    /// A drawn room is where you are, with its building; a room drawn only
    /// as a dot is at the dot; a room drawn neither way is nowhere.
    #[test]
    fn you_are_at_your_room_or_its_dot() {
        let town = town();
        assert_eq!(whereabouts(&town, 5), Some((vec2(5.0, 1.0), Some(0))));
        assert_eq!(whereabouts(&town, 40), Some((vec2(4.0, 4.45), None)));
        assert_eq!(whereabouts(&town, 99), None);
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
            zoom: 9.0,
            follow: true,
            area: 0,
            inside: false,
            glide: None,
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
