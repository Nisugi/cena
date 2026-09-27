use std::collections::HashSet;

use super::*;
use crate::layout::{Library, Preset};
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
    layout.release(room, Taking::Cell(exits), pos2(890.0, 300.0), &[], area);
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
    layout.release(
        room,
        Taking::Tab(exits),
        pos2(50.0, 80.0),
        &[(fresh, inside)],
        area,
    );
    assert_eq!(widgets_in(&layout, "Custom window"), [Widget::Exits]);
    let Some(Holds::Custom(custom)) = layout.holder(fresh).map(|holder| &holder.holds) else {
        panic!("still a custom window");
    };
    // At the pointer, as far right as the inside lets a widget 260 across go.
    assert_eq!(custom.cells[0].rect().min, pos2(28.0, 20.0));
    layout.release(fresh, Taking::Cell(exits), pos2(890.0, 590.0), &[], area);
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
    assert!(!layout.join(hunt, pos2(10.0, 100.0), &[(room, inside)]));
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
    custom.land(vec![placed(1, Widget::Story)], 0, pos2(0.0, 0.0));
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

/// A standalone window dropped on another's title bar: the two become one
/// window of one tab stack, where the other was, the first tab showing.
#[test]
fn a_window_dropped_on_anothers_title_stacks_with_it() {
    let mut layout = Layout::fitted(Vec2::new(900.0, 600.0));
    let hunt = layout
        .titled("Hunt")
        .map(|holder| holder.id)
        .expect("a hunt");
    let hydra = layout.titled("Hydra").map(Holder::rect).expect("hydra");
    let title = hydra.min + Vec2::new(40.0, 10.0);
    assert!(layout.join(hunt, title, &[]));
    assert!(layout.titled("Hunt").is_none());
    let stacked = layout.titled("Hydra").expect("still there");
    assert_eq!(stacked.rect(), hydra, "where it was");
    let Holds::Custom(custom) = &stacked.holds else {
        panic!("a custom window of one tab stack");
    };
    assert_eq!(custom.cells.len(), 1);
    let tabs: Vec<Widget> = custom.cells[0].tabs.iter().map(|tab| tab.widget).collect();
    assert_eq!(tabs, [Widget::Hydra, Widget::Hunt]);
    assert_eq!(custom.cells[0].showing, 0);
}

/// Let go over the middle of a widget, something joins its tab stack; over
/// its side, it takes a place of its own there.
#[test]
fn only_a_widgets_middle_stacks() {
    let placed = |id, widget| Placed { id, widget };
    let mut custom = Custom::empty("Streams", Vec2::new(400.0, 200.0));
    custom.land(vec![placed(1, Widget::Story)], 0, pos2(0.0, 0.0));
    let story = custom.cells[0].rect();
    custom.land(vec![placed(2, Widget::Hydra)], 0, story.center());
    assert_eq!(custom.cells.len(), 1, "stacked");
    custom.land(
        vec![placed(3, Widget::Hunt)],
        0,
        pos2(story.min.x + 5.0, story.center().y),
    );
    assert_eq!(custom.cells.len(), 2, "beside, not stacked");
    let widgets: Vec<Vec<Widget>> = custom
        .cells
        .iter()
        .map(|cell| cell.tabs.iter().map(|tab| tab.widget).collect())
        .collect();
    assert_eq!(
        widgets,
        [vec![Widget::Story, Widget::Hydra], vec![Widget::Hunt]]
    );
}

