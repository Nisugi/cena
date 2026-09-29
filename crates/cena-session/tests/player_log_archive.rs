//! `plan/25` step 4: where the writer cuts a day, and the archives.
//!
//! September 2026: the 1st is a Tuesday, the Sundays are 6, 13, 20 and 27,
//! and October begins on a Thursday. Eastern is on daylight time throughout.

use std::fs;
use std::path::{Path, PathBuf};

use cena_platform::eastern::{self, HOUR};
use cena_session::player_log::archive::{self, Archive};
use cena_session::player_log::reader;
use cena_session::player_log::writer::{self, file_key};

fn temp_dir(test: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "cena-player-log-archive-{test}-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&dir);
    dir
}

/// Unix time at `hour` o'clock Eastern on `date`.
fn eastern_at(date: eastern::Date, hour: i64) -> i64 {
    eastern::midnight(date) + hour * HOUR
}

/// Write Nisugi's file for `key` holding one line per text.
fn file(root: &Path, key: &str, texts: &[&str]) -> std::io::Result<()> {
    let path = writer::day_path(root, "Nisugi", key);
    fs::create_dir_all(path.parent().unwrap_or(root))?;
    let body: String = texts
        .iter()
        .map(|t| format!("[12:00:00.000][main] {t}\n"))
        .collect();
    fs::write(path, body)
}

fn texts(root: &Path, day: &str) -> Vec<String> {
    reader::read_day(root, "Nisugi", day)
        .expect("read")
        .into_iter()
        .map(|e| e.text)
        .collect()
}

#[test]
fn on_eastern_time_a_file_is_just_its_date() {
    // The two clocks agree, so the cut at a week's start is local midnight.
    let sunday = eastern_at((2026, 9, 27), 0) + 60;
    assert_eq!(file_key("2026-09-27", sunday), "2026-09-27");
    assert_eq!(file_key("2026-09-26", sunday - 120), "2026-09-26");
}

#[test]
fn a_pacific_saturday_is_cut_where_sunday_begins_in_eastern() {
    // 21:30 Saturday in Pacific is 00:30 Sunday in Eastern: a new week.
    let late = eastern_at((2026, 9, 27), 0) + 30 * 60;
    assert_eq!(file_key("2026-09-26", late), "2026-09-26_2026-09-27");
    // An hour earlier it is still Saturday in both.
    assert_eq!(file_key("2026-09-26", late - HOUR), "2026-09-26");
    // And on a Tuesday night nothing begins, so nothing is cut.
    let tuesday = eastern_at((2026, 9, 23), 0) + 30 * 60;
    assert_eq!(file_key("2026-09-22", tuesday), "2026-09-22");
}

#[test]
fn a_london_sunday_starts_in_the_eastern_week_before() {
    // 01:00 Sunday in London is 20:00 Saturday in Eastern: last week still.
    let small_hours = eastern_at((2026, 9, 26), 20);
    assert_eq!(file_key("2026-09-27", small_hours), "2026-09-27_2026-09-20");
    // From 05:00 in London it is Sunday in Eastern too.
    assert_eq!(file_key("2026-09-27", small_hours + 4 * HOUR), "2026-09-27");
}

#[test]
fn a_month_that_turns_mid_week_is_a_cut_too() {
    // October begins on a Thursday.
    let october = eastern_at((2026, 10, 1), 0) + 60;
    assert_eq!(file_key("2026-09-30", october), "2026-09-30_2026-10-01");
}

#[test]
fn a_files_name_says_its_day_and_its_stretch() {
    assert_eq!(
        writer::piece("nisugi_2026-09-26_2026-09-27.log"),
        Some(("2026-09-26".to_owned(), (2026, 9, 27)))
    );
    assert_eq!(
        writer::piece("nisugi_2026-09-30.log"),
        Some(("2026-09-30".to_owned(), (2026, 9, 27))),
        "a plain file holds its date's own stretch"
    );
    assert_eq!(
        writer::piece("con__2026-09-30.log").map(|(day, _)| day),
        Some("2026-09-30".to_owned()),
        "a device name's `_` is not a date"
    );
    assert_eq!(writer::piece("nisugi_2026-09.log.gz"), None);
    assert_eq!(writer::piece("notes.log"), None);
}

