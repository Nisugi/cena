//! A link clicked in the story, as a player clicks one (the author,
//! 2026-09-28: *"links using the preset highlights, clickable with their
//! menus popping up"*): an object asks the game for its menu, which pops up
//! when the game answers; a command link sends its command.

use std::sync::Arc;

use super::*;
use crate::story::Shown;
use cena_session::{Event, Frame, Generation, Link, LinkKind, ObservedEvent, SessionId};

pub(super) fn observed(event: Event) -> ObservedEvent {
    ObservedEvent {
        session: SessionId::FIRST,
        generation: Generation::FIRST,
        cursor: 1,
        event,
    }
}

/// The story hears a line of `runs`: each its text, and for some a link.
pub(super) fn heard(scene: &mut Scene, runs: &[(&str, Option<LinkKind>)]) {
    let mut line = cena_session::ChunkLine::plain("template").runs;
    let template = line.runs.first().cloned();
    line.runs = runs
        .iter()
        .filter_map(|(text, kind)| {
            let mut run = template.clone()?;
            run.text = (*text).to_owned();
            run.link = kind.clone().map(|kind| Link {
                kind,
                text: (*text).to_owned(),
                coord: None,
            });
            Some(run)
        })
        .collect();
    let event = Event::Line(Arc::new(cena_session::Line::new("", line)));
    scene.story.hear(&observed(event), None);
}

/// The game answers a menu asked for with number `id`, of these coordinates.
fn answered(scene: &mut Scene, id: &str, coords: &[&str]) {
    let mut menu = cena_session::Menu {
        id: id.to_owned(),
        ..Default::default()
    };
    menu.items.resize_with(coords.len(), Default::default);
    for (item, coord) in menu.items.iter_mut().zip(coords) {
        item.coord = Some((*coord).to_owned());
    }
    let event = Event::Frame(Box::new(Frame::MenuResponse(menu)));
    scene.story.hear(&observed(event), None);
}

pub(super) fn rat() -> LinkKind {
    LinkKind::Exist {
        id: "456".to_owned(),
        noun: "rat".to_owned(),
    }
}

/// A click at `at`: pressed and let go there, the pointer staying, as
/// kittest's `drop_at` does not (it removes the pointer as it lets go, and
/// egui then has no place for the click).
pub(super) fn click_at(harness: &mut Harness<'_, Scene>, at: egui::Pos2) {
    let button = |pressed| egui::Event::PointerButton {
        pos: at,
        button: egui::PointerButton::Primary,
        pressed,
        modifiers: egui::Modifiers::NONE,
    };
    harness.hover_at(at);
    harness.step();
    harness.event(button(true));
    harness.step();
    harness.event(button(false));
    harness.run();
}

/// A click on an object asks the game for its menu, not echoed; when the
/// game answers, the menu pops up, each entry labelled for the object, and
/// the one chosen sends its command and closes it.
#[test]
fn an_objects_menu_is_asked_for_and_chosen_from() {
    let mut harness = harness();
    heard(harness.state_mut(), &[("a grey rat", Some(rat()))]);
    harness.run();
    harness.get_by_label("a grey rat").click();
    harness.run();
    assert_eq!(
        harness.state().asked,
        [Asked::Quietly("_menu #456 1".to_owned())]
    );
    assert!(
        harness.query_by_label("attack").is_none(),
        "nothing until the game answers"
    );
    answered(harness.state_mut(), "1", &["2524,1543", "2524,1707"]);
    harness.run();
    assert!(harness.query_by_label("tell").is_some());
    harness.get_by_label("attack").click();
    harness.run();
    assert_eq!(
        harness.state().asked.last(),
        Some(&Asked::Send("attack #456".to_owned()))
    );
    assert!(harness.query_by_label("attack").is_none(), "closed");
}

/// A menu shows only the game's answer to the last one asked; Escape closes
/// one not yet answered, and each asked for is numbered anew.
#[test]
fn a_menu_waits_for_its_own_answer_and_escape_closes_it() {
    let mut harness = harness();
    heard(harness.state_mut(), &[("a grey rat", Some(rat()))]);
    harness.run();
    harness.get_by_label("a grey rat").click();
    harness.run();
    answered(harness.state_mut(), "7", &["2524,1543"]);
    harness.run();
    assert!(harness.query_by_label("attack").is_none(), "not its answer");
    harness.key_press(egui::Key::Escape);
    harness.run();
    answered(harness.state_mut(), "1", &["2524,1543"]);
    harness.run();
    assert!(harness.query_by_label("attack").is_none(), "closed");
    harness.get_by_label("a grey rat").click();
    harness.run();
    assert_eq!(
        harness.state().asked.last(),
        Some(&Asked::Quietly("_menu #456 2".to_owned()))
    );
    answered(harness.state_mut(), "2", &["2524,1543"]);
    harness.run();
    assert!(harness.query_by_label("attack").is_some(), "its own answer");
    click_at(&mut harness, egui::pos2(5.0, 5.0));
    assert!(
        harness.query_by_label("attack").is_none(),
        "a press elsewhere closes it"
    );
}

