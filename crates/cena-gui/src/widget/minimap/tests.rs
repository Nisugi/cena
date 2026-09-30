//! The minimap's tests: its pictures, its clicks, its camera.

use std::sync::Arc;

use cena_ui::{SceneDoor, SceneRoom};
use egui_kittest::Harness;
use egui_kittest::kittest::Queryable;

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
        paths: "Obvious paths: east, west".to_owned(),
        marks: if id == 3 {
            vec!["healer".to_owned()]
        } else {
            Vec::new()
        },
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
            marks: Vec::new(),
        }],
        buildings: vec!["Shop".to_owned()],
        labels: vec![cena_ui::SceneLabel {
            text: "Shop".to_owned(),
            at: (5.0, 1.0),
            building: Some(0),
        }],
        min: (0, 0),
        max: (30, 4),
    }
}

fn drawn(view: MinimapView) -> Harness<'static, ()> {
    Harness::builder()
        .with_size((260.0, 200.0))
        .build_ui(move |ui| {
            minimap(
                ui,
                Some(&view),
                Id::new("minimap"),
                true,
                MinimapLook::default(),
            );
        })
}

/// You in the town at `room`, routed to `target` along `route`.
fn here(room: u32, target: Option<u32>, route: &[u32]) -> MinimapView {
    MinimapView::Here {
        scene: Arc::new(town()),
        room,
        target,
        route: route.to_vec(),
        next_door: Vec::new(),
    }
}

/// A farm next door, two rooms, joined east of room 3 at (8, 0): drawn
/// from (12, 0).
fn farm() -> NextDoor {
    NextDoor {
        scene: Arc::new(MapScene {
            area: "test-farm".to_owned(),
            rooms: vec![
                SceneRoom {
                    id: 90,
                    cell: (0, 0),
                    title: "[Farm, Gate]".to_owned(),
                    building: None,
                    paths: String::new(),
                    marks: Vec::new(),
                },
                SceneRoom {
                    id: 91,
                    cell: (4, 0),
                    title: "[Farm, Barn]".to_owned(),
                    building: None,
                    paths: String::new(),
                    marks: Vec::new(),
                },
            ],
            edges: vec![SceneEdge {
                a: 90,
                b: 91,
                kind: EdgeKind::Directional,
                path: vec![(0.0, 0.0), (4.0, 0.0)],
                label: None,
                building: None,
            }],
            ..MapScene::default()
        }),
        offset: (12.0, 0.0),
        ring: 1,
    }
}

/// The town with the farm next door, you at `room` of `scene`.
fn with_farm(room: u32) -> MinimapView {
    MinimapView::Here {
        scene: Arc::new(town()),
        room,
        target: None,
        route: Vec::new(),
        next_door: vec![farm()],
    }
}

/// The farm next door is drawn dimmed beside the town, and a room of it
/// is clicked like one of the town's.
#[test]
fn a_map_next_door_is_drawn_dimmed_and_clicked() {
    let mut harness = drawn(with_farm(3));
    harness.run();
    harness.snapshot("minimap_next_door");
    let (town, farm) = (town(), farm());
    let layers = [(&town, Vec2::ZERO), (farm.scene.as_ref(), vec2(12.0, 0.0))];
    assert_eq!(hit(&layers, vec2(16.1, 0.0), 0.5), Some(91));
    assert_eq!(spot(&layers, 90), Some(vec2(12.0, 0.0)));
}

