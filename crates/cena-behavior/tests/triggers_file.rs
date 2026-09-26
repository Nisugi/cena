//! The one triggers file (`plan/45` §5, `crates/cena-behavior/src/triggers.rs`),
//! read as TOML the way Hydra will read it.

use cena_behavior::triggers::{self, Loaded};
use cena_session::trigger::{Color, Look, Pattern, Redirect, Span, Trigger};
use std::path::PathBuf;

/// `text`, read. A file that is not TOML is a broken fixture.
fn read(text: &str) -> Option<Loaded> {
    triggers::read(text).ok()
}

/// What `character` runs from `text`, by name.
fn names(text: &str, character: &str) -> Option<Vec<String>> {
    let loaded = read(text)?;
    Some(
        loaded
            .triggers
            .for_character(character)
            .into_iter()
            .map(|trigger| trigger.name)
            .collect(),
    )
}

/// The one trigger `character` runs from `text`.
fn only(text: &str, character: &str) -> Option<Trigger> {
    let mut mine = read(text)?.triggers.for_character(character);
    (mine.len() == 1).then(|| mine.remove(0))
}

const RED: Color = Color {
    red: 0xff,
    green: 0x40,
    blue: 0x40,
};

#[test]
fn a_trigger_is_typed_as_written() {
    let trigger = only(
        r##"
        [trigger."stunned"]
        category = "Combat"
        text = "You are stunned"
        stream = "main"
        look = { color = "#FF4040", bold = true, span = "line" }
        "##,
        "Nisugi",
    )
    .unwrap();
    assert_eq!(trigger.name, "stunned");
    let rule = trigger.rule;
    assert_eq!(rule.category, "Combat");
    assert_eq!(rule.pattern, Pattern::Literal("You are stunned".into()));
    assert!(!rule.case_sensitive, "case is ignored unless asked for");
    assert_eq!(
        rule.stream.as_deref(),
        Some(""),
        "`main` is the model's \"\""
    );
    assert_eq!(
        rule.look,
        Some(Look {
            color: Some(RED),
            background: None,
            bold: true,
            span: Span::Line,
        })
    );
    assert!(!rule.squelch);
}

