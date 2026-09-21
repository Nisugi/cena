//! The player log's writer: **`plan/25` step 2**.
//!
//! Takes lines off [`LogSink`](super::LogSink) and puts them on disk, one file
//! per character per day.
//!
//! # The shape, and where it comes from
//!
//! ```text
//! <log_dir>/player/<Character>/<Character>_2026-09-21.log
//! [06:47:12.481][main] You see a rock.
//! ```
//!
//! Both halves are Lichborne's (`src/main/sessionLog.ts`, BSD-3), which ships
//! this feature for DragonRealms and had to answer these questions first:
//!
//! - **A file per character per day.** A day is the unit a person searches in,
//!   and it makes rotation implicit rather than a size heuristic that cuts a
//!   hunt in half.
//! - **The date is in the filename, not the line.** `sink/config.rs:175`
//!   reached the same conclusion for the wire log: *"repeating it on 30,000
//!   lines is waste"*. The stamp is [`line_time`]'s, so the two logs of one
//!   session line up by eye.
//! - **The stream tag is in brackets.** A reader greps `[death]`; a future
//!   parser splits on the same bracket. One format serves both.
//!
//! # Buffered, and why the thresholds are theirs
//!
//! A line per `write` syscall would be one syscall per game line. Lichborne
//! flushes on a 1s timer, at 100 records, and forces at 5000; those numbers are
//! from a client in daily use, so they are taken rather than invented. Here the
//! timer belongs to whoever drives [`PlayerWriter::run`] -- the count
//! thresholds are this type's.
//!
//! # What this does NOT do
//!
//! **It does not decide what gets logged.** It writes what arrives. Deciding is
//! the actor's, and keeping that out of here is what lets a test drive the
//! writer with three synthetic lines and no session at all.
//!
//! **It does not compress or prune.** Steps 4 and 5.

use std::fs::{self, File, OpenOptions};
use std::io::{self, BufWriter, Write};
use std::path::{Path, PathBuf};

use cena_platform::Redactions;

use super::{LogLine, LogSink};

/// Flush once this many lines are buffered.
///
/// Lichborne's `FLUSH_THRESHOLD`. The ordinary path: a busy hunt reaches it in
/// well under a second, and an idle character never does -- which is what the
/// caller's timer is for.
pub const FLUSH_AFTER_LINES: usize = 100;

/// Flush unconditionally at this many, whatever else is happening.
///
/// Lichborne's `MAX_BUFFER`, and their comment's framing is right: this is the
/// flood guard, not the normal path. Reaching it means the login burst or a
/// wall of combat text arrived faster than the timer fires.
pub const FORCE_FLUSH_AT: usize = 5000;

/// The subdirectory under the log directory that player logs live in.
///
/// Beside the wire log's captures rather than mixed among them: the two have
/// different lifetimes (`sink/mod.rs:10` -- one *"churns freely"*, this is
/// *"kept; searched years later"*), and a retention sweep over one must not be
/// able to reach the other.
pub const SUBDIR: &str = "player";

/// One character's player log on disk.
///
/// Holds a file open for the current day and rolls when the date changes.
#[derive(Debug)]
pub struct PlayerWriter {
    root: PathBuf,
    character: String,
    /// The day the open file is for, `YYYY-MM-DD`. `None` before the first
    /// line, because a writer that has never been handed anything has not
    /// created a file -- an empty log for a character who never played is a
    /// lie about which characters were used.
    day: Option<String>,
    file: Option<BufWriter<File>>,
    buffered: usize,
    redactions: Redactions,
}

impl PlayerWriter {
    /// A writer for one character under `root`.
    ///
    /// Creates nothing: the directory and the file appear on the first line.
    #[must_use]
    pub fn new(root: impl Into<PathBuf>, character: &str) -> Self {
        Self {
            root: root.into(),
            character: character.to_owned(),
            day: None,
            file: None,
            buffered: 0,
            redactions: Redactions::new(),
        }
    }

    /// Register a secret to strip from every line before it is written.
    ///
    /// **A player log needs this exactly as the wire log does.** The launch key
    /// arrives as game text during login, and a log kept for years is the worst
    /// place for it. `sink/writer.rs:650` records the matching hazard: one log
    /// spans every generation, and each reconnect brings a NEW key, so the set
    /// must be able to grow after the file is open.
    ///
    /// # One hazard the wire log has and this does not
    ///
    /// `SessionSink` holds back a suffix (`pending`) so that a secret split
    /// across two *chunk* boundaries is still caught -- found by review after
    /// a rotation between a chunk ending `SEC` and one starting `RET` wrote
    /// both halves in clear.
    ///
    /// That cannot happen here: the unit is a **line**, already assembled by
    /// `LineAssembler` before it reaches [`LogLine`], and
    /// [`Redactions::apply`] sees each one whole. Recorded so nobody later
    /// reads the missing `pending` as an oversight -- and so that anyone who
    /// changes the unit from a line to something smaller knows what they are
    /// giving up.
    pub fn redact(&mut self, secret: &str, replacement: &'static str) {
        self.redactions.add(secret, replacement);
    }

