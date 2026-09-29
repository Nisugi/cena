//! `plan/25` step 3: the reader.
//!
//! Day-files are written by hand, under dates chosen here, because the writer
//! takes its day from the wall clock and a suite cannot cross midnight. The
//! one test that needs the writer itself is the flush, and it runs the real
//! writer task.

use std::fs;
use std::path::{Path, PathBuf};

use cena_session::lifecycle::{Generation, SessionId};
use cena_session::player_log::reader::{self, MAX_HITS, Moment, Pattern, Streams};
use cena_session::player_log::writer::{self, PlayerWriter};
use cena_session::{LogLine, PlayerLog};

/// A directory of this test's own; see `player_log_writer.rs` on why.
fn temp_dir(test: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "cena-player-log-reader-{test}-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&dir);
    dir
}

/// Write `lines` as `day`'s file for Nisugi, each ended by a newline.
fn day(root: &Path, day: &str, lines: &[&str]) -> std::io::Result<()> {
    let path = writer::day_path(root, "Nisugi", day);
    fs::create_dir_all(path.parent().unwrap_or(root))?;
    let mut text = lines.join("\n");
    text.push('\n');
    fs::write(path, text)
}

fn texts(entries: &[reader::Entry]) -> Vec<&str> {
    entries.iter().map(|e| e.text.as_str()).collect()
}

#[test]
fn days_are_listed_newest_first_and_other_files_are_not_days() {
    let root = temp_dir("days");
    day(&root, "2026-09-20", &["[01:00:00.000][main] a"]).expect("write");
    day(&root, "2026-09-21", &["[01:00:00.000][main] b"]).expect("write");
    let dir = writer::dir(&root, "Nisugi");
    fs::write(dir.join("notes.log"), "not a day\n").expect("write");
    fs::write(dir.join("nisugi_2026-09-22.txt"), "not a log\n").expect("write");

    assert_eq!(
        reader::days(&root, "Nisugi").expect("list"),
        ["2026-09-21", "2026-09-20"]
    );
    assert!(reader::days(&root, "Nobody").expect("list").is_empty());
}

#[test]
fn a_half_written_last_line_and_a_stray_line_are_not_read() {
    // The writer ends every line with a newline, so a last line without one
    // is still being written; read, it would show half a sentence as whole.
    let root = temp_dir("partial");
    let path = writer::day_path(&root, "Nisugi", "2026-09-21");
    fs::create_dir_all(path.parent().expect("a parent")).expect("mkdir");
    fs::write(
        &path,
        "[06:00:00.000][main] whole\nhand-edited junk\n[06:00:01.000][main] half a li",
    )
    .expect("write");

    let entries = reader::read_day(&root, "Nisugi", "2026-09-21").expect("read");
    assert_eq!(texts(&entries), ["whole"]);
    assert_eq!(entries[0].day, "2026-09-21");
    assert!(
        reader::read_day(&root, "Nisugi", "2026-01-01")
            .expect("a missing day is no lines, not an error")
            .is_empty()
    );
}

#[test]
fn the_tail_reaches_back_across_days_and_keeps_order() {
    let root = temp_dir("tail");
    day(
        &root,
        "2026-09-20",
        &["[23:59:58.000][main] one", "[23:59:59.000][thoughts] two"],
    )
    .expect("write");
    day(
        &root,
        "2026-09-21",
        &["[00:00:01.000][main] three", "[00:00:02.000][main] four"],
    )
    .expect("write");

    let all = reader::tail(&root, "Nisugi", 3, &Streams::all()).expect("tail");
    assert_eq!(texts(&all), ["two", "three", "four"]);

    let thoughts = reader::tail(&root, "Nisugi", 3, &Streams::only(["thoughts"])).expect("tail");
    assert_eq!(
        texts(&thoughts),
        ["two"],
        "the filter applies before the count"
    );
}

