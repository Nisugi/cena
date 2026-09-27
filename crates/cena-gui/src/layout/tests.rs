use std::collections::HashSet;

use super::*;
use crate::widget::LINE;

fn at(x: f32, y: f32, width: f32, height: f32) -> Rect {
    Rect::from_min_size(pos2(x, y), Vec2::new(width, height))
}

/// The first layout fills the area, no window over another, in the order a
/// player reads them down the right.
#[test]
fn the_first_layout_tiles_the_area() {
    let area = Vec2::new(900.0, 600.0);
    let layout = Layout::fitted(area);
    let titles: Vec<&str> = layout.holders.iter().map(Holder::title).collect();
    assert_eq!(
        titles,
        ["Story", "Vitals", "Loadout", "Hunt", "Room", "Hydra"]
    );
    let rects: Vec<Rect> = layout.holders.iter().map(Holder::rect).collect();
    let covered: f32 = rects.iter().map(|r| r.width() * r.height()).sum();
    assert!((covered - area.x * area.y).abs() < 1.0, "{covered}");
    for (i, a) in rects.iter().enumerate() {
        for b in rects.iter().skip(i + 1) {
            assert!(a.intersect(*b).area() < 0.5, "{a:?} over {b:?}");
        }
    }
    let story = layout.titled("Story").map(Holder::rect).expect("a story");
    assert!((story.width() % GRID).abs() < 0.01, "on the grid");
}

/// In a short area the windows as tall as what they hold give way, so the
/// room and Hydra's messages are never less than a window can be, and none
/// runs past the bottom.
#[test]
fn a_short_area_keeps_the_room_and_hydra() {
    let area = Vec2::new(900.0, 400.0);
    let layout = Layout::fitted(area);
    for title in ["Room", "Hydra"] {
        let rect = layout.titled(title).map(Holder::rect).expect(title);
        assert!(rect.height() >= SMALLEST.y, "{title}: {rect:?}");
    }
    for holder in &layout.holders {
        assert!(holder.rect().max.y <= area.y + 0.5, "{}", holder.title());
    }
}

/// Every window and every widget placed has an id of its own (`plan/28`
/// §7c), and the vitals are four widgets, a bar each (`plan/49` §1 row 1).
#[test]
fn every_window_and_widget_has_an_id_of_its_own() {
    let layout = Layout::fitted(Vec2::new(900.0, 600.0));
    let mut ids = HashSet::new();
    for holder in &layout.holders {
        assert!(ids.insert(holder.id), "window {}", holder.id);
        let placed: Vec<Placed> = match &holder.holds {
            Holds::One(placed) => vec![*placed],
            Holds::Custom(custom) => custom
                .cells
                .iter()
                .flat_map(|cell| cell.tabs.clone())
                .collect(),
        };
        for placed in placed {
            assert!(ids.insert(placed.id), "widget {}", placed.id);
        }
    }
    let Some(Holds::Custom(vitals)) = layout.titled("Vitals").map(|holder| &holder.holds) else {
        panic!("a custom window of vitals");
    };
    let shown: Vec<Widget> = vitals
        .cells
        .iter()
        .filter_map(|cell| cell.shown().map(|placed| placed.widget))
        .collect();
    assert_eq!(
        shown,
        [
            Widget::Health,
            Widget::Mana,
            Widget::Stamina,
            Widget::Spirit
        ]
    );
}

/// A row's widgets share its width; each row is as tall as it asks; the
/// last reaches the bottom.
#[test]
fn a_custom_window_lays_its_rows() {
    let placed = |id, widget| Placed { id, widget };
    let custom = Custom::rows(
        "Loadout",
        vec![
            vec![placed(1, Widget::RightHand)],
            vec![placed(2, Widget::Roundtime), placed(3, Widget::CastTime)],
        ],
        Vec2::new(200.0, 70.0),
    );
    let rects: Vec<Rect> = custom.cells.iter().map(Cell::rect).collect();
    assert_eq!(
        rects,
        [
            at(0.0, 0.0, 200.0, LINE),
            at(0.0, LINE, 100.0, 70.0 - LINE),
            at(100.0, LINE, 100.0, 70.0 - LINE),
        ]
    );
}

