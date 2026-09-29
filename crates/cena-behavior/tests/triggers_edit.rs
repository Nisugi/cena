//! `;trigger`'s changes to the triggers file
//! (`crates/cena-behavior/src/triggers/edit.rs`): each read back as Hydra
//! will read it, and written sorted by category, then name.

use cena_behavior::settings::typed;
use cena_behavior::triggers::edit::{self, Switch};
use cena_behavior::triggers::read;
use cena_session::trigger::{Pattern, Trigger};

/// What `Nisugi` runs from `text`.
fn triggers(text: &str) -> Option<Vec<Trigger>> {
    Some(read(text).ok()?.triggers.for_character("Nisugi"))
}

fn names(text: &str) -> Option<Vec<String>> {
    Some(triggers(text)?.into_iter().map(|t| t.name).collect())
}

#[test]
fn add_makes_a_trigger_that_bolds_its_words() {
    let text = edit::add("", "stunned", "You are stunned").unwrap();
    let mine = triggers(&text).unwrap();
    assert_eq!(mine.len(), 1);
    assert_eq!(
        mine[0].rule.pattern,
        Some(Pattern::Literal {
            text: "You are stunned".into(),
            whole_word: true,
        })
    );
    assert!(mine[0].rule.look.as_ref().is_some_and(|look| look.bold));
    let again = edit::add(&text, "stunned", "other words").unwrap_err();
    assert!(again.contains("already"), "{again}");
    assert!(edit::add(&text, "", "x").is_err());
}

#[test]
fn set_changes_one_field_and_a_refused_result_is_refused() {
    let text = edit::add("", "stunned", "You are stunned").unwrap();
    let (text, old) = edit::set(&text, "stunned", "look.color", typed("#ff4040")).unwrap();
    assert_eq!(old, None);
    let look = triggers(&text).unwrap()[0].rule.look.clone().unwrap();
    assert_eq!(
        look.color.map(|c| c.to_string()).as_deref(),
        Some("#ff4040")
    );
    assert!(look.bold, "the bold `add` gave it stays");

    let bad = edit::set(&text, "stunned", "look.color", typed("red")).unwrap_err();
    assert!(bad.contains("is not a colour"), "{bad}");
    let both = edit::set(&text, "stunned", "regex", typed("stun+ed")).unwrap_err();
    assert!(both.contains("both `text` and `regex`"), "{both}");
    let none = edit::set(&text, "nobody", "squelch", typed("on")).unwrap_err();
    assert!(none.contains("no trigger `nobody`"), "{none}");
}

/// A name is the trigger's whole key, dots and all: `set` does not split it.
#[test]
fn a_name_with_dots_is_one_name() {
    let text = edit::add("", "a.b", "words").unwrap();
    let (text, _) = edit::set(&text, "a.b", "squelch", typed("on")).unwrap();
    let mine = triggers(&text).unwrap();
    assert_eq!(mine[0].name, "a.b");
    assert!(mine[0].rule.squelch);
}

#[test]
fn unset_and_remove() {
    let text = edit::add("", "stunned", "You are stunned").unwrap();
    let (text, _) = edit::set(&text, "stunned", "squelch", typed("on")).unwrap();
    let text = edit::unset(&text, "stunned", "squelch").unwrap();
    assert!(!triggers(&text).unwrap()[0].rule.squelch);
    let never = edit::unset(&text, "stunned", "squelch").unwrap_err();
    assert!(never.contains("does not set"), "{never}");
    // Taking away the only response leaves it doing nothing: refused.
    let empty = edit::unset(&text, "stunned", "look").unwrap_err();
    assert!(empty.contains("does nothing"), "{empty}");

    let text = edit::remove(&text, "stunned").unwrap();
    assert!(names(&text).unwrap().is_empty());
    assert!(edit::remove(&text, "stunned").is_err());
}

#[test]
fn on_and_off_switch_a_trigger_a_category_or_a_kind() {
    let text = edit::add("", "spam", "gestures").unwrap();
    let (text, _) = edit::set(&text, "spam", "category", typed("Ignores")).unwrap();
    let (text, _) = edit::set(&text, "spam", "squelch", typed("on")).unwrap();

    let off = edit::switch(&text, Switch::Trigger("spam"), false).unwrap();
    assert!(names(&off).unwrap().is_empty());
    let on = edit::switch(&off, Switch::Trigger("spam"), true).unwrap();
    assert_eq!(names(&on).unwrap(), ["spam"]);

    let category = edit::switch(&text, Switch::Category("Ignores"), false).unwrap();
    assert!(names(&category).unwrap().is_empty());

    let squelch = edit::switch(&text, Switch::Every("squelch"), false).unwrap();
    let mine = triggers(&squelch).unwrap();
    assert!(
        !mine[0].rule.squelch,
        "the look is left, so the trigger stays"
    );

    let bad = edit::switch(&text, Switch::Every("rumble"), false).unwrap_err();
    assert!(bad.contains("not a kind of response"), "{bad}");
}