#[test]
fn a_window_is_chosen_by_stamp_across_midnight_and_excludes_its_end() {
    // Lichborne's pitfall #92: a window approximated by a line count reached
    // back 6 of 11 minutes on a busy character. The busy minute here holds
    // more lines than the whole window asked for elsewhere.
    let root = temp_dir("window");
    let mut busy = vec!["[23:50:00.000][main] before"];
    busy.extend(std::iter::repeat_n("[23:58:30.000][main] busy", 50));
    busy.push("[23:59:00.000][main] in, yesterday");
    day(&root, "2026-09-20", &busy).expect("write");
    day(
        &root,
        "2026-09-21",
        &[
            "[00:01:00.000][main] in, today",
            "[00:02:00.000][main] the end, excluded",
        ],
    )
    .expect("write");

    let found = reader::window(
        &root,
        "Nisugi",
        &Moment::new("2026-09-20", "23:58"),
        &Moment::new("2026-09-21", "00:02"),
        &Streams::all(),
    )
    .expect("window");
    assert!(!found.more);
    assert_eq!(found.entries.len(), 52);
    assert_eq!(found.entries[0].text, "busy");
    assert_eq!(
        texts(&found.entries[50..]),
        ["in, yesterday", "in, today"],
        "in time order, the end excluded, the line before the start left out"
    );
}

#[test]
fn a_window_stops_at_the_cap_and_says_there_was_more() {
    let root = temp_dir("window-cap");
    let lines: Vec<String> = (0..=MAX_HITS)
        .map(|i| format!("[06:00:00.000][main] line {i}"))
        .collect();
    let lines: Vec<&str> = lines.iter().map(String::as_str).collect();
    day(&root, "2026-09-21", &lines).expect("write");

    let found = reader::window(
        &root,
        "Nisugi",
        &Moment::new("2026-09-21", "00"),
        &Moment::new("2026-09-22", "00"),
        &Streams::all(),
    )
    .expect("window");
    assert_eq!(found.entries.len(), MAX_HITS);
    assert!(found.more, "a capped read must say it was capped");
    assert_eq!(
        found.entries[0].text, "line 0",
        "the first of them, in order"
    );
}

#[test]
fn search_finds_text_ignoring_case_newest_first() {
    let root = temp_dir("search");
    day(
        &root,
        "2026-09-20",
        &[
            "[06:00:00.000][main] A Kobold attacks!",
            "[06:00:01.000][main] [Wehnimer's Landing, Town Square]",
        ],
    )
    .expect("write");
    day(
        &root,
        "2026-09-21",
        &[
            "[07:00:00.000][main/combat] You swing at a kobold.",
            "[07:00:01.000][thoughts] [General]-GSIV:Someone: \"kobolds!\"",
        ],
    )
    .expect("write");

    let pattern = Pattern::literal("kobold").expect("a literal");
    let found = reader::search(&root, "Nisugi", &pattern, &Streams::all(), None).expect("search");
    assert_eq!(
        texts(&found.entries),
        [
            "[General]-GSIV:Someone: \"kobolds!\"",
            "You swing at a kobold.",
            "A Kobold attacks!"
        ]
    );

    // A bracket in the text is text: the room title is found as written.
    let title = Pattern::literal("[wehnimer's").expect("a literal");
    let found = reader::search(&root, "Nisugi", &title, &Streams::all(), None).expect("search");
    assert_eq!(found.entries.len(), 1);
    assert_eq!(found.entries[0].stream, "main");
}

#[test]
fn a_stream_is_matched_by_either_half_of_its_tag() {
    // `main/combat` is where the game sent it and what our definitions made
    // of it (`plan/25` step 2b). Asking for `main` must not lose combat, and
    // asking for `combat` must not bring back the rest of `main`.
    assert!(Streams::only(["main"]).admits("main/combat"));
    assert!(Streams::only(["main"]).admits("main"));
    assert!(Streams::only(["combat"]).admits("main/combat"));
    assert!(!Streams::only(["combat"]).admits("main"));
    assert!(
        Streams::only(["spells"]).admits("Spells"),
        "case does not matter"
    );
    assert!(
        !Streams::only(["mai"]).admits("main"),
        "a part is whole, not a prefix"
    );
    assert!(Streams::all().admits("anything"));
    // By class, the tag's last part alone: the log window's ticks.
    assert!(!Streams::classes(["main"]).admits("main/combat"));
    assert!(Streams::classes(["main"]).admits("main"));
    assert!(Streams::classes(["combat"]).admits("main/combat"));
}

