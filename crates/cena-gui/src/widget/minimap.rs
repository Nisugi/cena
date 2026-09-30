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

use cena_ui::{EdgeKind, MapScene, MinimapView, NextDoor, SceneEdge};

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
    let (scene, you, target, route, next_door) = match view {
        Some(MinimapView::Here {
            scene,
            room,
            target,
            route,
            next_door,
        }) => (
            scene.as_ref(),
            *room,
            *target,
            route.as_slice(),
            next_door.as_slice(),
        ),
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
    let camera = aimed(
        ui,
        id,
        &response,
        rect,
        (scene, next_door),
        here_cell,
        inside.is_some(),
    );
    let now = ui.input(|i| i.time);
    let (centre, zoom) = camera.drawn(now);
    if camera.glide.is_some_and(|(.., start)| now - start < GLIDE) {
        ui.ctx().request_repaint();
    }

    let to_screen = |p: Vec2| rect.center() + (p - centre) * zoom;
    // The maps next door first, dimmed, under this one.
    let mut layers: Vec<(&MapScene, Vec2)> = vec![(scene, Vec2::ZERO)];
    layers.extend(
        next_door
            .iter()
            .map(|n| (n.scene.as_ref(), vec2(n.offset.0, n.offset.1))),
    );
    let square = (zoom * 0.7).clamp(3.0, 10.0);
    for &(next, offset) in &layers[1..] {
        let moved = |p: Vec2| to_screen(p + offset);
        draw_lines(&painter, next, &moved, zoom, &|_| FADED);
        draw_rooms(&painter, rect, next, &moved, zoom, &|_| FADED);
    }
    // In a place, the rest of the sheet dimmed; outdoors, everything.
    let fade = |building: Option<usize>| {
        if inside.is_none() || building == inside {
            1.0
        } else {
            FADED
        }
    };
    draw_lines(&painter, scene, &to_screen, zoom, &fade);
    draw_route(&painter, &layers, route, &to_screen);
    draw_rooms(&painter, rect, scene, &to_screen, zoom, &fade);
    if let Some(at) = target.and_then(|t| spot(&layers, t)) {
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
        .and_then(|at| hit(&layers, from_screen(at), reach));
    clicked(ui, &response, pointed, target)
}

/// One sheet's lines, each as dim as `fade` says for the building it is in.
fn draw_lines(
    painter: &egui::Painter,
    scene: &MapScene,
    to_screen: &dyn Fn(Vec2) -> Pos2,
    zoom: f32,
    fade: &dyn Fn(Option<usize>) -> f32,
) {
    for edge in &scene.edges {
        draw_edge(painter, edge, to_screen, zoom, fade(edge.building));
    }
}

/// One sheet's rooms and way-in dots, over its lines and any route.
fn draw_rooms(
    painter: &egui::Painter,
    rect: Rect,
    scene: &MapScene,
    to_screen: &dyn Fn(Vec2) -> Pos2,
    zoom: f32,
    fade: &dyn Fn(Option<usize>) -> f32,
) {
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
    draw_doors(painter, scene, to_screen, zoom, fade(None));
}

/// The camera for this frame, kept for the next: a new area recentres at
/// once; a door crossed, into a place or out, glides to that side's default
/// zoom; following you glides along; the wheel and a drag steer it.
fn aimed(
    ui: &egui::Ui,
    id: Id,
    response: &egui::Response,
    rect: Rect,
    (scene, next_door): (&MapScene, &[NextDoor]),
    here: Vec2,
    inside: bool,
) -> Camera {
    let now = ui.input(|i| i.time);
    let default = if inside { ZOOM_INSIDE } else { ZOOM };
    // A place is opened on its area's sheet: its sheet is named for the
    // area and the place (`area@room`), and is the same area.
    let area_of =
        |scene: &MapScene| egui::util::hash(scene.area.split('@').next().unwrap_or(&scene.area));
    let area = area_of(scene);
    let fresh = Camera {
        centre: here,
        zoom: default,
        follow: true,
        area,
        inside,
        glide: None,
    };
    let mut camera = ui.data(|d| d.get_temp::<Camera>(id)).unwrap_or(fresh);
    // Into a map that was drawn next door: the view moves by where that
    // map was drawn, so nothing jumps.
    let crossed = next_door
        .iter()
        .find(|n| area_of(&n.scene) == camera.area)
        .map(|n| vec2(n.offset.0, n.offset.1));
    if camera.area != area {
        match crossed {
            Some(shift) => {
                camera.area = area;
                camera.centre += shift;
                camera.glide = camera
                    .glide
                    .map(|(from, zoom, start)| (from + shift, zoom, start));
            }
            None => camera = fresh,
        }
    }
    if camera.inside != inside {
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

/// The room drawn nearest `at` within `reach` cells on any of `layers`
/// (each sheet and where it is drawn), or the room behind the way-in dot
/// there.
fn hit(layers: &[(&MapScene, Vec2)], at: Vec2, reach: f32) -> Option<u32> {
    layers
        .iter()
        .flat_map(|&(scene, offset)| {
            let rooms = scene
                .rooms
                .iter()
                .map(move |r| (r.id, cell(r.cell) + offset));
            let doors = scene
                .doors
                .iter()
                .map(move |d| (d.inside, vec2(d.at.0, d.at.1) + offset));
            rooms.chain(doors)
        })
        .map(|(id, p)| (id, (p - at).length()))
        .filter(|&(_, d)| d <= reach)
        .min_by(|a, b| a.1.total_cmp(&b.1))
        .map(|(id, _)| id)
}

/// Where `room` is drawn on the first of `layers` to draw it: its cell, or
/// its first way-in dot.
fn spot(layers: &[(&MapScene, Vec2)], room: u32) -> Option<Vec2> {
    layers
        .iter()
        .find_map(|&(scene, offset)| whereabouts(scene, room).map(|(at, _)| at + offset))
}

/// The route over the rooms of it this sheet draws, each step along its
/// own line where the sheet has one (`atlas-view.mjs:247-252`).
fn draw_route(
    painter: &egui::Painter,
    layers: &[(&MapScene, Vec2)],
    route: &[u32],
    to_screen: &dyn Fn(Vec2) -> Pos2,
) {
    for step in route.windows(2) {
        let (a, b) = (step[0], step[1]);
        let along = layers.iter().find_map(|&(scene, offset)| {
            scene.edges.iter().find_map(|e| {
                let path = e.path.iter().map(|&(x, y)| vec2(x, y) + offset);
                if (e.a, e.b) == (a, b) {
                    Some(path.collect::<Vec<_>>())
                } else if (e.a, e.b) == (b, a) {
                    Some(path.rev().collect())
                } else {
                    None
                }
            })
        });
        let points: Vec<Pos2> = match along {
            Some(path) => path.into_iter().map(to_screen).collect(),
            None => match (spot(layers, a), spot(layers, b)) {
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
mod tests;
