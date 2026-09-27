//! Tab stacks (`plan/49` Stage A step 5), as a player uses and makes them.

use super::*;
use crate::layout::Holds;
use crate::widget::Widget;
use egui::accesskit::Role;

/// Press at `from`, move to `to` frame by frame, and let go.
fn drag(harness: &mut Harness<'_, Scene>, from: egui::Pos2, to: egui::Pos2) {
    harness.hover_at(from);
    harness.step();
    harness.drag_at(from);
    harness.step();
    for step in 1..=4u8 {
        harness.hover_at(from + (to - from) * (f32::from(step) / 4.0));
        harness.step();
    }
    harness.drop_at(to);
    harness.step();
    harness.step();
}

/// The tab stacks in the window titled `title`: each cell's widgets.
fn stacks(harness: &Harness<'_, Scene>, title: &str) -> Vec<Vec<Widget>> {
    let layout = harness.state().play.layout.as_ref().expect("laid out");
    match layout.titled(title).map(|holder| &holder.holds) {
        Some(Holds::Custom(custom)) => custom
            .cells
            .iter()
            .map(|cell| cell.tabs.iter().map(|tab| tab.widget).collect())
            .collect(),
        Some(Holds::One(placed)) => vec![vec![placed.widget]],
        None => Vec::new(),
    }
}

/// A play window whose Hydra window has Hunt stacked on it, as a drop on
/// its title bar makes it.
fn stacked<'a>() -> Harness<'a, Scene> {
    let mut harness = harness();
    harness.run();
    if let Some(layout) = harness.state_mut().play.layout.as_mut() {
        let hunt = layout
            .titled("Hunt")
            .map(|holder| holder.id)
            .expect("a hunt");
        let hydra = layout
            .titled("Hydra")
            .map(crate::layout::Holder::rect)
            .expect("hydra");
        assert!(layout.join(hunt, hydra.min + egui::vec2(40.0, 10.0), &[]));
    }
    harness.run();
    harness.step();
    harness
}

/// A tab stack shows one widget, a tab for each, and a click on a tab
/// shows its widget instead.
#[test]
fn a_tab_stack_shows_one_widget_and_a_click_switches() {
    let mut harness = stacked();
    assert_eq!(
        stacks(&harness, "Hydra"),
        [vec![Widget::Hydra, Widget::Hunt]]
    );
    assert!(
        harness
            .query_by_label("Hunt: resting until mana is 50%.")
            .is_some()
    );
    assert!(
        harness.query_by_label("ojandhaart").is_none(),
        "Hunt not showing"
    );
    harness.get_by_role_and_label(Role::Button, "Hunt").click();
    harness.run();
    assert!(harness.query_by_label("ojandhaart").is_some());
    assert!(
        harness
            .query_by_label("Hunt: resting until mana is 50%.")
            .is_none()
    );
}

/// A tab not showing counts what its widget has said since it last showed,
/// and showing it reads it all.
#[test]
fn a_tab_not_showing_counts_what_came() {
    let mut harness = stacked();
    harness.get_by_role_and_label(Role::Button, "Hunt").click();
    harness.run();
    for said in ["Loot: a gold ring.", "Heal: nothing to heal."] {
        harness.state_mut().story.tell(cena_session::Notice::line(
            cena_session::NoticeKind::Info,
            said,
        ));
    }
    harness.run();
    assert!(
        harness
            .query_by_role_and_label(Role::Button, "Hydra 2")
            .is_some()
    );
    harness
        .get_by_role_and_label(Role::Button, "Hydra 2")
        .click();
    harness.run();
    assert!(
        harness
            .query_by_role_and_label(Role::Button, "Hydra")
            .is_some()
    );
    assert!(harness.query_by_label("Heal: nothing to heal.").is_some());
    // Read while it showed, so hidden again it counts only what comes after;
    // and showing, it counts nothing, not even for the frame a line came in.
    harness.state_mut().story.tell(cena_session::Notice::line(
        cena_session::NoticeKind::Info,
        "Stop.",
    ));
    harness.step();
    assert!(
        harness
            .query_by_role_and_label(Role::Button, "Hydra")
            .is_some(),
        "a tab showing counts nothing"
    );
    harness.get_by_role_and_label(Role::Button, "Hunt").click();
    harness.run();
    assert!(
        harness
            .query_by_role_and_label(Role::Button, "Hydra")
            .is_some(),
        "nothing new since it showed"
    );
}

/// With Arrange on, a standalone window carried onto another's title bar
/// stacks with it; with it off, it does not.
#[test]
fn a_window_carried_onto_a_title_stacks_only_when_arranging() {
    for arrange in [false, true] {
        let mut harness = harness();
        harness.run();
        harness.state_mut().play.arranging = arrange;
        harness.run();
        let hunt = harness.get_by_label("Hunt").rect();
        let hydra = harness.get_by_label("Hydra").rect();
        drag(
            &mut harness,
            egui::pos2(hunt.center().x, hunt.min.y + 12.0),
            egui::pos2(hydra.center().x, hydra.min.y + 12.0),
        );
        let stacked = stacks(&harness, "Hydra") == [vec![Widget::Hydra, Widget::Hunt]];
        assert_eq!(stacked, arrange, "arranging: {arrange}");
    }
}