#[test]
fn a_regex_searches_as_written_and_a_bad_one_is_told() {
    let root = temp_dir("regex");
    day(
        &root,
        "2026-09-21",
        &[
            "[06:00:00.000][main] You gain 12 experience.",
            "[06:00:01.000][main] you gain nothing.",
        ],
    )
    .expect("write");

    let pattern = Pattern::regex(r"^You gain \d+").expect("an expression");
    let found = reader::search(&root, "Nisugi", &pattern, &Streams::all(), None).expect("search");
    assert_eq!(texts(&found.entries), ["You gain 12 experience."]);

    assert!(Pattern::regex("(unclosed").is_err());
}

#[test]
fn a_search_since_a_day_leaves_the_days_before_it() {
    let root = temp_dir("since");
    day(&root, "2026-09-20", &["[06:00:00.000][main] old gem"]).expect("write");
    day(&root, "2026-09-21", &["[06:00:00.000][main] new gem"]).expect("write");

    let pattern = Pattern::literal("gem").expect("a literal");
    let found = reader::search(
        &root,
        "Nisugi",
        &pattern,
        &Streams::all(),
        Some("2026-09-21"),
    )
    .expect("search");
    assert_eq!(texts(&found.entries), ["new gem"]);
}

#[test]
fn a_search_stops_at_the_cap_and_says_there_was_more() {
    let root = temp_dir("search-cap");
    let lines: Vec<String> = (0..=MAX_HITS)
        .map(|i| format!("[06:00:00.000][main] gem {i}"))
        .collect();
    let lines: Vec<&str> = lines.iter().map(String::as_str).collect();
    day(&root, "2026-09-21", &lines).expect("write");

    let pattern = Pattern::literal("gem").expect("a literal");
    let found = reader::search(&root, "Nisugi", &pattern, &Streams::all(), None).expect("search");
    assert_eq!(found.entries.len(), MAX_HITS);
    assert!(found.more);
    assert_eq!(
        found.entries[0].text,
        format!("gem {MAX_HITS}"),
        "newest first"
    );
}

/// **Reads flush the writer first** (`plan/25` §5), or a search of the live
/// session misses its last second. Time is paused, so the writer's own 1 s
/// timer cannot be what put the line on disk: were the flush request ignored,
/// `flush` would wait out its timeout, time would jump, the timer would fire,
/// and `flush` would still answer `false`.
#[tokio::test(start_paused = true)]
async fn a_flush_puts_what_was_recorded_on_disk_before_the_read() {
    let root = temp_dir("flush");
    let (log, sink) = PlayerLog::new();
    let task = tokio::spawn(PlayerWriter::new(&root, "Nisugi").run(sink));

    log.record(LogLine {
        at: "06:47:12.481".to_owned(),
        stream: "main".to_owned(),
        text: "the very last thing".to_owned(),
        session: SessionId::FIRST,
        generation: Generation::FIRST,
    });
    assert!(log.flush().await, "the writer answers a flush");

    let today = reader::days(&root, "Nisugi").expect("list");
    assert_eq!(today.len(), 1);
    let entries = reader::read_day(&root, "Nisugi", &today[0]).expect("read");
    assert_eq!(texts(&entries), ["the very last thing"]);

    drop(log);
    task.await.expect("the writer ends once the log is dropped");
}

#[tokio::test(start_paused = true)]
async fn a_flush_with_no_writer_says_so_rather_than_waiting_for_ever() {
    let (log, sink) = PlayerLog::new();
    drop(sink);
    assert!(!log.flush().await);
}
