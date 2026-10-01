//! The trigger editor's list: what it shows, and what it asks of the file.

use cena_ui::triggers::{Book, Change, Entry, Form, Look, Switch};
use egui_kittest::Harness;
use egui_kittest::kittest::Queryable;

use super::Editor;

fn entry(name: &str, category: &str, summary: &str) -> Entry {
    Entry {
        name: name.to_owned(),
        category: category.to_owned(),
        enabled: true,
        summary: summary.to_owned(),
        form: Form {
            name: name.to_owned(),
            category: category.to_owned(),
            text: name.to_owned(),
            look: Some(Look {
                bold: true,
                span: "match".to_owned(),
                ..Look::default()
            }),
            ..Form::default()
        },
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
        ..Book::default()
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
        .with_size((1100.0, 1200.0))
        .build_ui_state(
            |ui, scene: &mut Scene| {
                let asked = scene.editor.show(ui, Some(&scene.book), None, &[]);
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

/// The last save the editor asked for.
fn saved(harness: &Harness<'_, Scene>) -> Option<(Option<String>, Form)> {
    harness
        .state()
        .asked
        .iter()
        .rev()
        .find_map(|change| match change {
            Change::Save { was, form, .. } => Some((was.clone(), (**form).clone())),
            _ => None,
        })
}

/// *Duplicate* on a trigger from elsewhere whose send is held: the copy is
/// saved as a copy of it, so the writer keeps its origin and hold, and the
/// window says the copy's send waits too (GU-D-6).
#[test]
fn a_duplicate_is_saved_as_a_copy_and_says_its_send_waits() {
    let mut harness = harness();
    harness.get_by_label("webbed").click();
    harness.run();
    harness.get_by_label("Duplicate").click();
    harness.run();
    assert!(harness.query_by_label("A new trigger").is_some());
    assert!(
        harness
            .query_by_label_contains("A copy of `webbed`, from Wrayth: Nisugi3.xml")
            .is_some()
    );
    harness
        .state_mut()
        .editor
        .draft_form()
        .expect("a draft")
        .send = Some("stance defensive".to_owned());
    harness.run();
    assert!(
        harness
            .query_by_label_contains("The copy sends \"stance defensive\" only once you approve it")
            .is_some()
    );
    harness.get_by_label("Save").click();
    harness.run();
    let copy = harness
        .state()
        .asked
        .iter()
        .rev()
        .find_map(|change| match change {
            Change::Save { was, form, copy_of } => {
                Some((was.clone(), form.name.clone(), copy_of.clone()))
            }
            _ => None,
        });
    assert_eq!(
        copy,
        Some((None, "webbed copy".to_owned(), Some("webbed".to_owned())))
    );
}

#[test]
fn a_trigger_edited_is_saved_under_its_old_name() {
    let mut harness = harness();
    harness.get_by_label("stunned").click();
    harness.run();
    harness.get_by_label("Save").click();
    harness.run();
    assert_eq!(
        saved(&harness),
        None,
        "nothing to save until something changes"
    );
    harness
        .state_mut()
        .editor
        .draft_form()
        .expect("a draft")
        .text = "You are stunned".to_owned();
    harness.run();
    harness.get_by_label("Save").click();
    harness.run();
    let (was, form) = saved(&harness).expect("a save");
    assert_eq!(was.as_deref(), Some("stunned"));
    assert_eq!(form.text, "You are stunned");
    assert_eq!(form.category, "Combat", "the rest as it was");
}

#[test]
fn a_new_trigger_is_saved_with_no_old_name() {
    let mut harness = harness();
    harness.get_by_label("+ New").click();
    harness.run();
    assert!(harness.query_by_label("A new trigger").is_some());
    let form = harness.state_mut().editor.draft_form().expect("a draft");
    form.name = "  rock ".to_owned();
    form.text = "a rock".to_owned();
    form.squelch = true;
    harness.run();
    harness.get_by_label("Save").click();
    harness.run();
    let (was, form) = saved(&harness).expect("a save");
    assert_eq!(was, None);
    assert_eq!(form.name, "rock", "trimmed before it is saved");
}

#[test]
fn a_condition_saves_without_the_line_responses_it_cannot_have() {
    let mut harness = harness();
    harness.get_by_label("stunned").click();
    harness.run();
    harness.get_by_label("A condition").click();
    harness.run();
    assert!(
        harness
            .query_by_label("A condition has no line to colour, hide, change or move.")
            .is_some()
    );
    harness
        .state_mut()
        .editor
        .draft_form()
        .expect("a draft")
        .condition = "stunned".to_owned();
    harness.run();
    harness.get_by_label("Save").click();
    harness.run();
    let (_, form) = saved(&harness).expect("a save");
    assert_eq!(form.condition, "stunned");
    assert_eq!(form.text, "", "a condition watches no line");
    assert_eq!(form.look, None, "its look went with the line");
}

#[test]
fn unsaved_changes_are_not_thrown_away_by_a_click() {
    let mut harness = harness();
    harness.get_by_label("stunned").click();
    harness.run();
    harness
        .state_mut()
        .editor
        .draft_form()
        .expect("a draft")
        .text = "changed".to_owned();
    harness.run();
    harness.get_by_label("spam").click();
    harness.run();
    assert!(
        harness
            .query_by_label_contains("has changes not saved")
            .is_some()
    );
    assert_eq!(
        harness
            .state_mut()
            .editor
            .draft_form()
            .map(|form| form.name.clone()),
        Some("stunned".to_owned())
    );
}

#[test]
fn a_colour_reads_back_as_its_bytes() {
    assert_eq!(super::form::parse_hex("#ff4000"), Some([255, 64, 0]));
    assert_eq!(super::form::parse_hex("ff4000"), None);
    assert_eq!(super::form::parse_hex("#fff"), None);
}

mod live {
    //! The live test (`plan/54` step 3), through the real matcher.

    use cena_ui::triggers::{Book, Form, Look};

    use super::super::test::{Test, run};
    use super::entry;

    fn line(words: &str) -> Test {
        Test {
            line: words.to_owned(),
            stream: String::new(),
        }
    }

    fn book() -> Book {
        let mut rock = entry("rock", "Ignores", "\"a rock\" -> squelch");
        rock.form.text = "a rock".to_owned();
        rock.form.look = None;
        rock.form.squelch = true;
        Book {
            triggers: vec![entry("stunned", "Combat", "\"stunned\" -> look"), rock],
            categories: vec![("Combat".to_owned(), true), ("Ignores".to_owned(), true)],
            ..Book::default()
        }
    }

    #[test]
    fn a_saved_squelch_hides_the_line() {
        let outcome = run(&line("You see a rock."), &book(), None);
        assert_eq!(outcome.fired, ["rock"]);
        assert!(outcome.shown.is_empty(), "squelched");
    }

    #[test]
    fn the_form_stands_in_for_its_saved_self_before_it_is_saved() {
        let mut form = book().triggers[0].form.clone();
        form.look = Some(Look {
            color: "#ff4040".to_owned(),
            span: "line".to_owned(),
            ..Look::default()
        });
        let outcome = run(
            &line("You are stunned!"),
            &book(),
            Some((Some("stunned"), &form)),
        );
        assert_eq!(outcome.fired, ["stunned"], "once, as the form has it");
        let paint = &outcome.shown[0].paint;
        assert!(
            paint
                .iter()
                .any(|p| p.color.is_some_and(|c| c.red == 0xff && c.green == 0x40)),
            "{paint:?}"
        );
    }

    #[test]
    fn a_form_the_file_would_refuse_says_why() {
        let form = Form {
            name: "bad".to_owned(),
            text: "(".to_owned(),
            regex: true,
            squelch: true,
            ..Form::default()
        };
        let outcome = run(&line("anything"), &book(), Some((None, &form)));
        assert!(outcome.refused.is_some());
    }

    #[test]
    fn a_category_switched_off_does_not_fire() {
        let mut book = book();
        book.categories[1].1 = false;
        let outcome = run(&line("You see a rock."), &book, None);
        assert!(outcome.fired.is_empty());
        assert_eq!(outcome.shown.len(), 1);
    }
}
