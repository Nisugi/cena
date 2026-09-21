//! `plan/25` step 2: the writer.
//!
//! These touch the filesystem, under a per-test temp directory keyed by name
//! so a parallel run cannot have two tests in one directory. `character_store`
//! learned that the hard way -- a fixed temp path plus `remove_dir_all` means
//! concurrent `cargo test` invocations delete each other's data.

use std::fs;
use std::path::PathBuf;

use cena_session::lifecycle::{Generation, SessionId};
use cena_session::player_log::writer::{self, PlayerWriter};
use cena_session::{LogLine, PlayerLog};

/// A directory of this test's own. Named, not shared, and never removed at
/// entry -- removing at entry is what makes two concurrent runs fight.
fn temp_dir(test: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("cena-player-log-{test}-{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    dir
}

fn line(stream: &str, text: &str) -> LogLine {
    LogLine {
        at: "06:47:12.481".to_owned(),
        stream: stream.to_owned(),
        text: text.to_owned(),
        session: SessionId::FIRST,
        generation: Generation::FIRST,
    }
}

/// The one file a writer has created, and its contents.
fn only_file(dir: &PathBuf, character: &str) -> (PathBuf, String) {
    let days = writer::days(dir, character).expect("read the log directory");
    assert_eq!(days.len(), 1, "expected exactly one day-file, got {days:?}");
    let text = fs::read_to_string(&days[0]).expect("read the day file");
    (days[0].clone(), text)
}

#[test]
fn a_line_lands_in_a_dated_file_for_its_character() {
    let dir = temp_dir("lands");
    {
        let mut w = PlayerWriter::new(&dir, "Nisugi");
        w.write(&line("main", "You see a rock.")).expect("write");
    } // dropped, which flushes

    let (path, text) = only_file(&dir, "Nisugi");

    let name = path.file_name().unwrap().to_string_lossy().into_owned();
    assert!(
        name.starts_with("Nisugi_") && name.ends_with(".log"),
        "the filename carries the character and the date: {name}"
    );
    assert_eq!(text, "[06:47:12.481][main] You see a rock.\n");
}

#[test]
fn the_written_line_is_the_format_the_reader_parses() {
    // One format, defined once. If `format_line` and `parse_line` ever
    // disagree, step 3's reader silently stops finding lines the writer wrote.
    let l = line("death", "The kobold falls to the ground and dies.");
    let rendered = writer::format_line(&l);

    let (at, stream, text) = writer::parse_line(&rendered).expect("our own line must parse");
    assert_eq!(at, "06:47:12.481");
    assert_eq!(stream, "death");
    assert_eq!(text, "The kobold falls to the ground and dies.");
}

#[test]
fn a_line_whose_text_holds_brackets_still_parses() {
    // Game text contains brackets -- room titles are `[Tower, Landing]`. The
    // parse must take the FIRST two bracket groups and leave the rest alone,
    // or every room title becomes a stream name.
    let l = line("main", "[Wehnimer's Landing, Town Square] is busy.");
    let rendered = writer::format_line(&l);

    let (at, stream, text) = writer::parse_line(&rendered).expect("parse");
    assert_eq!(at, "06:47:12.481");
    assert_eq!(stream, "main", "the room title was read as the stream");
    assert_eq!(text, "[Wehnimer's Landing, Town Square] is busy.");
}

#[test]
fn text_that_is_not_ours_does_not_parse() {
    // The reader must be able to skip a stray line -- a hand-edited file, a
    // partial write from a power cut -- rather than treat it as data.
    assert!(writer::parse_line("").is_none());
    assert!(writer::parse_line("no brackets at all").is_none());
    assert!(
        writer::parse_line("[06:47:12.481] only one group").is_none(),
        "one bracket group is not this format"
    );
}

#[test]
fn a_second_session_the_same_day_appends_rather_than_truncating() {
    // **The destructive failure.** Opening with truncate would erase the
    // morning's play when a character logs back in after lunch.
    let dir = temp_dir("append");
    {
        let mut w = PlayerWriter::new(&dir, "Nisugi");
        w.write(&line("main", "morning")).expect("write");
    }
    {
        let mut w = PlayerWriter::new(&dir, "Nisugi");
        w.write(&line("main", "afternoon")).expect("write");
    }

    let (_, text) = only_file(&dir, "Nisugi");
    assert!(text.contains("morning"), "the first session was erased");
    assert!(text.contains("afternoon"));
    assert_eq!(text.lines().count(), 2);
}

#[test]
fn a_registered_secret_never_reaches_the_disk() {
    // The launch key arrives as game text during login, and this file is kept
    // for years. `sink/writer.rs` redacts the wire log for the same reason;
    // a player log that did not would be the easier place to find a secret.
    let dir = temp_dir("redact");
    {
        let mut w = PlayerWriter::new(&dir, "Nisugi");
        w.redact("SUPERSECRETKEY", "<key>");
        w.write(&line("main", "key is SUPERSECRETKEY ok"))
            .expect("write");
    }

    let (_, text) = only_file(&dir, "Nisugi");
    assert!(
        !text.contains("SUPERSECRETKEY"),
        "the secret reached disk: {text}"
    );
    assert!(text.contains("<key>"), "and the replacement is there");
}

#[test]
fn a_secret_registered_after_the_file_is_open_still_redacts() {
    // **One log spans every generation, and each reconnect brings a NEW launch
    // key** (`sink/writer.rs:650`). A redaction set fixed at construction
    // would write generation 2's key in clear into a file opened during
    // generation 1.
    let dir = temp_dir("late-redact");
    {
        let mut w = PlayerWriter::new(&dir, "Nisugi");
        w.write(&line("main", "first generation")).expect("write");
        w.redact("SECONDKEY", "<key>");
        w.write(&line("main", "now SECONDKEY arrives"))
            .expect("write");
    }

    let (_, text) = only_file(&dir, "Nisugi");
    assert!(text.contains("first generation"), "guard: both lines wrote");
    assert!(!text.contains("SECONDKEY"), "the later key leaked: {text}");
}

#[test]
fn a_character_who_never_played_has_no_file() {
    // A writer that created its file eagerly would leave an empty log for
    // every character ever constructed -- a lie about which were used.
    let dir = temp_dir("never");
    {
        let _w = PlayerWriter::new(&dir, "Nisugi");
    }
    assert!(
        writer::days(&dir, "Nisugi").expect("read").is_empty(),
        "a file was created for a character that logged nothing"
    );
}

#[test]
fn listing_days_for_an_unknown_character_is_empty_not_an_error() {
    // "This character has no logs" is a fact, not a failure. Returning an
    // error would make every caller handle NotFound to display "no results".
    let dir = temp_dir("unknown");
    let days = writer::days(&dir, "Nobody").expect("a missing directory is not an error");
    assert!(days.is_empty());
}

#[test]
fn a_name_that_cannot_be_a_filename_does_not_escape_the_directory() {
    // A name of entirely forbidden characters must not produce a path outside
    // the feature's directory, and must not produce a bare `_DATE.log`.
    let dir = temp_dir("badname");
    {
        let mut w = PlayerWriter::new(&dir, "../../etc");
        w.write(&line("main", "nope")).expect("write");
    }
    let days = writer::days(&dir, "../../etc").expect("read");
    assert_eq!(days.len(), 1);
    assert!(
        days[0].starts_with(&dir),
        "the file escaped the log directory: {:?}",
        days[0]
    );
}

#[test]
fn a_flush_empties_the_buffer_count() {
    let dir = temp_dir("flush");
    let mut w = PlayerWriter::new(&dir, "Nisugi");
    w.write(&line("main", "one")).expect("write");
    assert_eq!(w.buffered(), 1, "guard: the line was buffered");
    w.flush().expect("flush");
    assert_eq!(w.buffered(), 0);
}

#[tokio::test]
async fn the_writer_drains_a_sink_and_flushes_at_the_end() {
    // The run loop: everything sent before the log was dropped is on disk
    // afterwards, including the tail that never reached a flush threshold.
    let dir = temp_dir("drain");
    let (log, sink) = PlayerLog::new();
    let w = PlayerWriter::new(&dir, "Nisugi");

    log.record(line("main", "first"));
    log.record(line("thoughts", "second"));
    drop(log);

    w.run(sink).await;

    let (_, text) = only_file(&dir, "Nisugi");
    assert!(text.contains("[main] first"));
    assert!(text.contains("[thoughts] second"));
    assert_eq!(text.lines().count(), 2, "the tail was not flushed");
}

#[tokio::test]
async fn a_write_failure_is_counted_rather_than_ending_the_run() {
    // `plan/12` §5.5: a session survives a failed log. The line is lost, the
    // loss is VISIBLE, and the writer keeps going -- which is exactly what the
    // reference implementation does not do (its error goes to console.error
    // and nowhere a player can see).
    //
    // The failure is forced by making the log directory a FILE, so
    // `create_dir_all` cannot succeed.
    let dir = temp_dir("failure");
    fs::create_dir_all(&dir).expect("make the root");
    fs::write(dir.join(writer::SUBDIR), b"not a directory").expect("block the path");

    let (log, sink) = PlayerLog::new();
    let w = PlayerWriter::new(&dir, "Nisugi");

    log.record(line("main", "this cannot be written"));
    assert_eq!(
        log.dropped(),
        0,
        "guard: the channel accepted it, so any loss below is the WRITER's"
    );

    // **Every sender must go before `run` can end.** A first draft kept a
    // `log.clone()` alive past this point so the counter could be read
    // afterwards -- which meant a sender still existed, so `sink.recv()` never
    // returned `None`, so `run` looped forever and the whole suite hung. The
    // count is readable through the sink's own handle, which `run` gives back.
    drop(log);

    // Completing at all is half the assertion: a writer that propagated the
    // error would end the loop here rather than drain.
    let dropped = w.run_reporting(sink).await;

    assert_eq!(
        dropped, 1,
        "a write failure must reach the same counter a channel drop does"
    );
}

#[test]
fn a_name_of_nothing_but_forbidden_characters_still_gets_a_file() {
    // **The input is the point.** `a_name_that_cannot_be_a_filename...` uses
    // `../../etc`, which keeps `etc` after filtering -- so it never reaches
    // the empty case, and a mutant returning `String::new()` from `safe_name`
    // passed all thirteen other tests.
    //
    // Every character here is stripped. Without the fallback the file becomes
    // `_2026-09-21.log` inside a directory named "", which on Windows resolves
    // to the PARENT -- a log written outside the feature's own tree.
    let dir = temp_dir("allbad");
    {
        let mut w = PlayerWriter::new(&dir, r#"//\::??"#);
        w.write(&line("main", "still logged")).expect("write");
    }

    let days = writer::days(&dir, r#"//\::??"#).expect("read");
    assert_eq!(days.len(), 1, "no file was written for an unusable name");

    let name = days[0].file_name().unwrap().to_string_lossy().into_owned();
    assert!(
        name.starts_with("unnamed_"),
        "expected the fallback name, got {name}"
    );
    assert!(
        days[0].parent().unwrap().starts_with(&dir),
        "the file escaped the log directory: {:?}",
        days[0]
    );
}