    /// The directory this character's files live in.
    #[must_use]
    pub fn dir(&self) -> PathBuf {
        self.root.join(SUBDIR).join(safe_name(&self.character))
    }

    /// The file one day's lines go to.
    #[must_use]
    pub fn path_for(&self, day: &str) -> PathBuf {
        self.dir()
            .join(format!("{}_{day}.log", safe_name(&self.character)))
    }

    /// Write one line, opening or rolling the file if the day changed.
    ///
    /// # Errors
    ///
    /// Any failure to create the directory, open the file, or write.
    pub fn write(&mut self, line: &LogLine) -> io::Result<()> {
        let day = today();
        if self.day.as_deref() != Some(day.as_str()) {
            self.roll(&day)?;
        }
        let Some(file) = self.file.as_mut() else {
            // Unreachable: `roll` sets it or returns an error. Not an
            // `unwrap`, because a panic here would take down the session over
            // a log line, and `plan/12` §5.5 says a session survives a failed
            // log.
            return Err(io::Error::other("player log: no open file after roll"));
        };
        writeln!(
            file,
            "[{}][{}] {}",
            line.at,
            line.stream,
            self.redactions.apply(&line.text)
        )?;
        self.buffered += 1;
        if self.buffered >= FLUSH_AFTER_LINES {
            self.flush()?;
        }
        Ok(())
    }

    /// Close the current day's file and open the next.
    ///
    /// # Two behaviours here are unenforced, and deliberately so
    ///
    /// MEASURED by mutation (2026-09-21): making this roll only on the FIRST
    /// line (`self.day.is_none()`) passes all 14 tests, and so does deleting
    /// the final flush in [`Self::run_reporting`].
    ///
    /// Neither is a hole in the tests so much as a limit on what a test can
    /// reach. The day comes from the wall clock ([`today`]), so a suite that
    /// runs in a second cannot cross midnight; and `Drop` flushes too, so a
    /// missing explicit flush is invisible whenever the writer is dropped
    /// afterwards -- which every test does.
    ///
    /// Testing them needs an injected clock, which `plan/25` does not yet call
    /// for and which would exist only for the test (Rule −1). Recorded here
    /// rather than covered by a test that would pass either way -- `plan/19`'s
    /// standing finding is that the dangerous test is the one that cannot
    /// reach the code it names.
    fn roll(&mut self, day: &str) -> io::Result<()> {
        // Flush the old day BEFORE opening the new one. A line written at
        // 23:59:59 belongs in yesterday's file, and losing it to an
        // unflushed buffer would put a hole exactly at the boundary a reader
        // is most likely to be looking at.
        self.flush()?;
        let dir = self.dir();
        fs::create_dir_all(&dir)?;
        let path = self.path_for(day);
        // Append, never truncate. A second session for the same character on
        // the same day continues the file; `create(true).truncate(true)`
        // would silently erase the morning's play.
        let file = OpenOptions::new().create(true).append(true).open(&path)?;
        self.file = Some(BufWriter::new(file));
        self.day = Some(day.to_owned());
        Ok(())
    }

    /// Push everything buffered to the OS.
    ///
    /// # Errors
    ///
    /// Any write failure.
    pub fn flush(&mut self) -> io::Result<()> {
        if let Some(file) = self.file.as_mut() {
            file.flush()?;
        }
        self.buffered = 0;
        Ok(())
    }

    /// How many lines are written but not yet flushed.
    #[must_use]
    pub fn buffered(&self) -> usize {
        self.buffered
    }

    /// Drain a sink until it ends, writing what arrives.
    ///
    /// **A write failure is counted, not propagated.** `plan/12` §5.5: a
    /// session survives a failed log. The count reaches
    /// [`PlayerLog::dropped`](super::PlayerLog::dropped), so a frontend can
    /// say *"your history has a hole in it"* -- which is the whole difference
    /// between this and the reference implementation, whose write errors go to
    /// a console nobody reads.
    pub async fn run(self, sink: LogSink) {
        let _ = self.run_reporting(sink).await;
    }