/// Walking from the town into the farm: the farm is now the sheet, the
/// town next door to it, and the view moves by where the farm was drawn,
/// so it does not jump.
#[test]
fn crossing_into_a_map_next_door_does_not_jump() {
    let view = Arc::new(std::sync::Mutex::new(with_farm(3)));
    let shown = Arc::clone(&view);
    let mut harness = Harness::builder()
        .with_size((260.0, 200.0))
        .build_ui(move |ui| {
            let view = shown.lock().expect("unpoisoned").clone();
            minimap(
                ui,
                Some(&view),
                Id::new("minimap"),
                true,
                MinimapLook::default(),
            );
        });
    harness.run();
    let camera = |harness: &Harness<'_, ()>| {
        harness
            .ctx
            .data(|d| d.get_temp::<Camera>(Id::new("minimap")))
            .expect("kept")
    };
    let before = camera(&harness).centre;
    // On the farm's own sheet the town is drawn at (-12, 0).
    let town = NextDoor {
        scene: Arc::new(town()),
        offset: (-12.0, 0.0),
        ring: 1,
    };
    *view.lock().expect("unpoisoned") = MinimapView::Here {
        scene: Arc::clone(&farm().scene),
        room: 90,
        target: None,
        route: Vec::new(),
        next_door: vec![town],
    };
    harness.run();
    assert_eq!(
        camera(&harness).centre,
        before + vec2(-12.0, 0.0),
        "the view moved with the sheet"
    );
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
            minimap(
                ui,
                Some(&view),
                Id::new("minimap"),
                true,
                MinimapLook::default(),
            );
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
            if let Some(clicked) = minimap(
                ui,
                Some(&view),
                Id::new("minimap"),
                own,
                MinimapLook::default(),
            ) {
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
    let layers = [(&town, Vec2::ZERO)];
    assert_eq!(hit(&layers, vec2(8.2, 0.1), 0.5), Some(3));
    assert_eq!(hit(&layers, vec2(4.0, 4.4), 0.2), Some(40));
    assert_eq!(hit(&layers, vec2(20.0, 2.0), 0.5), None);
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

/// The page's settings take: with no maps next door the farm is not there
/// to click, and a door goes to the page's inside zoom.
#[test]
fn the_page_sets_the_zooms_and_the_maps_next_door() {
    let run = |view: MinimapView, look: MinimapLook, at: Option<Pos2>| {
        let asked = Arc::new(std::sync::Mutex::new(None));
        let out = Arc::clone(&asked);
        let mut harness = Harness::builder()
            .with_size((260.0, 200.0))
            .build_ui(move |ui| {
                if let Some(clicked) = minimap(ui, Some(&view), Id::new("minimap"), true, look) {
                    *out.lock().expect("unpoisoned") = Some(clicked);
                }
            });
        harness.run();
        if let Some(at) = at {
            for pressed in [true, false] {
                harness
                    .input_mut()
                    .events
                    .push(egui::Event::PointerMoved(at));
                harness.input_mut().events.push(egui::Event::PointerButton {
                    pos: at,
                    button: egui::PointerButton::Primary,
                    pressed,
                    modifiers: egui::Modifiers::NONE,
                });
                harness.run();
            }
        }
        let zoom = harness
            .ctx
            .data(|d| d.get_temp::<Camera>(Id::new("minimap")))
            .expect("kept")
            .zoom;
        let asked = asked.lock().expect("unpoisoned").clone();
        (asked, zoom)
    };
    // You at room 3 (8, 0), the view's middle; the barn at (16, 0).
    let barn = Pos2::new(8.0f32.mul_add(ZOOM, 130.0), 100.0);
    let one = MinimapLook::default();
    let none = MinimapLook {
        maps_out: 0,
        ..MinimapLook::default()
    };
    assert_eq!(
        run(with_farm(3), one, Some(barn)).0,
        Some(Clicked::Aim(Some(91)))
    );
    assert_eq!(run(with_farm(3), none, Some(barn)).0, None);
    let inside = MinimapLook {
        inside: 24.0,
        ..MinimapLook::default()
    };
    assert!((run(here(5, None, &[]), inside, None).1 - 24.0).abs() < 1e-3);
}

/// Hovering room 3 shows its card: its title, its number and map, its ways
/// out, and what it is.
#[test]
fn hovering_a_room_shows_its_card() {
    let mut harness = drawn(here(2, None, &[]));
    harness.run();
    let three = Pos2::new(4.0f32.mul_add(ZOOM, 130.0), 100.0);
    harness
        .input_mut()
        .events
        .push(egui::Event::PointerMoved(three));
    harness.run();
    harness.run();
    let _ = harness.get_by_label("[Room 3]");
    let _ = harness.get_by_label("#3 · test town");
    let _ = harness.get_by_label("Healer");
}
