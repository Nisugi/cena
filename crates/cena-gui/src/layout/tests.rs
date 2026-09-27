use std::collections::HashSet;

use super::custom::Cell;
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
