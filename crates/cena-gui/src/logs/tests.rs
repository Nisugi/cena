//! The log window: what it reads, what it shows, and what it asks.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use cena_session::player_log::reader::{Entry, Streams};
use cena_session::player_log::writer;
use egui_kittest::Harness;
use egui_kittest::kittest::Queryable;

use super::{Ask, Logs, Preset, Reply, run, shown};

fn temp_dir(test: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("cena-gui-logs-{test}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    dir
}

/// Write Nisugi's file for `key` with these `(tag, text)` lines.
fn file(root: &Path, key: &str, lines: &[(&str, &str)]) -> std::io::Result<()> {
    let path = writer::day_path(root, "Nisugi", key);
    std::fs::create_dir_all(path.parent().unwrap_or(root))?;
    let mut body = String::new();
    for (tag, text) in lines {
        body.push_str("[19:07:05.000][");
        body.push_str(tag);
        body.push_str("] ");
        body.push_str(text);
        body.push('\n');
    }
    std::fs::write(path, body)
}

fn entry(tag: &str, text: &str) -> Entry {
    Entry {
        day: "2026-09-20".to_owned(),
        at: "19:07:05.000".to_owned(),
        stream: tag.to_owned(),
        text: text.to_owned(),
    }
}

#[test]
fn a_line_is_shown_by_its_class_and_dedup_folds_a_run() {
    let lines = [
        entry("main", "R>"),
        entry("main", "R>"),
        entry("main/combat", "You bull-rush an ice archon."),
        entry("spells", "Piranha (82 roisaen)"),
    ];
    let ticked: BTreeMap<String, bool> = [("combat".to_owned(), false)].into();

    let plain = shown(&lines, &ticked, false);
    assert_eq!(
        plain.len(),
        3,
        "combat unticked; main and spells, unheard of, shown"
    );

    let folded = shown(&lines, &BTreeMap::new(), true);
    assert_eq!(folded.len(), 3);
    assert_eq!((folded[0].0.text.as_str(), folded[0].1), ("R>", 2));
}

#[test]
fn presets_tick_the_classes_they_name() {
    assert!(Preset::Combat.ticks("combat") && !Preset::Combat.ticks("main"));
    assert!(Preset::Social.ticks("thoughts") && !Preset::Social.ticks("combat"));
    assert!(Preset::Quiet.ticks("main") && !Preset::Quiet.ticks("combat"));
    assert!(!Preset::Quiet.ticks("atmospherics"));
    assert!(Preset::Everything.ticks("anything"));
}

#[test]
fn the_reads_answer_from_the_log_on_disk() {
    let root = temp_dir("run");
    file(
        &root,
        "2026-09-20",
        &[
            ("main", "You feel fully rested."),
            ("main/combat", "You bull-rush an ice archon."),
        ],
    )
    .expect("write");

    let Reply::Days(Ok((days, usage))) = run(&root, "Nisugi", Ask::Days) else {
        panic!("days");
    };
    assert_eq!(days.len(), 1);
    assert!(days[0].1.is_some(), "a plain day has a size");
    assert_eq!(usage.days, 1);

    let Reply::Day(_, Ok(lines)) = run(&root, "Nisugi", Ask::Day("2026-09-20".to_owned())) else {
        panic!("a day");
    };
    assert_eq!(lines.len(), 2);

    let Reply::Found(Ok(found)) = run(&root, "Nisugi", Ask::Search("ARCHON".to_owned(), false))
    else {
        panic!("a search");
    };
    assert_eq!(found.entries.len(), 1);
    assert!(matches!(
        run(&root, "Nisugi", Ask::Search("(".to_owned(), true)),
        Reply::Found(Err(_))
    ));

    // An export of the window's ticks: `main` by class is the story without
    // its combat.
    let Reply::Exported(Ok(done)) = run(
        &root,
        "Nisugi",
        Ask::Export(
            "2026-09-20".to_owned(),
            "2026-09-20".to_owned(),
            Streams::classes(["main"]),
        ),
    ) else {
        panic!("an export");
    };
    assert_eq!(done.lines, 1);
    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn a_window_asks_for_its_days_then_the_newest_day() {
    let mut logs = Logs::new("Nisugi");
    assert_eq!(logs.take_asks(), [Ask::Days]);
    logs.answer(Reply::Days(Ok((
        vec![
            ("2026-09-21".to_owned(), Some(2048)),
            ("2026-09-20".to_owned(), None),
        ],
        cena_session::player_log::archive::Usage::default(),
    ))));
    assert_eq!(logs.take_asks(), [Ask::Day("2026-09-21".to_owned())]);
    // The picker says each day's size, or that it is archived.
    assert_eq!(logs.day_label("2026-09-21"), "2026-09-21 · 2 KB");
    assert_eq!(logs.day_label("2026-09-20"), "2026-09-20 · archived");
    assert_eq!(
        (logs.from.as_str(), logs.to.as_str()),
        ("2026-09-21", "2026-09-21")
    );

    logs.answer(Reply::Day(
        "2026-09-21".to_owned(),
        Ok(vec![entry("main", "hello"), entry("main/combat", "swing")]),
    ));
    assert_eq!(logs.lines.len(), 2);
    assert_eq!(logs.ticked.keys().collect::<Vec<_>>(), ["combat", "main"]);
    // The answer for a day no longer chosen is dropped.
    logs.answer(Reply::Day(
        "2026-09-20".to_owned(),
        Ok(vec![entry("main", "stale")]),
    ));
    assert_eq!(logs.lines.len(), 2);

    logs.preset(Preset::Combat);
    assert_eq!(logs.export_streams(), Streams::classes(["combat"]));
    logs.preset(Preset::Everything);
    assert_eq!(logs.export_streams(), Streams::all());
}

#[test]
fn the_window_draws_a_day_and_a_preset_hides_what_it_does_not_tick() {
    let mut logs = Logs::new("Nisugi");
    // Answered through the inbox, as a read answers: taking a reply is what
    // counts it as no longer waiting, which stops the spinner.
    let inbox = logs.inbox();
    let _ = logs.take_asks();
    crate::sessions::lock(&inbox).push(Reply::Days(Ok((
        vec![("2026-09-20".to_owned(), Some(9850 * 1024))],
        cena_session::player_log::archive::Usage::default(),
    ))));
    logs.take_replies();
    let _ = logs.take_asks();
    crate::sessions::lock(&inbox).push(Reply::Day(
        "2026-09-20".to_owned(),
        Ok(vec![
            entry("main", "You feel fully rested."),
            entry("main/combat", "You bull-rush an ice archon."),
        ]),
    ));
    let mut harness = Harness::builder()
        .with_size((1000.0, 640.0))
        .build_ui_state(|ui, logs: &mut Logs| logs.show(ui), logs);
    harness.run();
    for label in [
        "Recent",
        "You feel fully rested.",
        "You bull-rush an ice archon.",
    ] {
        assert!(harness.query_by_label(label).is_some(), "{label}");
    }
    harness.get_by_label("Combat").click();
    harness.run();
    assert!(harness.query_by_label("You feel fully rested.").is_none());
    assert!(
        harness
            .query_by_label("You bull-rush an ice archon.")
            .is_some()
    );
}