/// Rows that ask more than the inside has: the lines keep a line each, and
/// the lists share the rest.
#[test]
fn rows_that_ask_too_much_share_what_is_left() {
    let rows = [Widget::RoomTitle, Widget::Creatures, Widget::Objects]
        .into_iter()
        .zip(1..)
        .map(|(widget, id)| vec![Placed { id, widget }])
        .collect();
    let custom = Custom::rows("Room", rows, Vec2::new(200.0, 60.0));
    let heights: Vec<f32> = custom
        .cells
        .iter()
        .map(|cell| cell.rect().height())
        .collect();
    assert_eq!(heights, [LINE, 20.0, 20.0]);
}

/// Resized, a custom window's cells scale across, the bottom row keeps to
/// the bottom, and a line above it stays a line.
#[test]
fn a_resized_custom_window_keeps_its_cells_to_it() {
    let placed = |id, widget| Placed { id, widget };
    let mut custom = Custom::rows(
        "Loadout",
        vec![
            vec![placed(1, Widget::RightHand)],
            vec![placed(2, Widget::Roundtime), placed(3, Widget::CastTime)],
        ],
        Vec2::new(200.0, 60.0),
    );
    assert!(
        !custom.fit(Vec2::new(200.0, 60.0)),
        "the same size moves nothing"
    );
    assert!(custom.fit(Vec2::new(300.0, 100.0)));
    let rects: Vec<Rect> = custom.cells.iter().map(Cell::rect).collect();
    assert_eq!(
        rects,
        [
            at(0.0, 0.0, 300.0, LINE),
            at(0.0, LINE, 150.0, 100.0 - LINE),
            at(150.0, LINE, 150.0, 100.0 - LINE),
        ]
    );
    assert!(custom.fit(Vec2::new(30.0, 10.0)));
    for cell in &custom.cells {
        let rect = cell.rect();
        assert!(rect.width() >= 20.0 && rect.height() >= LINE, "{rect:?}");
    }
}

/// One file per character on every filesystem: its name in lower case,
/// letters and digits only -- asserted here rather than through a load,
/// which Windows' case-blind files would pass either way.
#[test]
fn a_characters_file_is_its_name_in_lower_case() {
    let dir = Path::new("layouts");
    assert_eq!(file(dir, "Ashryn"), dir.join("ashryn.json"));
    assert_eq!(file(dir, "Lord Ashryn:2"), dir.join("lordashryn2.json"));
}

