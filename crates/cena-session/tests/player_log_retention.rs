//! `plan/25` steps 5 and 6: how long the log is kept, and export.

use std::fs;
use std::path::{Path, PathBuf};

use cena_platform::eastern;
use cena_session::player_log::archive::{self, Archive};
use cena_session::player_log::reader::{self, Streams};
use cena_session::player_log::retention;
use cena_session::player_log::writer;

fn temp_dir(test: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "cena-player-log-retention-{test}-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&dir);
    dir
}

/// Write Nisugi's file for `key` with these `(tag, text)` lines.
fn file(root: &Path, key: &str, lines: &[(&str, &str)]) -> std::io::Result<()> {
    let path = writer::day_path(root, "Nisugi", key);
    fs::create_dir_all(path.parent().unwrap_or(root))?;
    let mut body = String::new();
    for (tag, text) in lines {
        body.push_str("[12:00:00.000][");
        body.push_str(tag);
        body.push_str("] ");
        body.push_str(text);
        body.push('\n');
    }
    fs::write(path, body)
}

#[test]
fn keeping_days_counts_today_and_zero_is_forever() {
    assert_eq!(
        retention::first_kept("2026-09-29", 1).as_deref(),
        Some("2026-09-29")
    );
    assert_eq!(
        retention::first_kept("2026-09-29", 30).as_deref(),
        Some("2026-08-31")
    );
    assert_eq!(retention::first_kept("2026-09-29", 0), None);
}

#[test]
fn what_goes_is_previewed_and_the_preview_is_what_goes() {
    let root = temp_dir("prune");
    file(&root, "2026-06-10", &[("main", "june")]).expect("write");
    file(&root, "2026-08-31", &[("main", "the last of august")]).expect("write");
    file(&root, "2026-09-28", &[("main", "yesterday")]).expect("write");
    // June goes into an archive; August stays plain (the sweep is monthly and
    // runs in August's own month here, so only June is closed).
    archive::sweep(
        &root,
        "Nisugi",
        Archive::Monthly,
        eastern::midnight((2026, 8, 31)) + 3600,
    )
    .expect("sweep");

    let doomed = retention::doomed(&root, "Nisugi", 30, "2026-09-29").expect("preview");
    let names: Vec<String> = doomed
        .iter()
        .filter_map(|d| d.path.file_name()?.to_str().map(str::to_owned))
        .collect();
    assert_eq!(
        names,
        ["nisugi_2026-06.log.gz"],
        "the 31st of August is the first day kept of thirty"
    );
    assert!(retention::preview(&doomed).contains("1 file"), "{doomed:?}");

    let gone = retention::prune(&root, "Nisugi", 30, "2026-09-29").expect("prune");
    assert_eq!(gone, doomed, "what was shown is what went");
    assert_eq!(
        reader::days(&root, "Nisugi").expect("days"),
        ["2026-09-28", "2026-08-31"]
    );
    assert!(
        retention::prune(&root, "Nisugi", 0, "2026-09-29")
            .expect("forever")
            .is_empty()
    );
}

#[test]
fn an_archive_goes_only_when_every_day_in_it_is_old_enough() {
    let root = temp_dir("whole");
    file(&root, "2026-08-01", &[("main", "early")]).expect("write");
    file(&root, "2026-08-30", &[("main", "late")]).expect("write");
    archive::sweep(
        &root,
        "Nisugi",
        Archive::Monthly,
        eastern::midnight((2026, 9, 2)),
    )
    .expect("sweep");

    // Keeping 30 days on the 29th keeps the 31st of August on: the archive's
    // newest day, the 30th, is older, so it goes whole.
    assert_eq!(
        retention::doomed(&root, "Nisugi", 30, "2026-09-29")
            .expect("preview")
            .len(),
        1
    );
    // Keeping 31 keeps the 30th, so none of August's archive goes.
    assert!(
        retention::doomed(&root, "Nisugi", 31, "2026-09-29")
            .expect("preview")
            .is_empty()
    );
}

#[test]
fn an_export_writes_the_days_and_tags_asked_for_with_each_lines_day() {
    let root = temp_dir("export");
    file(
        &root,
        "2026-09-26",
        &[("main", "a rock"), ("thoughts", "hello all")],
    )
    .expect("write");
    file(
        &root,
        "2026-09-26_2026-09-27",
        &[("main/combat", "you swing")],
    )
    .expect("write");
    file(&root, "2026-09-28", &[("main", "outside the range")]).expect("write");

    let out = reader::export_path(&root, "Nisugi", ("2026-09-26", "2026-09-27"));
    let done = reader::export(
        &root,
        "Nisugi",
        ("2026-09-26", "2026-09-27"),
        &Streams::only(["main"]),
        &out,
    )
    .expect("export");
    assert_eq!((done.lines, done.days), (2, 1));
    assert_eq!(
        fs::read_to_string(&out).expect("the file"),
        "2026-09-26 12:00:00.000 [main] a rock\n2026-09-26 12:00:00.000 [main/combat] you swing\n"
    );
    assert!(
        out.starts_with(writer::dir(&root, "Nisugi").join("exports")),
        "{}",
        out.display()
    );
    // An export is not read back as a day of the log.
    assert_eq!(reader::days(&root, "Nisugi").expect("days").len(), 2);
}
