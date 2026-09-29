//! The trigger editor's list: what it shows, and what it asks of the file.

use cena_ui::triggers::{Book, Change, Entry, Switch};
use egui_kittest::Harness;
use egui_kittest::kittest::Queryable;

use super::Editor;

fn entry(name: &str, category: &str, summary: &str) -> Entry {
    Entry {
        name: name.to_owned(),
        category: category.to_owned(),
        enabled: true,
        summary: summary.to_owned(),
        ..Entry::default()
    }
}

fn book() -> Book {
    let mut webbed = entry("webbed", "Combat", "\"webbed\" -> look, send");
    webbed.held = Some("stance defensive".to_owned());
    webbed.origin = Some("Wrayth: Nisugi3.xml".to_owned());
    let mut broken = entry("broken", "", "/(/ -> look");
    broken.refused = Some("the regex does not read: unclosed group".to_owned());
    Book {
        triggers: vec![
            broken,
            entry("stunned", "Combat", "\"You are stunned\" -> look, sound"),
            webbed,
            entry("spam", "Ignores", "/gestures/ -> squelch"),
        ],
        categories: vec![("Combat".to_owned(), true), ("Ignores".to_owned(), false)],
        kinds: ["look", "squelch", "send"]
            .iter()
            .map(|kind| ((*kind).to_owned(), true))
            .collect(),
        file: "triggers.toml".to_owned(),
        problem: None,
    }
}

/// The editor over a book, gathering what it asks.
struct Scene {
    editor: Editor,
    book: Book,
    asked: Vec<Change>,
}

fn harness<'a>() -> Harness<'a, Scene> {
    let scene = Scene {
        editor: Editor::default(),
        book: book(),
        asked: Vec::new(),
    };
    Harness::builder()
        .with_size((1100.0, 680.0))
        .build_ui_state(
            |ui, scene: &mut Scene| {
                let asked = scene.editor.show(ui, Some(&scene.book), None);
                scene.asked.extend(asked);
            },
            scene,
        )
}

#[test]
fn the_list_shows_each_category_and_what_each_trigger_does() {
    let harness = harness();
    for label in [
        "Combat (2)",
        "Ignores (1)",
        "(no category) (1)",
        "stunned",
        "\"You are stunned\" -> look, sound",
        "send waits",
        "refused",
    ] {
        assert!(harness.query_by_label(label).is_some(), "{label}");
    }
}

#[test]
fn a_held_send_is_approved_and_a_removal_asked_twice() {
    let mut harness = harness();
    harness.get_by_label("webbed").click();
    harness.run();
    assert!(
        harness
            .query_by_label_contains("sends \"stance defensive\" only once you approve it")
            .is_some()
    );
    harness.get_by_label("Approve this command").click();
    harness.run();
    harness.get_by_label("Remove...").click();
    harness.run();
    assert!(
        harness.state().asked == [Change::Approve("webbed".to_owned())],
        "asking to remove asks again first: {:?}",
        harness.state().asked
    );
    harness.get_by_label("Remove").click();
    harness.run();
    assert_eq!(
        harness.state().asked.last(),
        Some(&Change::Remove("webbed".to_owned()))
    );
}

#[test]
fn search_narrows_the_list() {
    let mut harness = harness();
    harness.state_mut().editor.search = "squelch".to_owned();
    harness.run();
    assert!(harness.query_by_label("spam").is_some());
    assert!(harness.query_by_label("stunned").is_none());
}

#[test]
fn a_category_switch_asks_for_the_category() {
    let mut harness = harness();
    // Ignores is off; its box turns it on.
    let boxes = harness.query_all_by_label("on").count();
    assert_eq!(boxes, 2, "one per named category");
    harness
        .query_all_by_label("on")
        .nth(1)
        .expect("Ignores' switch")
        .click();
    harness.run();
    assert_eq!(
        harness.state().asked,
        [Change::Switch(Switch::Category("Ignores".to_owned()), true)]
    );
}