#[test]
fn a_closed_month_becomes_one_archive_and_reads_back_whole() {
    let root = temp_dir("monthly");
    file(&root, "2026-09-05", &["early september"]).expect("write");
    file(&root, "2026-09-30", &["the last evening of september"]).expect("write");
    // A Pacific player's night of the 30th, past October's start in Eastern.
    file(
        &root,
        "2026-09-30_2026-10-01",
        &["already october in eastern"],
    )
    .expect("write");
    file(&root, "2026-10-02", &["today"]).expect("write");

    let now = eastern_at((2026, 10, 2), 12);
    let swept = archive::sweep(&root, "Nisugi", Archive::Monthly, now).expect("sweep");
    assert_eq!(swept.files, 2, "{swept:?}");
    assert_eq!(swept.archives, ["nisugi_2026-09.log.gz"]);

    let left: Vec<String> = writer::days(&root, "Nisugi")
        .expect("list")
        .iter()
        .filter_map(|p| p.file_name()?.to_str().map(str::to_owned))
        .collect();
    assert_eq!(
        left,
        ["nisugi_2026-10-02.log", "nisugi_2026-09-30_2026-10-01.log"],
        "October's files, and the piece of the 30th that is October's, stay"
    );

    // The day reads the same, one part archived and one not, in order.
    assert_eq!(
        texts(&root, "2026-09-30"),
        [
            "the last evening of september",
            "already october in eastern"
        ]
    );
    assert_eq!(texts(&root, "2026-09-05"), ["early september"]);
    assert_eq!(
        reader::days(&root, "Nisugi").expect("days"),
        ["2026-10-02", "2026-09-30", "2026-09-05"]
    );

    // Search reaches into the archive.
    let found = reader::search(
        &root,
        "Nisugi",
        &reader::Pattern::literal("september").expect("a literal"),
        &reader::Streams::all(),
        None,
    )
    .expect("search");
    assert_eq!(found.entries.len(), 2);
}

#[test]
fn a_london_mornings_first_hours_go_with_the_week_before() {
    // The small hours of Sunday the 27th in London were Saturday in Eastern:
    // they belong to the week of the 20th, archived without the rest of the day.
    let root = temp_dir("london");
    file(&root, "2026-09-27_2026-09-20", &["still last week"]).expect("write");
    file(&root, "2026-09-27", &["this week"]).expect("write");

    let now = eastern_at((2026, 9, 28), 12);
    let swept = archive::sweep(&root, "Nisugi", Archive::Weekly, now).expect("sweep");
    assert_eq!(swept.archives, ["nisugi_week-2026-09-20.log.gz"]);
    assert_eq!(texts(&root, "2026-09-27"), ["still last week", "this week"]);
}

#[test]
fn off_archives_nothing_and_the_current_period_is_never_touched() {
    let root = temp_dir("off");
    file(&root, "2026-08-10", &["august"]).expect("write");
    file(&root, "2026-09-29", &["this month"]).expect("write");
    let now = eastern_at((2026, 9, 29), 12);

    let swept = archive::sweep(&root, "Nisugi", Archive::Off, now).expect("sweep");
    assert_eq!(swept, archive::Swept::default());
    assert_eq!(writer::days(&root, "Nisugi").expect("list").len(), 2);

    let swept = archive::sweep(&root, "Nisugi", Archive::Monthly, now).expect("sweep");
    assert_eq!(swept.archives, ["nisugi_2026-08.log.gz"]);
    assert_eq!(texts(&root, "2026-09-29"), ["this month"]);
}

#[test]
fn a_late_file_is_added_to_its_archive_keeping_what_was_there() {
    // A day-file that turns up after its month was archived (a setting turned
    // back on, a file copied in) joins the archive; nothing already in it is lost.
    let root = temp_dir("late");
    file(&root, "2026-08-10", &["first"]).expect("write");
    let now = eastern_at((2026, 9, 29), 12);
    archive::sweep(&root, "Nisugi", Archive::Monthly, now).expect("sweep");
    file(&root, "2026-08-20", &["second"]).expect("write");
    archive::sweep(&root, "Nisugi", Archive::Monthly, now).expect("sweep");

    let archived = archive::path(&root, "Nisugi", "2026-08");
    assert_eq!(
        archive::names(&archived).expect("manifest"),
        ["nisugi_2026-08-10.log", "nisugi_2026-08-20.log"]
    );
    assert_eq!(texts(&root, "2026-08-10"), ["first"]);
    assert_eq!(texts(&root, "2026-08-20"), ["second"]);
}

#[test]
fn a_plain_file_wins_over_the_same_name_in_an_archive() {
    // Both exist only between an archive's rename and the day-files' removal.
    let root = temp_dir("both");
    file(&root, "2026-08-10", &["archived copy"]).expect("write");
    let now = eastern_at((2026, 9, 29), 12);
    archive::sweep(&root, "Nisugi", Archive::Monthly, now).expect("sweep");
    file(&root, "2026-08-10", &["plain copy"]).expect("write");

    assert_eq!(texts(&root, "2026-08-10"), ["plain copy"]);
}

#[test]
fn an_archive_is_readable_by_zcat_as_the_days_in_turn() {
    // A player can still grep an archive by hand: every member inflated, one
    // after another, is the manifest and then each day-file whole.
    use std::io::Read;
    let root = temp_dir("zcat");
    file(&root, "2026-08-10", &["a rock"]).expect("write");
    let now = eastern_at((2026, 9, 29), 12);
    archive::sweep(&root, "Nisugi", Archive::Monthly, now).expect("sweep");

    let mut all = String::new();
    flate2::read::MultiGzDecoder::new(
        fs::File::open(archive::path(&root, "Nisugi", "2026-08")).expect("open"),
    )
    .read_to_string(&mut all)
    .expect("inflate");
    assert!(all.ends_with("[12:00:00.000][main] a rock\n"), "{all:?}");
}