#[test]
fn a_layout_is_kept_by_name_whatever_its_case() {
    let dir = std::env::temp_dir().join(format!("cena-layout-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let mut layout = Layout::fitted(Vec2::new(900.0, 600.0));
    let room = layout
        .titled("Room")
        .map(|holder| holder.id)
        .expect("a room");
    layout.set(room, at(10.0, 20.0, 300.0, 200.0));
    layout.grid = 16.0;
    layout.save(&dir, "Ashryn").expect("saved");
    assert_eq!(Layout::load(&dir, "ASHRYN").as_ref(), Some(&layout));
    assert_eq!(Layout::load(&dir, "Baelor"), None, "never saved");
    std::fs::write(file(&dir, "Lorwyn"), "{ not json").expect("written");
    assert_eq!(Layout::load(&dir, "Lorwyn"), None, "unreadable: fitted");
    let earlier = serde_json::to_string(&layout)
        .expect("written")
        .replace("\"version\":2", "\"version\":1");
    std::fs::write(file(&dir, "Orsen"), earlier).expect("written");
    assert_eq!(Layout::load(&dir, "Orsen"), None, "another version: fitted");
    let _ = std::fs::remove_dir_all(&dir);
}

/// The Room window of a fitted layout, and the id of its widget `widget`.
fn room_with(layout: &Layout, widget: Widget) -> (u32, u32) {
    let room = layout.titled("Room").expect("a room");
    let Holds::Custom(custom) = &room.holds else {
        panic!("a custom window");
    };
    let placed = custom
        .cells
        .iter()
        .flat_map(|cell| cell.tabs.iter())
        .find(|placed| placed.widget == widget)
        .expect("in the room");
    (room.id, placed.id)
}

fn widgets_in(layout: &Layout, title: &str) -> Vec<Widget> {
    match layout.titled(title).map(|holder| &holder.holds) {
        Some(Holds::Custom(custom)) => custom
            .cells
            .iter()
            .flat_map(|cell| cell.tabs.iter().map(|placed| placed.widget))
            .collect(),
        Some(Holds::One(placed)) => vec![placed.widget],
        None => Vec::new(),
    }
}

/// A widget let go outside every custom window gets a standalone window of
/// its own there, kept inside the play area; the rest of its custom window
/// stays.
#[test]
fn a_widget_let_go_in_the_open_gets_a_window_of_its_own() {
    let area = Vec2::new(900.0, 600.0);
    let mut layout = Layout::fitted(area);
    let (room, exits) = room_with(&layout, Widget::Exits);
    layout.release(room, exits, pos2(890.0, 300.0), &[], area);
    let window = layout.titled("Exits").expect("a window of its own");
    assert!(matches!(window.holds, Holds::One(placed) if placed.id == exits));
    assert!(window.rect().max.x <= area.x + 0.5, "{:?}", window.rect());
    assert!(!widgets_in(&layout, "Room").contains(&Widget::Exits));
    assert_eq!(widgets_in(&layout, "Room").len(), 4);
}

/// Let go over another custom window's inside, it goes in there, bare, at
/// the pointer; the last widget out of a custom window takes it along.
#[test]
fn a_widget_let_go_on_a_custom_window_joins_it() {
    let area = Vec2::new(900.0, 600.0);
    let mut layout = Layout::fitted(area);
    let fresh = layout.new_custom();
    let inside = Rect::from_min_size(pos2(20.0, 60.0), Vec2::new(288.0, 156.0));
    let (room, exits) = room_with(&layout, Widget::Exits);
    layout.release(room, exits, pos2(50.0, 80.0), &[(fresh, inside)], area);
    assert_eq!(widgets_in(&layout, "Custom window"), [Widget::Exits]);
    let Some(Holds::Custom(custom)) = layout.holder(fresh).map(|holder| &holder.holds) else {
        panic!("still a custom window");
    };
    // At the pointer, as far right as the inside lets a widget 260 across go.
    assert_eq!(custom.cells[0].rect().min, pos2(28.0, 20.0));
    layout.release(fresh, exits, pos2(890.0, 590.0), &[], area);
    assert!(
        layout.holder(fresh).is_none(),
        "its last widget took it along"
    );
    assert!(layout.titled("Exits").is_some());
}

/// A standalone window dropped with the pointer on a custom window's inside
/// joins it and goes; dropped elsewhere, nothing changes.
#[test]
fn a_standalone_window_joins_the_custom_window_it_is_dropped_on() {
    let mut layout = Layout::fitted(Vec2::new(900.0, 600.0));
    let hunt = layout
        .titled("Hunt")
        .map(|holder| holder.id)
        .expect("a hunt");
    let room = layout
        .titled("Room")
        .map(|holder| holder.id)
        .expect("a room");
    let inside = Rect::from_min_size(pos2(600.0, 400.0), Vec2::new(290.0, 100.0));
    assert!(!layout.join(hunt, pos2(10.0, 10.0), &[(room, inside)]));
    assert!(layout.titled("Hunt").is_some());
    assert!(layout.join(hunt, pos2(620.0, 420.0), &[(room, inside)]));
    assert!(layout.titled("Hunt").is_none());
    assert!(widgets_in(&layout, "Room").contains(&Widget::Hunt));
}

/// A widget taken out of a tab stack leaves its other tabs in the cell, the
/// one showing still one there.
#[test]
fn a_tab_taken_out_leaves_the_rest_of_its_stack() {
    let placed = |id, widget| Placed { id, widget };
    let mut custom = Custom::empty("Streams", Vec2::new(200.0, 100.0));
    custom.put(placed(1, Widget::Story), pos2(0.0, 0.0));
    custom.cells[0].tabs.push(placed(2, Widget::Hydra));
    custom.cells[0].showing = 1;
    assert_eq!(
        custom.take(2).map(|taken| taken.widget),
        Some(Widget::Hydra)
    );
    assert_eq!(custom.cells.len(), 1);
    assert_eq!(
        custom.cells[0].shown().map(|shown| shown.widget),
        Some(Widget::Story)
    );
    assert_eq!(
        custom.cells[0].showing, 0,
        "the tab showing is one still there"
    );
    assert_eq!(custom.take(9), None);
}
