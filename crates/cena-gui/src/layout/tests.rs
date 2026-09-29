use std::collections::HashSet;

use super::*;
use crate::widget::LINE;
use std::path::Path;

fn at(x: f32, y: f32, width: f32, height: f32) -> Rect {
    Rect::from_min_size(pos2(x, y), Vec2::new(width, height))
}

/// The first layout fills the area, no window over another, in the order a
/// player reads them down the right.
#[test]
fn the_first_layout_tiles_the_area() {
    let area = Vec2::new(900.0, 600.0);
    let layout = Layout::with_room_parts(area);
    let titles: Vec<String> = layout
        .holders
        .iter()
        .map(|holder| holder.title().into_owned())
        .collect();
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
    let layout = Layout::with_room_parts(area);
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
    let layout = Layout::with_room_parts(Vec2::new(900.0, 600.0));
    let mut ids = HashSet::new();
    for holder in &layout.holders {
        assert!(ids.insert(holder.id), "window {}", holder.id);
        let placed: Vec<Placed> = match &holder.holds {
            Holds::One(placed) => vec![placed.clone()],
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
        .filter_map(|cell| cell.shown().map(|placed| placed.widget.clone()))
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
    assert_eq!(file(dir, None, "Ashryn"), dir.join("ashryn.json"));
    assert_eq!(
        file(dir, None, "Lord Ashryn:2"),
        dir.join("lordashryn2.json")
    );
}

/// A layout is kept by game and name: the same name on another game is
/// another layout. One kept under the name alone, as layouts were before,
/// is taken as the first, and saving keeps the game's own from then on
/// (`plan/50` §6 item 6).
#[test]
fn a_layout_is_kept_by_game_and_name() {
    let dir = std::env::temp_dir().join(format!("cena-layout-game-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    assert_eq!(
        file(&dir, Some("Prime"), "Nisugi"),
        dir.join("prime_nisugi.json")
    );
    let old = Layout::with_room_parts(Vec2::new(900.0, 600.0));
    old.save(&dir, None, "Nisugi").expect("saved");
    assert_eq!(
        Layout::load(&dir, Some("Prime"), "Nisugi").as_ref(),
        Some(&old),
        "the name's, taken as the first"
    );
    let mut prime = old.clone();
    prime.grid = 16.0;
    prime.save(&dir, Some("Prime"), "Nisugi").expect("saved");
    assert_eq!(
        Layout::load(&dir, Some("Prime"), "Nisugi"),
        Some(prime.clone())
    );
    assert_eq!(
        Layout::load(&dir, Some("Shattered"), "Nisugi"),
        Some(old),
        "another game's is its own"
    );
    assert!(dir.join("prime_nisugi.json").exists() && dir.join("nisugi.json").exists());
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_layout_is_kept_by_name_whatever_its_case() {
    let dir = std::env::temp_dir().join(format!("cena-layout-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let mut layout = Layout::with_room_parts(Vec2::new(900.0, 600.0));
    let room = layout
        .titled("Room")
        .map(|holder| holder.id)
        .expect("a room");
    layout.set(room, at(10.0, 20.0, 300.0, 200.0));
    layout.grid = 16.0;
    layout.save(&dir, None, "Ashryn").expect("saved");
    assert_eq!(Layout::load(&dir, None, "ASHRYN").as_ref(), Some(&layout));
    assert_eq!(Layout::load(&dir, None, "Baelor"), None, "never saved");
    std::fs::write(file(&dir, None, "Lorwyn"), "{ not json").expect("written");
    assert_eq!(
        Layout::load(&dir, None, "Lorwyn"),
        None,
        "unreadable: fitted"
    );
    let earlier = serde_json::to_string(&layout)
        .expect("written")
        .replace("\"version\":2", "\"version\":1");
    std::fs::write(file(&dir, None, "Orsen"), earlier).expect("written");
    assert_eq!(
        Layout::load(&dir, None, "Orsen"),
        None,
        "another version: fitted"
    );
    let _ = std::fs::remove_dir_all(&dir);
}

/// A file the load could not read is kept, not written over: a broken
/// hand edit and a later build's layout both. A second one is refused.
#[test]
fn a_layout_that_cannot_be_read_is_kept_beside_the_one_saved() {
    let dir = std::env::temp_dir().join(format!("cena-layout-kept-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("a folder");
    let layout = Layout::with_room_parts(Vec2::new(900.0, 600.0));
    let path = file(&dir, Some("prime"), "Lorwyn");
    let later = serde_json::to_string(&layout)
        .expect("written")
        .replace("\"version\":2", "\"version\":3");
    assert!(later.contains("\"version\":3"), "the version is 2");
    std::fs::write(&path, &later).expect("written");
    assert_eq!(Layout::load(&dir, Some("prime"), "Lorwyn"), None);

    layout.save(&dir, Some("prime"), "Lorwyn").expect("saved");
    assert_eq!(
        std::fs::read_to_string(super::kept::unread(&path)).ok(),
        Some(later),
        "the later build's layout is still there"
    );
    assert_eq!(
        Layout::load(&dir, Some("prime"), "Lorwyn").as_ref(),
        Some(&layout)
    );
    // One that reads is saved over as ever.
    layout.save(&dir, Some("prime"), "Lorwyn").expect("saved");

    std::fs::write(&path, "{ not json").expect("written");
    assert!(
        layout.save(&dir, Some("prime"), "Lorwyn").is_err(),
        "one is kept already: neither is lost"
    );
    assert_eq!(
        std::fs::read_to_string(&path).ok().as_deref(),
        Some("{ not json")
    );
    let _ = std::fs::remove_dir_all(&dir);
}

/// The Room window of a fitted layout, and the id of its widget `widget`.
fn room_with(layout: &Layout, widget: &Widget) -> (u32, u32) {
    let room = layout.titled("Room").expect("a room");
    let Holds::Custom(custom) = &room.holds else {
        panic!("a custom window");
    };
    let placed = custom
        .cells
        .iter()
        .flat_map(|cell| cell.tabs.iter())
        .find(|placed| placed.widget == *widget)
        .expect("in the room");
    (room.id, placed.id)
}

fn widgets_in(layout: &Layout, title: &str) -> Vec<Widget> {
    match layout.titled(title).map(|holder| &holder.holds) {
        Some(Holds::Custom(custom)) => custom
            .cells
            .iter()
            .flat_map(|cell| cell.tabs.iter().map(|placed| placed.widget.clone()))
            .collect(),
        Some(Holds::One(placed)) => vec![placed.widget.clone()],
        None => Vec::new(),
    }
}

/// A widget let go outside every custom window gets a standalone window of
/// its own there, kept inside the play area; the rest of its custom window
/// stays.
#[test]
fn a_widget_let_go_in_the_open_gets_a_window_of_its_own() {
    let area = Vec2::new(900.0, 600.0);
    let mut layout = Layout::with_room_parts(area);
    let (room, exits) = room_with(&layout, &Widget::Exits);
    layout.release(
        room,
        Taking::Cell(exits),
        pos2(890.0, 300.0),
        &[],
        &Zones::of(&Drawers::default(), area),
    );
    let window = layout.titled("Exits").expect("a window of its own");
    assert!(matches!(&window.holds, Holds::One(placed) if placed.id == exits));
    assert!(window.rect().max.x <= area.x + 0.5, "{:?}", window.rect());
    assert!(!widgets_in(&layout, "Room").contains(&Widget::Exits));
    assert_eq!(widgets_in(&layout, "Room").len(), 4);
}

/// Let go over another custom window's inside, it goes in there, bare, at
/// the pointer; the last widget out of a custom window takes it along.
#[test]
fn a_widget_let_go_on_a_custom_window_joins_it() {
    let area = Vec2::new(900.0, 600.0);
    let mut layout = Layout::with_room_parts(area);
    let fresh = layout.new_custom();
    let inside = Rect::from_min_size(pos2(20.0, 60.0), Vec2::new(288.0, 156.0));
    let (room, exits) = room_with(&layout, &Widget::Exits);
    layout.release(
        room,
        Taking::Tab(exits),
        pos2(50.0, 80.0),
        &[(fresh, inside)],
        &Zones::of(&Drawers::default(), area),
    );
    assert_eq!(widgets_in(&layout, "Custom window"), [Widget::Exits]);
    let Some(Holds::Custom(custom)) = layout.holder(fresh).map(|holder| &holder.holds) else {
        panic!("still a custom window");
    };
    // At the pointer, as far right as the inside lets a widget 260 across go.
    assert_eq!(custom.cells[0].rect().min, pos2(28.0, 20.0));
    layout.release(
        fresh,
        Taking::Cell(exits),
        pos2(890.0, 590.0),
        &[],
        &Zones::of(&Drawers::default(), area),
    );
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
    let mut layout = Layout::with_room_parts(Vec2::new(900.0, 600.0));
    let hunt = layout
        .titled("Hunt")
        .map(|holder| holder.id)
        .expect("a hunt");
    let room = layout
        .titled("Room")
        .map(|holder| holder.id)
        .expect("a room");
    let inside = Rect::from_min_size(pos2(600.0, 400.0), Vec2::new(290.0, 100.0));
    assert!(!layout.join(
        hunt,
        pos2(10.0, 100.0),
        &[(room, inside)],
        &Zones::default()
    ));
    assert!(layout.titled("Hunt").is_some());
    assert!(layout.join(
        hunt,
        pos2(620.0, 420.0),
        &[(room, inside)],
        &Zones::default()
    ));
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
        custom.cells[0].shown().map(|shown| shown.widget.clone()),
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
    let mut layout = Layout::with_room_parts(Vec2::new(900.0, 600.0));
    let hunt = layout
        .titled("Hunt")
        .map(|holder| holder.id)
        .expect("a hunt");
    let hydra = layout.titled("Hydra").map(Holder::rect).expect("hydra");
    let title = hydra.min + Vec2::new(40.0, 10.0);
    assert!(layout.join(hunt, title, &[], &Zones::default()));
    assert!(layout.titled("Hunt").is_none());
    let stacked = layout.titled("Hydra").expect("still there");
    assert_eq!(stacked.rect(), hydra, "where it was");
    let Holds::Custom(custom) = &stacked.holds else {
        panic!("a custom window of one tab stack");
    };
    assert_eq!(custom.cells.len(), 1);
    let tabs: Vec<Widget> = custom.cells[0]
        .tabs
        .iter()
        .map(|tab| tab.widget.clone())
        .collect();
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
        .map(|cell| cell.tabs.iter().map(|tab| tab.widget.clone()).collect())
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
            .map(|cell| cell.tabs.iter().map(|tab| tab.widget.clone()).collect())
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
    let tabs: Vec<Widget> = custom.cells[0]
        .tabs
        .iter()
        .map(|tab| tab.widget.clone())
        .collect();
    assert_eq!(tabs, [Widget::Story, Widget::Hydra]);
}

/// A whole tab stack let go in the open becomes a custom window of that one
/// stack, titled by the widget showing, which still shows.
#[test]
fn a_stack_let_go_in_the_open_keeps_together() {
    let area = Vec2::new(900.0, 600.0);
    let mut layout = Layout::with_room_parts(area);
    let (room, creatures) = room_with(&layout, &Widget::Creatures);
    let (_, objects) = room_with(&layout, &Widget::Objects);
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
    layout.release(
        room,
        Taking::Cell(creatures),
        pos2(300.0, 300.0),
        &[],
        &Zones::of(&Drawers::default(), area),
    );
    let window = layout.titled("Objects").expect("a window of the stack");
    let Holds::Custom(custom) = &window.holds else {
        panic!("a custom window");
    };
    let tabs: Vec<Widget> = custom.cells[0]
        .tabs
        .iter()
        .map(|tab| tab.widget.clone())
        .collect();
    assert_eq!(tabs, [Widget::Creatures, Widget::Objects]);
    assert_eq!(custom.cells[0].showing, 1);
}

/// Widgets added from the list each get a standalone window, set apart
/// from the last; what one follows is kept, and saved with the layout.
#[test]
fn an_added_widget_gets_a_window_and_keeps_whom_it_follows() {
    let dir = std::env::temp_dir().join(format!("cena-layout-follows-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let mut layout = Layout::with_room_parts(Vec2::new(900.0, 600.0));
    let first = layout.add_widget(Widget::Health, Some("Baelor".to_owned()));
    let second = layout.add_widget(Widget::Mana, None);
    let rect_of = |layout: &Layout, placed: u32| {
        layout
            .holders
            .iter()
            .find(|holder| matches!(&holder.holds, Holds::One(one) if one.id == placed))
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
    layout.save(&dir, None, "Ashryn").expect("saved");
    let loaded = Layout::load(&dir, None, "Ashryn").expect("loaded");
    assert_eq!(loaded.follows, layout.follows);
    let _ = std::fs::remove_dir_all(&dir);
}

/// A widget removed takes its standalone window with it, but leaves a
/// custom window standing; either way, whom it followed is forgotten. A
/// window removed takes all its widgets, and theirs.
#[test]
fn removing_forgets_what_followed() {
    let mut layout = Layout::with_room_parts(Vec2::new(900.0, 600.0));
    let (room, exits) = room_with(&layout, &Widget::Exits);
    layout.follow(exits, Some("Baelor".to_owned()));
    layout.remove_widget(room, exits);
    assert!(!widgets_in(&layout, "Room").contains(&Widget::Exits));
    assert!(layout.follows.is_empty());
    let added = layout.add_widget(Widget::Spirit, Some("Baelor".to_owned()));
    let window = layout
        .holders
        .iter()
        .find(|holder| matches!(&holder.holds, Holds::One(one) if one.id == added))
        .map(|holder| holder.id)
        .expect("its window");
    layout.remove_widget(window, added);
    assert!(layout.holder(window).is_none(), "its window went with it");
    let (_, creatures) = room_with(&layout, &Widget::Creatures);
    layout.follow(creatures, Some("Baelor".to_owned()));
    layout.remove_window(room);
    assert!(layout.titled("Room").is_none());
    assert!(layout.follows.is_empty());
}

/// A custom window removed takes what its widgets kept by their ids: a
/// bar's look, a Room widget's parts, and how a story draws its lines.
#[test]
fn a_window_removed_takes_its_widgets_looks_and_parts() {
    let mut layout = Layout::fitted(Vec2::new(900.0, 600.0));
    let window = layout.custom(
        Rect::from_min_size(pos2(0.0, 0.0), Vec2::splat(300.0)),
        "Around",
        &[&[Widget::Room], &[Widget::Health], &[Widget::Story]],
    );
    let Some(Holds::Custom(custom)) = layout.holder(window).map(|holder| &holder.holds) else {
        panic!("a custom window");
    };
    let placed: Vec<Placed> = custom
        .cells
        .iter()
        .flat_map(|cell| cell.tabs.iter().cloned())
        .collect();
    for one in placed {
        if let Some(look) = one.widget.bar_look() {
            layout.looks.insert(one.id, look);
        } else if one.widget == Widget::Story {
            let unwrapped = crate::widget::Lines {
                wrap: false,
                ..crate::widget::Lines::default()
            };
            layout.lines.insert(one.id, unwrapped);
        } else {
            let apart = crate::widget::RoomParts {
                apart: true,
                ..crate::widget::RoomParts::default()
            };
            layout.rooms.insert(one.id, apart);
        }
    }
    assert_eq!(
        (layout.rooms.len(), layout.looks.len(), layout.lines.len()),
        (1, 1, 1)
    );
    layout.remove_window(window);
    assert!(layout.rooms.is_empty(), "its parts");
    assert!(layout.looks.is_empty(), "its look");
    assert!(layout.lines.is_empty(), "its lines");
}

/// A tab added beside a widget following another character follows them
/// too, and the story is not offered there; beside a widget not in the
/// window, nothing is added.
#[test]
fn a_tab_beside_another_characters_widget_follows_them() {
    let mut layout = Layout::fitted(Vec2::new(900.0, 600.0));
    let thoughts = layout.add_widget(
        Widget::Stream("thoughts".to_owned()),
        Some("Baelor".to_owned()),
    );
    let window = layout
        .holders
        .iter()
        .find(|holder| matches!(&holder.holds, Holds::One(one) if one.id == thoughts))
        .map(|holder| holder.id)
        .expect("its window");
    let speech = layout
        .add_tab(window, thoughts, Widget::Stream("speech".to_owned()))
        .expect("added");
    assert_eq!(
        layout.follows.get(&speech).map(String::as_str),
        Some("Baelor")
    );
    assert_eq!(
        layout.add_tab(window, 999, Widget::Hydra),
        None,
        "no such widget"
    );
}

/// A custom window takes a new title, trimmed; following is undone by
/// following the window's own again.
#[test]
fn a_custom_window_is_renamed_and_a_widget_unfollowed() {
    let mut layout = Layout::with_room_parts(Vec2::new(900.0, 600.0));
    let vitals = layout
        .titled("Vitals")
        .map(|holder| holder.id)
        .expect("vitals");
    layout.rename(vitals, "  Bars ");
    assert!(layout.titled("Bars").is_some());
    let (_, exits) = room_with(&layout, &Widget::Exits);
    layout.follow(exits, Some("Baelor".to_owned()));
    layout.follow(exits, None);
    assert!(layout.follows.is_empty());
}

/// A bar's look is kept with the layout and read back as it was.
#[test]
fn a_bars_look_is_kept_with_the_layout() {
    use crate::bar::{Fills, Look, Place, Says};
    let dir = std::env::temp_dir().join(format!("cena-layout-look-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let mut layout = Layout::with_room_parts(Vec2::new(900.0, 600.0));
    let look = Look {
        fills: Fills::Up,
        place: Place::Below,
        says: Says {
            label: true,
            numbers: false,
            percent: true,
        },
        color: [0x10, 0x20, 0x30],
        overlay: Some("C:/overlays/gloss.png".to_owned()),
        background: Some("C:/overlays/glass.png".to_owned()),
        fill_image: None,
        ring: 40,
    };
    layout.looks.insert(7, look.clone());
    let lines = crate::widget::Lines {
        stamps: crate::widget::Stamps::End,
        seconds: true,
        hours: crate::story::Hours::TwentyFour,
        ..crate::widget::Lines::default()
    };
    layout.lines.insert(8, lines);
    layout.save(&dir, None, "Ashryn").expect("saved");
    let read = Layout::load(&dir, None, "Ashryn").expect("read back");
    assert_eq!(read.looks.get(&7), Some(&look));
    assert_eq!(read.lines.get(&8), Some(&lines), "a story's times too");
    let _ = std::fs::remove_dir_all(&dir);
}

/// A look saved before orbs, rings and their images reads as it was, a
/// ring's thickness its default: a look that failed to read would lose the
/// whole layout to a fitted one.
#[test]
fn a_look_saved_before_orbs_reads_as_it_was() {
    use crate::bar::{Fills, Look};
    let saved = r#"{"fills":"up","place":"inside","says":{"label":true,"numbers":false,"percent":true},"color":[71,132,217]}"#;
    let look: Look = serde_json::from_str(saved).expect("read");
    assert_eq!(look.fills, Fills::Up);
    assert_eq!(
        (look.ring, look.background, look.fill_image),
        (Look::RING, None, None)
    );
}

/// The first layout's Room window is the one Room widget, the room as the
/// game describes it (the author, 2026-09-28), not the room in its parts.
#[test]
fn the_first_layouts_room_is_one_widget() {
    let layout = Layout::fitted(Vec2::new(900.0, 600.0));
    match layout.titled("Room").map(|holder| &holder.holds) {
        Some(Holds::One(placed)) => assert_eq!(placed.widget, Widget::Room),
        other => panic!("{other:?}"),
    }
}