/// Written sorted, whatever order it was in, and the head comments kept.
#[test]
fn the_file_is_written_by_category_then_name() {
    let text = "# my triggers\n\n\
                [trigger.zeta]\ncategory = \"B\"\ntext = \"1\"\nsquelch = true\n\n\
                [trigger.alpha]\ncategory = \"B\"\ntext = \"2\"\nsquelch = true\n";
    let text = edit::add(text, "omega", "3").unwrap();
    let (text, _) = edit::set(&text, "omega", "category", typed("A")).unwrap();
    assert!(text.starts_with("# my triggers\n"), "{text}");
    let at = |name: &str| text.find(&format!("[trigger.{name}]"));
    assert!(
        at("omega") < at("alpha") && at("alpha") < at("zeta"),
        "{text}"
    );
}

#[test]
fn show_and_list() {
    let text = edit::add("", "stunned", "You are stunned").unwrap();
    let (text, _) = edit::set(&text, "stunned", "category", typed("Combat")).unwrap();
    // As written: a table's plain keys come before its tables.
    assert_eq!(
        edit::show(&text, "stunned").unwrap(),
        [
            "text = \"You are stunned\"",
            "category = \"Combat\"",
            "look.bold = true"
        ]
    );
    // A trigger the file cannot use is listed, with why.
    let text = format!("{text}\n[trigger.broken]\ntext = \"x\"\n");
    let listed = edit::list(&text).unwrap();
    let rows: Vec<(&str, &str, bool)> = listed
        .iter()
        .map(|row| {
            (
                row.category.as_str(),
                row.name.as_str(),
                row.refused.is_some(),
            )
        })
        .collect();
    assert_eq!(rows, [("", "broken", true), ("Combat", "stunned", false)]);
}

/// A trigger refused elsewhere in the file does not stop a change to another.
#[test]
fn one_bad_trigger_does_not_block_editing_another() {
    let text = "[trigger.broken]\ntext = \"x\"\n";
    let text = edit::add(text, "fine", "words").unwrap();
    assert_eq!(names(&text).unwrap(), ["fine"]);
}

/// `approve` names the line approved; the player's own needs none.
#[test]
fn approve_names_the_line_it_lets_send() {
    let text = "[trigger.theirs]\ntext = 'x'\nsend = 'stand'\norigin = 'a shared file'\n";
    let (approved, line) = edit::approve(text, "theirs").unwrap();
    assert_eq!(line, "stand");
    assert!(read(&approved).unwrap().held.is_empty());
    let own = "[trigger.mine]\ntext = 'x'\nsend = 'stand'\n";
    assert!(
        edit::approve(own, "mine")
            .unwrap_err()
            .contains("player's own")
    );
    let silent = "[trigger.quiet]\ntext = 'x'\nsquelch = true\norigin = 'a shared file'\n";
    assert!(
        edit::approve(silent, "quiet")
            .unwrap_err()
            .contains("sends nothing")
    );
}

/// The trigger editor's list (`plan/54` step 1): what each trigger watches
/// and does in a line, a send waiting on approval, where it came from; and
/// every category and kind with its switch.
#[test]
fn the_list_says_what_each_does_and_the_switches_are_read() {
    let text = "[categories]\nIgnores = false\n\n[responses]\nsound = false\n\n\
                [trigger.theirs]\ncategory = 'Combat'\ntext = 'webbed'\nsend = 'stand'\norigin = 'a shared file'\n\n\
                [trigger.spam]\ncategory = 'Ignores'\nregex = 'gestures'\nsquelch = true\n\n\
                [trigger.low]\ncondition = '!health_at_least 30'\nflag = { name = 'low' }\n";
    let listed = edit::list(text).unwrap();
    let theirs = listed.iter().find(|row| row.name == "theirs").unwrap();
    assert_eq!(theirs.summary, "\"webbed\" -> send");
    assert_eq!(theirs.held.as_deref(), Some("stand"));
    assert_eq!(theirs.origin.as_deref(), Some("a shared file"));
    let spam = listed.iter().find(|row| row.name == "spam").unwrap();
    assert_eq!(spam.summary, "/gestures/ -> squelch");
    assert_eq!(spam.held, None);
    let low = listed.iter().find(|row| row.name == "low").unwrap();
    assert_eq!(low.summary, "when !health_at_least 30 -> flag");

    let switches = edit::switches(text).unwrap();
    assert_eq!(
        switches.categories,
        [("Combat".to_owned(), true), ("Ignores".to_owned(), false)]
    );
    assert!(switches.kinds.contains(&("sound", false)));
    assert!(switches.kinds.contains(&("look", true)));
    assert_eq!(switches.kinds.len(), edit::KINDS.len());
}