/// A command link sends its command, as typed; a click on the line's plain
/// words does nothing.
#[test]
fn a_command_link_sends_and_plain_words_do_not() {
    let mut harness = harness();
    let north = Some(LinkKind::Direct {
        cmd: "go north".to_owned(),
    });
    heard(
        harness.state_mut(),
        &[("Obvious paths: ", None), ("north", north)],
    );
    harness.run();
    let line = harness.get_by_label("Obvious paths: north").rect();
    click_at(&mut harness, egui::pos2(line.left() + 4.0, line.center().y));
    assert!(harness.state().asked.is_empty(), "plain words");
    click_at(
        &mut harness,
        egui::pos2(line.right() - 4.0, line.center().y),
    );
    assert_eq!(harness.state().asked, [Asked::Send("go north".to_owned())]);
}

/// An object link whose own `coord=` names a command sends it at once, for
/// that object, asking no menu.
#[test]
fn an_object_naming_its_own_command_sends_it() {
    let mut harness = harness();
    heard(harness.state_mut(), &[("a grey rat", Some(rat()))]);
    let Some((_, Shown::Game(runs))) = harness.state_mut().story.lines.back_mut() else {
        panic!("the rat's line");
    };
    if let Some(cena_ui::RunLink::Object { coord, .. }) =
        runs.first_mut().and_then(|run| run.link.as_mut())
    {
        *coord = Some("2524,1543".to_owned());
    }
    harness.run();
    harness.get_by_label("a grey rat").click();
    harness.run();
    assert_eq!(
        harness.state().asked,
        [Asked::Send("attack #456".to_owned())]
    );
}

/// A reconnect closes an object's menu, open or asked for: the object's id
/// and the game's answer were the old connection's (the crate review of
/// 2026-09-28, R13).
#[test]
fn a_reconnect_closes_an_objects_menu() {
    let mut harness = harness();
    heard(harness.state_mut(), &[("a grey rat", Some(rat()))]);
    harness.run();
    harness.get_by_label("a grey rat").click();
    harness.run();
    answered(harness.state_mut(), "1", &["2524,1543"]);
    harness.run();
    assert!(harness.query_by_label("attack").is_some(), "open");
    harness.state_mut().snapshot.generation = Generation::FIRST.next();
    harness.run();
    assert!(harness.query_by_label("attack").is_none(), "closed");
}

/// A window reopened begins its numbers again, and takes no answer the
/// story kept from before it asked: only one that comes after.
#[test]
fn a_reopened_window_takes_no_answer_from_before() {
    let mut harness = harness();
    heard(harness.state_mut(), &[("a grey rat", Some(rat()))]);
    answered(harness.state_mut(), "1", &["2524,1543"]);
    harness.run();
    harness.get_by_label("a grey rat").click();
    harness.run();
    assert_eq!(
        harness.state().asked,
        [Asked::Quietly("_menu #456 1".to_owned())]
    );
    assert!(harness.query_by_label("attack").is_none(), "the old answer");
    answered(harness.state_mut(), "1", &["2524,1543"]);
    harness.run();
    assert!(harness.query_by_label("attack").is_some(), "its own");
}

/// A press at `from`, dragged to `to` and let go there, then copied: what
/// the copy put on the clipboard.
fn dragged_and_copied(
    harness: &mut Harness<'_, Scene>,
    from: egui::Pos2,
    to: egui::Pos2,
) -> Vec<String> {
    let button = |pos, pressed| egui::Event::PointerButton {
        pos,
        button: egui::PointerButton::Primary,
        pressed,
        modifiers: egui::Modifiers::NONE,
    };
    harness.hover_at(from);
    harness.step();
    harness.event(button(from, true));
    harness.step();
    for step in 1..=4u8 {
        harness.hover_at(from + (to - from) * (f32::from(step) / 4.0));
        harness.step();
    }
    harness.event(button(to, false));
    harness.step();
    harness.event(egui::Event::Copy);
    harness.step();
    harness
        .output()
        .platform_output
        .commands
        .iter()
        .filter_map(|command| match command {
            egui::OutputCommand::CopyText(text) => Some(text.clone()),
            _ => None,
        })
        .collect()
}

/// A line with a link in it is selected as any other: a drag across it
/// selects its words, the link's among them, and a selection runs on from a
/// plain line into it; a drag is not a click on the link (the author,
/// 2026-09-28: *"It selects lines with just normal text, but I think links
/// in the lines break text selection"*).
#[test]
fn a_line_with_a_link_is_selected_and_copied() {
    let mut harness = harness();
    heard(harness.state_mut(), &[("You swing.", None)]);
    heard(
        harness.state_mut(),
        &[
            ("You see ", None),
            ("a grey rat", Some(rat())),
            (" here.", None),
        ],
    );
    harness.run();
    let plain = harness.get_by_label("You swing.").rect();
    let line = harness.get_by_label("You see a grey rat here.").rect();
    let (start, end) = (
        egui::pos2(line.left() + 1.0, line.center().y),
        egui::pos2(line.right() - 1.0, line.center().y),
    );
    let copied = dragged_and_copied(&mut harness, start, end);
    assert!(
        copied
            .iter()
            .any(|text| text.contains("You see a grey rat here.")),
        "the one line: {copied:?}"
    );
    let above = egui::pos2(plain.left() + 1.0, plain.center().y);
    let copied = dragged_and_copied(&mut harness, above, end);
    assert!(
        copied
            .iter()
            .any(|text| text.contains("You swing.") && text.contains("a grey rat here.")),
        "on from a plain line: {copied:?}"
    );
    assert!(harness.state().asked.is_empty(), "a drag is not a click");
}
