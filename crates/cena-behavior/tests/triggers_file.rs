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
    assert_eq!(
        rule.pattern,
        Some(Pattern::Literal {
            text: "You are stunned".into(),
            whole_word: true,
        })
    );
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
        Some(Pattern::Regex(r#"^(\w+) whispers, "(.*)"$"#.into()))
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

/// A literal matches whole words unless told otherwise (Wizard FE's "Not on
/// Word Boundary", `crates/cena-model/src/trigger.rs`).
#[test]
fn whole_word_is_the_default_and_can_be_turned_off() {
    let rule = only(
        "[trigger.send]\ntext = 'SEND['\nwhole_word = false\nsquelch = true\n",
        "Nisugi",
    )
    .unwrap()
    .rule;
    assert_eq!(
        rule.pattern,
        Some(Pattern::Literal {
            text: "SEND[".into(),
            whole_word: false,
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
        ("squelch = true", "nothing to fire on"),
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
        (
            "regex = 'x'\nwhole_word = false\nsquelch = true",
            "`whole_word` is for `text`",
        ),
    ];
    assert_eq!(missed(&cases), Vec::<String>::new());
}

/// Stage 2's keys, refused the same way: an event, a condition, `only_if`
/// and a flag that cannot work.
#[test]
fn a_stage_2_trigger_that_cannot_work_is_refused_by_name() {
    let cases = [
        (
            "event = 'shouting'\nsquelch = true",
            "not an event Hydra knows",
        ),
        (
            "event = 'incident nope'\nsquelch = true",
            "no incident Lich names",
        ),
        ("event = 'speech loud'\nsquelch = true", "takes no name"),
        (
            "event = 'speech'\ncase_sensitive = true\nsquelch = true",
            "`case_sensitive` is for `text` or `regex`",
        ),
        (
            "event = 'speech'\nlook = { bold = true, span = 1 }",
            "needs a `regex`",
        ),
        (
            "condition = 'hidden'\ntext = 'x'\nflag = { name = 'h' }",
            "watches the character, not a line",
        ),
        (
            "condition = 'hidden'\nsquelch = true",
            "no line to colour, hide, change or move",
        ),
        (
            "condition = ''\nflag = { name = 'h' }",
            "names no guard word",
        ),
        (
            "condition = 'hiden'\nflag = { name = 'h' }",
            "not a guard Hydra knows",
        ),
        (
            "text = 'x'\nrearm = 5\nsquelch = true",
            "`rearm` is for a `condition`",
        ),
        (
            "text = 'x'\nonly_if = 'once'\nsquelch = true",
            "reads what a hunt's routine sent",
        ),
        (
            "text = 'x'\nonly_if = '!splashy'\nsquelch = true",
            "reads the map",
        ),
        ("text = 'x'\nflag = { name = ' ' }", "flag has no name"),
        (
            "text = 'x'\nflag = { name = 'f', clear = true, seconds = 5 }",
            "has no `seconds`",
        ),
        (
            "text = 'x'\nflag = { name = 'f', seconds = 0 }",
            "sets nothing",
        ),
        ("condition = 'hidden'", "it does nothing"),
    ];
    assert_eq!(missed(&cases), Vec::<String>::new());
}

/// Stage 4's keys: where a trigger came from and what it holds belong to
/// the trigger, typed, and not to one character's copy.
#[test]
fn an_origin_or_held_that_cannot_be_read_is_refused_by_name() {
    let cases = [
        ("text = 'x'\nsquelch = true\norigin = 5", "`origin` is 5"),
        (
            "text = 'x'\nsquelch = true\nheld = 'sound'",
            "`held` is not a table",
        ),
        (
            "text = 'x'\nsquelch = true\nheld = { sound = 3 }",
            "`held` is not a table",
        ),
        (
            "text = 'x'\nsquelch = true\nfor = { Dicate = { origin = 'y' } }",
            "belong to the trigger",
        ),
    ];
    assert_eq!(missed(&cases), Vec::<String>::new());
    let kept = "[trigger.ok]\ntext = 'x'\nsquelch = true\norigin = 'Wrayth: a.xml'\n\
                held = { sound = 'C:\\\\a.wav' }\n";
    assert_eq!(names(kept, "Nisugi"), Some(vec!["ok".to_owned()]));
}

/// Each bad trigger, as `body`, beside a good one: the cases where it was
/// not refused by name for `reason`, or the good one did not load.
fn missed(cases: &[(&str, &str)]) -> Vec<String> {
    let mut missed = Vec::new();
    for &(body, reason) in cases {
        let file = format!("[trigger.bad]\n{body}\n\n[trigger.good]\ntext = 'y'\nsquelch = true\n");
        let Some(loaded) = read(&file) else {
            missed.push(format!("{body}: not TOML"));
            continue;
        };
        let kept: Vec<String> = loaded
            .triggers
            .for_character("Nisugi")
            .into_iter()
            .map(|trigger| trigger.name)
            .collect();
        let named = matches!(
            loaded.refused.as_slice(),
            [one] if one.name == "bad" && one.why.contains(reason)
        );
        if !named || kept != ["good"] {
            missed.push(format!("{body}: {:?}, kept {kept:?}", loaded.refused));
        }
    }
    missed
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
    assert_eq!(
        dicate.pattern,
        Some(Pattern::Literal {
            text: "You are stunned".into(),
            whole_word: true,
        })
    );
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

/// Stage 3's keys: a sound, a notification, a banner and a cooldown that
/// cannot work are refused by name; a condition may call for attention.
#[test]
fn attention_that_cannot_work_is_refused_by_name() {
    let cases = [
        ("text = 'x'\nsound = ''", "names no file"),
        ("text = 'x'\nnotify = false", "`false` says nothing"),
        ("text = 'x'\nalert = ' '", "says nothing"),
        (
            "text = 'x'\nsquelch = true\ncooldown = 5",
            "`cooldown` is for",
        ),
        (
            "condition = 'hidden'\nnotify = true\nsquelch = true",
            "no line to colour",
        ),
    ];
    assert_eq!(missed(&cases), Vec::<String>::new());
    let fine = "[trigger.hid]\ncondition = 'hidden'\nnotify = true\ncooldown = 10\n\
                [trigger.ding]\ntext = 'x'\nsound = 'ding.wav'\n";
    assert_eq!(
        names(fine, "Nisugi"),
        Some(vec!["ding".to_owned(), "hid".to_owned()])
    );
    // Every sound off leaves a trigger that only sounds with nothing to do.
    let off = format!("{fine}[responses]\nsound = false\n");
    assert_eq!(names(&off, "Nisugi"), Some(vec!["hid".to_owned()]));
}