#[test]
fn a_regex_keeps_its_groups_and_the_text_responses_read() {
    let rule = only(
        r##"
        [trigger."whispers"]
        regex = '^(\w+) whispers, "(.*)"$'
        case_sensitive = true
        look = { background = "#000080", span = 2 }
        substitute = "$1: $2"
        redirect = { stream = "whispers", copy = true }
        "##,
        "Nisugi",
    )
    .unwrap()
    .rule;
    assert_eq!(
        rule.pattern,
        Pattern::Regex(r#"^(\w+) whispers, "(.*)"$"#.into())
    );
    assert!(rule.case_sensitive);
    assert_eq!(rule.look.map(|look| look.span), Some(Span::Group(2)));
    assert_eq!(rule.substitute.as_deref(), Some("$1: $2"));
    assert_eq!(
        rule.redirect,
        Some(Redirect {
            stream: "whispers".into(),
            copy: true,
        })
    );
}

/// Each bad trigger beside a good one: it is named, with a reason a player
/// can act on, and the good one still loads.
#[test]
fn a_bad_trigger_is_refused_by_name_and_the_rest_load() {
    let cases = [
        (
            "text = 'x'\nregex = 'x'\nsquelch = true",
            "both `text` and `regex`",
        ),
        ("squelch = true", "nothing to match"),
        ("text = ''\nsquelch = true", "`text` is empty"),
        ("regex = 'a(?=b)'\nsquelch = true", "regex cannot be used"),
        ("text = 'x'", "it does nothing"),
        (
            "text = 'x'\nlook = { colour = '#ff0000' }",
            "unknown field `colour`",
        ),
        ("text = 'x'\nlook = { color = 'red' }", "write it #rrggbb"),
        (
            "text = 'x'\nlook = { span = 'line' }",
            "no colour, background or bold",
        ),
        (
            "regex = '(a)'\nlook = { bold = true, span = 2 }",
            "has no group 2",
        ),
        (
            "text = 'x'\nlook = { bold = true, span = 1 }",
            "needs a `regex`",
        ),
        (
            "text = 'x'\nlook = { bold = true, span = 0 }",
            "groups count from 1",
        ),
        ("text = 'x'\nredirect = { stream = '' }", "names no stream"),
        (
            "text = 'x'\nsquelch = true\nenabled = 'no'",
            "not true or false",
        ),
        (
            "text = 'x'\nsquelch = true\ncharacters = []",
            "names nobody",
        ),
    ];
    for (body, reason) in cases {
        let file = format!("[trigger.bad]\n{body}\n\n[trigger.good]\ntext = 'y'\nsquelch = true\n");
        let loaded = read(&file).unwrap();
        assert_eq!(loaded.refused.len(), 1, "{body}: {:?}", loaded.refused);
        let refused = &loaded.refused[0];
        assert_eq!(refused.name, "bad", "{body}");
        assert!(refused.why.contains(reason), "{body}: {}", refused.why);
        let kept: Vec<String> = loaded
            .triggers
            .for_character("Nisugi")
            .into_iter()
            .map(|trigger| trigger.name)
            .collect();
        assert_eq!(kept, ["good"], "{body}");
    }
}

#[test]
fn characters_names_who_runs_it_ignoring_case() {
    let file = r#"
        [trigger.mine]
        text = "x"
        squelch = true
        characters = ["Nisugi"]

        [trigger.everyones]
        text = "y"
        squelch = true
    "#;
    assert_eq!(names(file, "NISUGI").unwrap(), ["everyones", "mine"]);
    assert_eq!(names(file, "Dicate").unwrap(), ["everyones"]);
}

/// `for.<name>` changes only what it names: the colour moves, the bold and
/// the pattern stay. A whole-rule replace, `VellumFE`'s way (`plan/45` §2a),
/// would leave Dicate a trigger with no pattern.
#[test]
fn an_override_changes_only_the_fields_it_names() {
    let file = r##"
        [trigger.stunned]
        text = "You are stunned"
        look = { color = "#ff4040", bold = true }

        [trigger.stunned.for.Dicate]
        look = { color = "#4040ff" }
    "##;
    let dicate = only(file, "dicate").unwrap().rule;
    assert_eq!(dicate.pattern, Pattern::Literal("You are stunned".into()));
    let look = dicate.look.unwrap();
    assert_eq!(
        look.color.map(|c| c.to_string()).as_deref(),
        Some("#4040ff")
    );
    assert!(
        look.bold,
        "the bold was everyone's and Dicate did not change it"
    );
    let nisugi = only(file, "Nisugi").unwrap().rule;
    assert_eq!(nisugi.look.and_then(|look| look.color), Some(RED));
}

#[test]
fn an_override_turns_a_trigger_off_or_on_for_one_character() {
    let file = r#"
        [trigger.off-for-dicate]
        text = "x"
        squelch = true
        for.Dicate.enabled = false

        [trigger.on-for-dicate]
        text = "y"
        squelch = true
        enabled = false
        for.Dicate.enabled = true
    "#;
    assert_eq!(names(file, "Dicate").unwrap(), ["on-for-dicate"]);
    assert_eq!(names(file, "Nisugi").unwrap(), ["off-for-dicate"]);
}

/// The trigger is the unit: a bad override refuses it for everyone, rather
/// than leave Dicate running what the player meant to change.
#[test]
fn a_bad_override_refuses_the_whole_trigger() {
    for (over, reason) in [
        (
            "look = { color = 'blue' }",
            "for Dicate, `blue` is not a colour",
        ),
        ("characters = ['Dicate']", "belong to the trigger"),
    ] {
        let file = format!(
            "[trigger.stunned]\ntext = 'x'\nlook = {{ bold = true }}\n\n\
             [trigger.stunned.for.Dicate]\n{over}\n"
        );
        let loaded = read(&file).unwrap();
        assert_eq!(loaded.refused.len(), 1, "{over}");
        assert!(
            loaded.refused[0].why.contains(reason),
            "{}",
            loaded.refused[0]
        );
        assert!(loaded.triggers.for_character("Nisugi").is_empty(), "{over}");
    }
    let twice = "[trigger.t]\ntext = 'x'\nsquelch = true\n\
                 [trigger.t.for.Dicate]\nenabled = false\n\
                 [trigger.t.for.dicate]\nenabled = false\n";
    let loaded = read(twice).unwrap();
    assert!(
        loaded.refused[0].why.contains("named twice"),
        "{:?}",
        loaded.refused
    );
}

#[test]
fn a_category_switched_off_is_off_for_everyone() {
    let file = r#"
        [trigger.spam]
        category = "Ignores"
        text = "x"
        squelch = true

        [trigger.stunned]
        category = "Combat"
        text = "y"
        look = { bold = true }

        [categories]
        Ignores = false
        Combat = true
    "#;
    assert_eq!(names(file, "Nisugi").unwrap(), ["stunned"]);
}

/// With squelches off, a squelch-only trigger has nothing left and goes; one
/// that also colours keeps its colour.
#[test]
fn a_response_switched_off_is_taken_from_every_trigger() {
    let file = r##"
        [trigger.hide]
        text = "x"
        squelch = true

        [trigger.hide-and-colour]
        text = "y"
        squelch = true
        look = { color = "#ff4040" }

        [responses]
        squelch = false
    "##;
    let mine = read(file).unwrap().triggers.for_character("Nisugi");
    assert_eq!(mine.len(), 1, "{mine:?}");
    assert_eq!(mine[0].name, "hide-and-colour");
    assert!(!mine[0].rule.squelch);
    assert!(mine[0].rule.look.is_some());
}

#[test]
fn triggers_come_in_category_then_name_order_whatever_the_file_says() {
    let file = r#"
        [trigger.zeta]
        category = "B"
        text = "1"
        squelch = true

        [trigger.alpha]
        category = "B"
        text = "2"
        squelch = true

        [trigger.omega]
        category = "A"
        text = "3"
        squelch = true
    "#;
    assert_eq!(names(file, "Nisugi").unwrap(), ["omega", "alpha", "zeta"]);
}

#[test]
fn a_section_the_file_does_not_have_is_refused_by_name() {
    let loaded = read(
        r#"
        [triger.typo]
        text = "x"

        [responses]
        squelch = "no"

        [trigger.kept]
        text = "y"
        squelch = true
        "#,
    )
    .unwrap();
    let refused: Vec<&str> = loaded.refused.iter().map(|r| r.name.as_str()).collect();
    assert_eq!(refused, ["responses", "triger"], "{:?}", loaded.refused);
    assert!(loaded.refused[0].why.contains("switches are all on"));
    // A bad `[responses]` leaves every response on, so the squelch stands.
    let kept = loaded.triggers.for_character("Nisugi");
    assert!(kept[0].rule.squelch);
}

#[test]
fn a_file_that_is_not_toml_loads_nothing_and_no_file_is_no_triggers() {
    assert!(triggers::read("[trigger.x\ntext = ").is_err());

    let dir: PathBuf = std::env::temp_dir().join(format!("cena-triggers-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    assert_eq!(triggers::load(&dir), Ok(Loaded::default()));

    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(triggers::path(&dir), "[trigger.x\n").unwrap();
    let error = triggers::load(&dir).unwrap_err();
    assert!(error.contains("triggers.toml is not TOML"), "{error}");
    let _ = std::fs::remove_dir_all(&dir);
}