/// A tab or a whole cell stacked onto another cell's widget: the tab alone
/// moves, the cell with all its tabs; onto its own cell, nothing moves.
#[test]
fn stacking_onto_a_cell_takes_what_was_dragged() {
    let placed = |id, widget| Placed { id, widget };
    let mut custom = Custom::empty("Streams", Vec2::new(600.0, 400.0));
    custom.land(
        vec![placed(1, Widget::Story), placed(2, Widget::Hydra)],
        0,
        pos2(0.0, 0.0),
    );
    custom.land(vec![placed(3, Widget::Hunt)], 0, pos2(0.0, 350.0));
    custom.stack_onto(Taking::Tab(2), 1);
    assert_eq!(
        custom.cells[0].tabs.len(),
        2,
        "onto its own cell: nothing moved"
    );
    custom.stack_onto(Taking::Tab(2), 3);
    let widgets = |custom: &Custom| -> Vec<Vec<Widget>> {
        custom
            .cells
            .iter()
            .map(|cell| cell.tabs.iter().map(|tab| tab.widget).collect())
            .collect()
    };
    assert_eq!(
        widgets(&custom),
        [vec![Widget::Story], vec![Widget::Hunt, Widget::Hydra]]
    );
    custom.stack_onto(Taking::Cell(3), 1);
    assert_eq!(
        widgets(&custom),
        [vec![Widget::Story, Widget::Hunt, Widget::Hydra]]
    );
}

/// Stacking a cell onto a widget of its own cell moves nothing, and loses
/// nothing.
#[test]
fn a_cell_stacked_onto_itself_stays() {
    let placed = |id, widget| Placed { id, widget };
    let mut custom = Custom::empty("Streams", Vec2::new(400.0, 200.0));
    custom.land(
        vec![placed(1, Widget::Story), placed(2, Widget::Hydra)],
        0,
        pos2(0.0, 0.0),
    );
    custom.stack_onto(Taking::Cell(1), 2);
    custom.stack_onto(Taking::Tab(2), 1);
    let tabs: Vec<Widget> = custom.cells[0].tabs.iter().map(|tab| tab.widget).collect();
    assert_eq!(tabs, [Widget::Story, Widget::Hydra]);
}

/// A whole tab stack let go in the open becomes a custom window of that one
/// stack, titled by the widget showing, which still shows.
#[test]
fn a_stack_let_go_in_the_open_keeps_together() {
    let area = Vec2::new(900.0, 600.0);
    let mut layout = Layout::fitted(area);
    let (room, creatures) = room_with(&layout, Widget::Creatures);
    let (_, objects) = room_with(&layout, Widget::Objects);
    if let Some(Holder {
        holds: Holds::Custom(custom),
        ..
    }) = layout.holders.iter_mut().find(|holder| holder.id == room)
    {
        custom.stack_onto(Taking::Cell(objects), creatures);
        let stacked = custom
            .cells
            .iter_mut()
            .find(|cell| cell.tabs.len() == 2)
            .expect("a stack");
        stacked.showing = 1;
    }
    layout.release(room, Taking::Cell(creatures), pos2(300.0, 300.0), &[], area);
    let window = layout.titled("Objects").expect("a window of the stack");
    let Holds::Custom(custom) = &window.holds else {
        panic!("a custom window");
    };
    let tabs: Vec<Widget> = custom.cells[0].tabs.iter().map(|tab| tab.widget).collect();
    assert_eq!(tabs, [Widget::Creatures, Widget::Objects]);
    assert_eq!(custom.cells[0].showing, 1);
}