/// With Arrange on, one tab dragged out of its stack onto the middle of a
/// widget in another custom window joins that widget's stack; the rest of
/// its own stack stays.
#[test]
fn a_tab_dragged_onto_a_widget_joins_its_stack() {
    let mut harness = stacked();
    harness.state_mut().play.arranging = true;
    harness.run();
    let tab = harness.get_by_role_and_label(Role::Button, "Hunt").rect();
    let creatures = harness.get_by_label("Creatures").rect();
    drag(&mut harness, tab.center(), creatures.center());
    assert_eq!(stacks(&harness, "Hydra"), [vec![Widget::Hydra]]);
    assert!(
        stacks(&harness, "Room").contains(&vec![Widget::Creatures, Widget::Hunt]),
        "{:?}",
        stacks(&harness, "Room")
    );
}

/// With Arrange on, a cell moved onto the middle of another cell in its own
/// window joins that cell's stack; moved onto its side, it does not.
#[test]
fn a_cell_moved_onto_a_widgets_middle_stacks() {
    let mut harness = harness();
    harness.run();
    harness.state_mut().play.arranging = true;
    harness.run();
    let objects = harness.get_by_label("Objects").rect();
    let creatures = harness.get_by_label("Creatures").rect();
    drag(&mut harness, objects.center(), creatures.center());
    assert!(
        stacks(&harness, "Room").contains(&vec![Widget::Creatures, Widget::Objects]),
        "{:?}",
        stacks(&harness, "Room")
    );
}

/// A widget first seen as a tab not showing counts from then, not from the
/// start of the story; once shown, it is read.
#[test]
fn a_tab_first_seen_hidden_counts_from_then() {
    let story = crate::fixture::story();
    let seen = crate::widget::Seen {
        snapshot: None,
        story: &story,
        hunt: None,
        who: None,
    };
    let hydra = crate::layout::Placed {
        id: 7,
        widget: Widget::Hydra,
    };
    let mut read = std::collections::HashMap::new();
    assert_eq!(super::super::draw::unread(&mut read, hydra, &seen), None);
    assert_eq!(read.get(&7), Some(&story.told));
}

/// With Arrange on, a tab still switches on a click.
#[test]
fn a_tab_switches_while_arranging() {
    let mut harness = stacked();
    harness.state_mut().play.arranging = true;
    harness.run();
    harness.get_by_role_and_label(Role::Button, "Hunt").click();
    harness.run();
    assert!(harness.query_by_label("ojandhaart").is_some());
}

/// A tab stack as drawn: Hydra's messages showing, and the Hunt tab.
#[test]
fn a_tab_stack_as_drawn() {
    let mut scene = Scene::new();
    scene.snapshot.state.roundtime_ends = None;
    let mut harness = Harness::builder()
        .with_size((1000.0, 700.0))
        .wgpu()
        .build_ui_state(|ui, scene: &mut Scene| scene.draw(ui), scene);
    harness.run();
    if let Some(layout) = harness.state_mut().play.layout.as_mut() {
        let hunt = layout
            .titled("Hunt")
            .map(|holder| holder.id)
            .expect("a hunt");
        let hydra = layout
            .titled("Hydra")
            .map(crate::layout::Holder::rect)
            .expect("hydra");
        assert!(layout.join(hunt, hydra.min + egui::vec2(40.0, 10.0), &[]));
    }
    harness.run();
    harness.step();
    harness.snapshot("tabs");
}

/// While a cell is carried over the middle of another, the other lights up:
/// let go there, they stack.
#[test]
fn the_cell_it_would_stack_onto_lights_up() {
    let mut scene = Scene::new();
    scene.snapshot.state.roundtime_ends = None;
    let mut harness = Harness::builder()
        .with_size((1000.0, 700.0))
        .wgpu()
        .build_ui_state(|ui, scene: &mut Scene| scene.draw(ui), scene);
    harness.run();
    harness.state_mut().play.arranging = true;
    harness.run();
    let objects = harness.get_by_label("Objects").rect().center();
    let creatures = harness.get_by_label("Creatures").rect().center();
    harness.hover_at(objects);
    harness.step();
    harness.drag_at(objects);
    harness.step();
    for step in 1..=4u8 {
        harness.hover_at(objects + (creatures - objects) * (f32::from(step) / 4.0));
        harness.step();
    }
    harness.snapshot("stack");
    harness.drop_at(creatures);
    harness.step();
}

/// With Arrange on, a tab let go on empty space in its own window takes a
/// cell of its own there, leaving the rest of its stack.
#[test]
fn a_tab_let_go_on_empty_space_takes_a_cell_of_its_own() {
    let mut harness = harness();
    harness.run();
    harness.state_mut().play.arranging = true;
    harness.run();
    let objects = harness.get_by_label("Objects").rect();
    let creatures = harness.get_by_label("Creatures").rect();
    drag(&mut harness, objects.center(), creatures.center());
    // Where Objects was is empty now; its tab goes back there.
    let tab = harness
        .get_by_role_and_label(Role::Button, "Objects")
        .rect();
    drag(&mut harness, tab.center(), objects.center());
    let room = stacks(&harness, "Room");
    assert!(room.contains(&vec![Widget::Creatures]), "{room:?}");
    assert!(room.contains(&vec![Widget::Objects]), "{room:?}");
}