    /// [`Self::run`], returning the total lines lost.
    ///
    /// **Ends only once every [`PlayerLog`](super::PlayerLog) has been
    /// dropped**, because that is what closes the channel. A caller holding a
    /// handle in order to read the count afterwards will wait forever -- which
    /// is exactly how this method came to exist, after a test did that and hung
    /// the suite. The count comes back from here instead.
    pub async fn run_reporting(mut self, mut sink: LogSink) -> u64 {
        while let Some(line) = sink.recv().await {
            if self.write(&line).is_err() {
                sink.note_dropped();
            }
            if self.buffered >= FORCE_FLUSH_AT && self.flush().is_err() {
                sink.note_dropped();
            }
        }
        // The senders are gone, so the session has ended. Anything buffered is
        // the last thing that happened, which is the part of a log people
        // actually go looking for.
        if self.flush().is_err() {
            sink.note_dropped();
        }
        sink.dropped()
    }
}

impl Drop for PlayerWriter {
    /// Flush on drop, so a writer that is simply dropped still leaves a
    /// complete file. `sink/writer.rs:213` does the same, for the same reason,
    /// and swallows the error for the same one: `Drop` cannot report.
    fn drop(&mut self) {
        let _ = self.flush();
    }
}

/// Strip characters that cannot appear in a filename.
///
/// Character names are the game's and ought to be alphabetic, but a filename
/// is not the place to find out otherwise. Lichborne strips the same set; the
/// development platform is Windows, where `:` and `?` are hard errors rather
/// than odd names.
fn safe_name(character: &str) -> String {
    let cleaned: String = character
        .chars()
        .filter(|c| !r#"/\:*?"<>|"#.contains(*c) && !c.is_control())
        .collect();
    if cleaned.is_empty() {
        // A name that was ENTIRELY forbidden characters would otherwise
        // produce `_2026-09-21.log` in the log root's parent -- a path outside
        // this feature's directory. Never silently.
        "unnamed".to_owned()
    } else {
        cleaned
    }
}

/// Which day's file a line belongs in: today.
///
/// **This reads a clock rather than the line.** `LogLine::at` is
/// `HH:MM:SS.mmm` and deliberately carries no date (D2), so there is nothing in
/// the record to take a day from.
///
/// That is right for what this feature does -- lines are written within
/// milliseconds of arriving -- and it is the one thing that would have to
/// change to replay an old capture into a log, because every line would then
/// land in today's file. Recorded here rather than discovered later.
fn today() -> String {
    cena_platform::date_dir()
}

/// The default root for player logs, matching the wire log's.
#[must_use]
pub fn root() -> PathBuf {
    cena_platform::log_dir()
}

/// The line one [`LogLine`] becomes on disk, without writing it.
///
/// Exposed so a test can assert the format without a filesystem, and so the
/// reader (step 3) has one place to look for what it must parse.
#[must_use]
pub fn format_line(line: &LogLine) -> String {
    format!("[{}][{}] {}", line.at, line.stream, line.text)
}

/// Read a written line back into its three parts.
///
/// **Both formats this project will ever write are accepted here, forever.**
/// Today there is one, which makes this look like speculation -- it is not:
/// Lichborne's reader carries the same promise (*"Two line formats are
/// accepted forever"*) because they changed theirs once and a format migration
/// over years of archive is not a thing anyone wants to write. The place to
/// add the second is here, and the test that both parse is what makes the
/// promise real.
///
/// Returns `(time, stream, text)`, or `None` for a line that is not ours.
#[must_use]
pub fn parse_line(line: &str) -> Option<(&str, &str, &str)> {
    let rest = line.strip_prefix('[')?;
    let (at, rest) = rest.split_once(']')?;
    let rest = rest.strip_prefix('[')?;
    let (stream, rest) = rest.split_once(']')?;
    Some((at, stream, rest.strip_prefix(' ').unwrap_or(rest)))
}

/// Every day-file this character has, newest first.
///
/// # Errors
///
/// Any failure to read the directory. A missing directory is **not** an error:
/// a character who has never played has no logs, which is a fact rather than a
/// failure.
pub fn days(root: &Path, character: &str) -> io::Result<Vec<PathBuf>> {
    let dir = root.join(SUBDIR).join(safe_name(character));
    let entries = match fs::read_dir(&dir) {
        Ok(entries) => entries,
        Err(err) if err.kind() == io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(err) => return Err(err),
    };
    let mut paths: Vec<PathBuf> = entries
        .filter_map(Result::ok)
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|e| e == "log"))
        .collect();
    // Newest first. The names embed an ISO date, so lexical order IS date
    // order -- which is the reason for that format rather than a locale one.
    paths.sort_unstable_by(|a, b| b.cmp(a));
    Ok(paths)
}