/// Widgets added from the list each get a standalone window, set apart
/// from the last; what one follows is kept, and saved with the layout.
#[test]
fn an_added_widget_gets_a_window_and_keeps_whom_it_follows() {
    let dir = std::env::temp_dir().join(format!("cena-layout-follows-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let mut layout = Layout::fitted(Vec2::new(900.0, 600.0));
    let first = layout.add_widget(Widget::Health, Some("Baelor".to_owned()));
    let second = layout.add_widget(Widget::Mana, None);
    let rect_of = |layout: &Layout, placed: u32| {
        layout
            .holders
            .iter()
            .find(|holder| matches!(holder.holds, Holds::One(one) if one.id == placed))
            .map(Holder::rect)
    };
    let (a, b) = (
        rect_of(&layout, first).expect("a"),
        rect_of(&layout, second).expect("b"),
    );
    assert_ne!(a.min, b.min, "set apart");
    assert_eq!(
        layout.follows.get(&first).map(String::as_str),
        Some("Baelor")
    );
    assert!(!layout.follows.contains_key(&second));
    layout.save(&dir, "Ashryn").expect("saved");
    let loaded = Layout::load(&dir, "Ashryn").expect("loaded");
    assert_eq!(loaded.follows, layout.follows);
    let _ = std::fs::remove_dir_all(&dir);
}

/// A widget removed takes its standalone window with it, but leaves a
/// custom window standing; either way, whom it followed is forgotten. A
/// window removed takes all its widgets, and theirs.
#[test]
fn removing_forgets_what_followed() {
    let mut layout = Layout::fitted(Vec2::new(900.0, 600.0));
    let (room, exits) = room_with(&layout, Widget::Exits);
    layout.follow(exits, Some("Baelor".to_owned()));
    layout.remove_widget(room, exits);
    assert!(!widgets_in(&layout, "Room").contains(&Widget::Exits));
    assert!(layout.follows.is_empty());
    let added = layout.add_widget(Widget::Spirit, Some("Baelor".to_owned()));
    let window = layout
        .holders
        .iter()
        .find(|holder| matches!(holder.holds, Holds::One(one) if one.id == added))
        .map(|holder| holder.id)
        .expect("its window");
    layout.remove_widget(window, added);
    assert!(layout.holder(window).is_none(), "its window went with it");
    let (_, creatures) = room_with(&layout, Widget::Creatures);
    layout.follow(creatures, Some("Baelor".to_owned()));
    layout.remove_window(room);
    assert!(layout.titled("Room").is_none());
    assert!(layout.follows.is_empty());
}

/// A custom window takes a new title, trimmed; following is undone by
/// following the window's own again.
#[test]
fn a_custom_window_is_renamed_and_a_widget_unfollowed() {
    let mut layout = Layout::fitted(Vec2::new(900.0, 600.0));
    let vitals = layout
        .titled("Vitals")
        .map(|holder| holder.id)
        .expect("vitals");
    layout.rename(vitals, "  Bars ");
    assert!(layout.titled("Bars").is_some());
    let (_, exits) = room_with(&layout, Widget::Exits);
    layout.follow(exits, Some("Baelor".to_owned()));
    layout.follow(exits, None);
    assert!(layout.follows.is_empty());
}

/// The widgets of a custom window, in its cells' order.
fn in_custom(custom: &Custom) -> Vec<Widget> {
    custom
        .cells
        .iter()
        .flat_map(|cell| cell.tabs.iter().map(|tab| tab.widget))
        .collect()
}

/// Hydra's presets: each named once, the vitals row four bars side by side.
#[test]
fn hydras_presets_are_put_together() {
    let presets = Preset::hydras();
    let names: HashSet<&str> = presets.iter().map(|preset| preset.name.as_str()).collect();
    assert_eq!(names.len(), presets.len());
    let mut layout = Layout::fitted(Vec2::new(900.0, 600.0));
    let row = presets
        .iter()
        .find(|preset| preset.name == "Vitals row")
        .expect("a vitals row");
    let placed = layout.add_preset(row, None);
    let Some(Holds::Custom(custom)) = layout.holder(placed).map(|holder| &holder.holds) else {
        panic!("a custom window");
    };
    assert_eq!(
        in_custom(custom),
        [
            Widget::Health,
            Widget::Mana,
            Widget::Stamina,
            Widget::Spirit
        ]
    );
    let tops: HashSet<u32> = custom
        .cells
        .iter()
        .map(|cell| cell.rect().min.y.to_bits())
        .collect();
    assert_eq!(tops.len(), 1, "one row");
}

/// Placing a preset places a copy: its widgets get ids of their own, each
/// copy apart, and saving over the preset later changes no copy.
#[test]
fn a_preset_placed_is_a_copy() {
    let mut layout = Layout::fitted(Vec2::new(900.0, 600.0));
    let vitals = Preset::hydras().remove(0);
    let first = layout.add_preset(&vitals, None);
    let second = layout.add_preset(&vitals, None);
    assert_ne!(first, second);
    let mut ids = HashSet::new();
    for holder in &layout.holders {
        let placed: Vec<u32> = match &holder.holds {
            Holds::One(placed) => vec![placed.id],
            Holds::Custom(custom) => custom
                .cells
                .iter()
                .flat_map(|cell| cell.tabs.iter().map(|tab| tab.id))
                .collect(),
        };
        for id in placed {
            assert!(ids.insert(id), "widget {id} twice");
        }
        assert!(ids.insert(holder.id), "window {}", holder.id);
    }
    let mut library = Library::load(None);
    library.keep(vitals.clone());
    let Some(Holds::Custom(custom)) = layout.holder(first).map(|holder| &holder.holds) else {
        panic!("a custom window");
    };
    library.keep(Preset::of(
        "Vitals",
        &Custom::empty("Vitals", Vec2::new(10.0, 10.0)),
    ));
    let Some(Holds::Custom(after)) = layout.holder(first).map(|holder| &holder.holds) else {
        panic!("still a custom window");
    };
    assert_eq!(after, custom, "the copy placed is untouched");
    assert_eq!(library.presets().len(), 1, "kept over the same name");
}

/// A preset placed for another character: each widget follows that one,
/// but a story, which stays its own window's.
#[test]
fn a_preset_placed_for_another_follows_but_its_story() {
    let mut layout = Layout::fitted(Vec2::new(900.0, 600.0));
    let placed = |id, widget| Placed { id, widget };
    let custom = Custom::rows(
        "Mixed",
        vec![
            vec![placed(0, Widget::Story)],
            vec![placed(0, Widget::Health)],
        ],
        Vec2::new(200.0, 200.0),
    );
    let window = layout.add_preset(&Preset::of("Mixed", &custom), Some("Baelor"));
    let Some(Holds::Custom(custom)) = layout.holder(window).map(|holder| &holder.holds) else {
        panic!("a custom window");
    };
    for tab in custom.cells.iter().flat_map(|cell| cell.tabs.iter()) {
        let follows = layout.follows.get(&tab.id).map(String::as_str);
        match tab.widget {
            Widget::Story => assert_eq!(follows, None),
            _ => assert_eq!(follows, Some("Baelor")),
        }
    }
}

/// The library is kept in its file: what is kept is there when it is read
/// again, one of the same name kept over, a forgotten one gone; no file, or
/// one of another version, is an empty library.
#[test]
fn the_library_is_kept_in_its_file() {
    let dir = std::env::temp_dir().join(format!("cena-presets-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    assert!(
        Library::load(Some(dir.clone())).presets().is_empty(),
        "no file"
    );
    let mut library = Library::load(Some(dir.clone()));
    let mut hydras = Preset::hydras().into_iter();
    let (Some(vitals), Some(row)) = (hydras.next(), hydras.next()) else {
        panic!("Hydra's presets");
    };
    library.keep(vitals);
    library.keep(row);
    assert_eq!(
        Library::load(Some(dir.clone())).presets(),
        library.presets()
    );
    library.forget("Vitals");
    let read = Library::load(Some(dir.clone()));
    let names: Vec<&str> = read
        .presets()
        .iter()
        .map(|preset| preset.name.as_str())
        .collect();
    assert_eq!(names, ["Vitals row"]);
    let file = dir.join("presets.json");
    let text = std::fs::read_to_string(&file).expect("written");
    std::fs::write(&file, text.replace("\"version\": 1", "\"version\": 2")).expect("written");
    assert!(
        Library::load(Some(dir.clone())).presets().is_empty(),
        "another version"
    );
    let _ = std::fs::remove_dir_all(&dir);
}
