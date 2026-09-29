//! The player log's reader: **`plan/25` step 3**.
//!
//! Day listing, the tail, a time window, and literal or regex search, over the
//! files [`super::writer`] leaves: `<root>/player/<character>/<character>_<day>.log`,
//! one [`format_line`](super::writer::format_line) per line.
//!
//! # Off the actor, always
//!
//! Every function here is plain blocking file I/O and touches no session
//! state. Lichborne's reason, from its own source: *"main owns every session's
//! socket, so a synchronous multi-file scan freezes EVERY connected character
//! at once."* A caller in async code runs these on a blocking task, and asks
//! the writer to flush first ([`PlayerLog::flush`](super::PlayerLog::flush)),
//! or a search of the live session misses its last second.
//!
//! # A time window is a time window
//!
//! [`window`] compares stamps; it never counts lines back from the end.
//! Lichborne's pitfall #92: an 8,000-line tail reached back 6.2 minutes of a
//! requested 11, because a busy character logs ~820 lines a minute. The stamp
//! is `HH:MM:SS.mmm`, fixed width, so within a day text order IS time order,
//! and the day is the file's.
//!
//! # Bounded
//!
//! At most [`MAX_HITS`] lines come back from any read, and at most
//! [`MAX_DAYS`] files are opened by one: `plan/25` §5's caps, Lichborne's
//! numbers. A read that stopped at the cap says so ([`Found::more`]).
//!
//! # Plain or archived, one day
//!
//! A day is read from wherever its files are: plain `.log` files, or members
//! of a month's or a week's archive (step 4, [`super::archive`]). Only
//! [`days`] and [`read_day`] know there is a difference.

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::io;
use std::path::Path;

use cena_platform::eastern::{self, Date};

use super::archive::{self, Archive};
use super::writer;

/// The most lines one read returns.
pub const MAX_HITS: usize = 1000;

/// The most day-files one read opens: a year, and a leap day.
pub const MAX_DAYS: usize = 366;

/// A moment in the log: the day's file and the stamp within it.
///
/// Ordered as the files are, day then stamp, both compared as text; that is
/// right because both are fixed width (`YYYY-MM-DD`, `HH:MM:SS.mmm`).
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct Moment {
    /// `YYYY-MM-DD`.
    pub day: String,
    /// `HH:MM:SS.mmm`, or any prefix of it: `06:47` is the start of that
    /// minute, since a shorter string sorts before every longer one it begins.
    pub at: String,
}

impl Moment {
    /// The moment `at` on `day`.
    #[must_use]
    pub fn new(day: impl Into<String>, at: impl Into<String>) -> Self {
        Self {
            day: day.into(),
            at: at.into(),
        }
    }
}

/// One line read back.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Entry {
    /// The day of the file it was in, `YYYY-MM-DD`.
    pub day: String,
    /// When it arrived, `HH:MM:SS.mmm`.
    pub at: String,
    /// Its tag: `main`, `thoughts`, `cmd`, `hydra`, `main/combat`.
    pub stream: String,
    /// What it said.
    pub text: String,
}

impl Entry {
    fn after_or_at(&self, moment: &Moment) -> bool {
        (self.day.as_str(), self.at.as_str()) >= (moment.day.as_str(), moment.at.as_str())
    }

    fn before(&self, moment: &Moment) -> bool {
        (self.day.as_str(), self.at.as_str()) < (moment.day.as_str(), moment.at.as_str())
    }
}

/// Which tags a read keeps.
///
/// A tag is matched by any of its `/`-separated parts (`plan/25` step 2b: *"a
/// viewer splits on `/`"*), so `main` keeps `main/combat` as well as `main`, and
/// `combat` keeps only `main/combat`. Case does not matter: the game's own ids
/// are mixed (`Spells`).
///
/// Or, made with [`Streams::classes`], by the tag's last part alone: what
/// the log window ticks, where `main` is the story without its combat.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Streams {
    names: Vec<String>,
    by_class: bool,
}

impl Streams {
    /// Every tag.
    #[must_use]
    pub fn all() -> Self {
        Self::default()
    }

    /// Only these tags, by any part. None named is every tag.
    #[must_use]
    pub fn only<S: Into<String>>(names: impl IntoIterator<Item = S>) -> Self {
        Self {
            names: names.into_iter().map(Into::into).collect(),
            by_class: false,
        }
    }

    /// Only tags whose last part is one of these: `main` keeps `main` and
    /// not `main/combat`. None named is every tag.
    #[must_use]
    pub fn classes<S: Into<String>>(names: impl IntoIterator<Item = S>) -> Self {
        Self {
            by_class: true,
            ..Self::only(names)
        }
    }

    /// The tags named; none for every tag.
    #[must_use]
    pub fn names(&self) -> &[String] {
        &self.names
    }

    /// Whether a line tagged `tag` is kept.
    #[must_use]
    pub fn admits(&self, tag: &str) -> bool {
        let named = |part: &str| self.names.iter().any(|w| w.eq_ignore_ascii_case(part));
        self.names.is_empty()
            || if self.by_class {
                tag.rsplit('/').next().is_some_and(named)
            } else {
                tag.split('/').any(named)
            }
    }
}

/// What a search looks for in a line's text.
#[derive(Clone, Debug)]
pub struct Pattern(regex::Regex);

impl Pattern {
    /// The text itself, ignoring case: what a player types to find a word.
    ///
    /// # Errors
    ///
    /// Only past the regex crate's size limit, for a literal of megabytes:
    /// an escaped literal is otherwise always a valid expression.
    pub fn literal(text: &str) -> Result<Self, String> {
        Self::build(&regex::escape(text), true)
    }

    /// A regular expression, as written: case matters unless it says `(?i)`.
    ///
    /// # Errors
    ///
    /// Why the expression is not one, in the regex crate's words.
    pub fn regex(expression: &str) -> Result<Self, String> {
        Self::build(expression, false)
    }

    fn build(expression: &str, fold: bool) -> Result<Self, String> {
        regex::RegexBuilder::new(expression)
            .case_insensitive(fold)
            .build()
            .map(Self)
            .map_err(|e| e.to_string())
    }

    /// Whether `text` holds a match.
    #[must_use]
    pub fn matches(&self, text: &str) -> bool {
        self.0.is_match(text)
    }
}

/// What a bounded read found.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Found {
    /// The lines, in the order the read gives them.
    pub entries: Vec<Entry>,
    /// Whether the read stopped at [`MAX_HITS`] or [`MAX_DAYS`] with more
    /// left: *"the first thousand"* is not *"all of them"*, and a reader
    /// must be able to tell.
    pub more: bool,
}

/// Every day this character has a log for, `YYYY-MM-DD`, newest first.
///
/// # Errors
///
/// A failure to read the directory. None at all is not one: a character who
/// has never played has no days.
pub fn days(root: &Path, character: &str) -> io::Result<Vec<String>> {
    let mut days: BTreeSet<String> = writer::days(root, character)?
        .iter()
        .filter_map(|path| writer::day_of(path))
        .collect();
    for path in archive::archives(root, character)? {
        days.extend(
            archive::names(&path)?
                .iter()
                .filter_map(|name| writer::piece(name).map(|(day, _)| day)),
        );
    }
    Ok(days.into_iter().rev().collect())
}

/// One day's lines, in the order they were written; none for a day with no
/// file.
///
/// A day may be in more than one file (the writer cuts where an Eastern week
/// or month begins, [`writer::file_key`]) and each may be plain or archived.
/// They are read in the order of the stretches they hold; a plain file wins
/// over an archived one of the same name, which exists only for the moment
/// between an archive being written and the day-files it replaces being
/// removed.
///
/// A line that is not ours is skipped, and so is a last line with no newline:
/// the writer ends every line with one, so a line without it is a write still
/// under way (or cut off by a power loss), and half a line would read as a
/// whole one.
///
/// # Errors
///
/// A failure to read a file or an archive that exists.
pub fn read_day(root: &Path, character: &str, day: &str) -> io::Result<Vec<Entry>> {
    let ours = |name: &str| writer::piece(name).filter(|(of, _)| of == day);
    let mut parts: BTreeMap<(Date, String), Vec<u8>> = BTreeMap::new();
    for path in writer::days(root, character)? {
        let Some(name) = path.file_name().and_then(|n| n.to_str()) else {
            continue;
        };
        let Some((_, stretch)) = ours(name) else {
            continue;
        };
        match fs::read(&path) {
            Ok(bytes) => {
                parts.insert((stretch, name.to_owned()), bytes);
            }
            // Archived and removed since it was listed: the archive has it.
            Err(err) if err.kind() == io::ErrorKind::NotFound => {}
            Err(err) => return Err(err),
        }
    }
    for path in archives_for(root, character, day) {
        for (name, bytes) in archive::read(&path, |name| ours(name).is_some())? {
            if let Some((_, stretch)) = ours(&name) {
                parts.entry((stretch, name)).or_insert(bytes);
            }
        }
    }
    Ok(parts
        .values()
        .flat_map(|bytes| entries(day, bytes))
        .collect())
}

/// The lines of one file's bytes, as [`read_day`] describes.
fn entries(day: &str, bytes: &[u8]) -> Vec<Entry> {
    // Lossy: a byte the writer never wrote (a hand edit, a torn sector) costs
    // that character, not the day.
    let text = String::from_utf8_lossy(bytes);
    let whole = text.rfind('\n').map_or("", |end| &text[..end]);
    whole
        .lines()
        .filter_map(writer::parse_line)
        .map(|(at, stream, text)| Entry {
            day: day.to_owned(),
            at: at.to_owned(),
            stream: stream.to_owned(),
            text: text.trim_end_matches('\r').to_owned(),
        })
        .collect()
}

/// The archives that can hold any of `day`'s files, whichever way they were
/// archived: a file of that date holds a stretch from six days before it
/// (the week began earlier) to the day after (a player west of Eastern, past
/// midnight there).
fn archives_for(root: &Path, character: &str, day: &str) -> Vec<std::path::PathBuf> {
    let Some(date) = eastern::parse(day) else {
        return Vec::new();
    };
    let days = eastern::days_from_civil(date);
    let periods: BTreeSet<String> = (days - 6..=days + 1)
        .map(eastern::civil_from_days)
        .flat_map(|d| [Archive::Monthly, Archive::Weekly].map(|choice| choice.period(d)))
        .flatten()
        .collect();
    periods
        .iter()
        .map(|period| archive::path(root, character, period))
        .filter(|path| path.is_file())
        .collect()
}

/// The last `count` lines kept by `streams`, oldest first, across as many
/// days as it takes (at most [`MAX_DAYS`]). `count` is capped at
/// [`MAX_HITS`].
///
/// # Errors
///
/// A failure to read the directory or a day's file.
pub fn tail(
    root: &Path,
    character: &str,
    count: usize,
    streams: &Streams,
) -> io::Result<Vec<Entry>> {
    let count = count.min(MAX_HITS);
    let mut kept: Vec<Entry> = Vec::new();
    for day in days(root, character)?.iter().take(MAX_DAYS) {
        if kept.len() >= count {
            break;
        }
        let mut earlier: Vec<Entry> = read_day(root, character, day)?
            .into_iter()
            .filter(|e| streams.admits(&e.stream))
            .collect();
        earlier.append(&mut kept);
        kept = earlier;
    }
    let cut = kept.len().saturating_sub(count);
    kept.drain(..cut);
    Ok(kept)
}

/// The lines from `from` up to but not including `to`, kept by `streams`,
/// oldest first: the first [`MAX_HITS`] of them, with [`Found::more`] set when
/// there were others after.
///
/// Chosen by stamp, never by counting lines (the module doc).
///
/// # Errors
///
/// A failure to read the directory or a day's file.
pub fn window(
    root: &Path,
    character: &str,
    from: &Moment,
    to: &Moment,
    streams: &Streams,
) -> io::Result<Found> {
    let mut found = Found::default();
    let mut within: Vec<String> = days(root, character)?
        .into_iter()
        .filter(|day| *day >= from.day && *day <= to.day)
        .collect();
    within.reverse();
    found.more = within.len() > MAX_DAYS;
    for day in within.iter().take(MAX_DAYS) {
        for entry in read_day(root, character, day)? {
            if !entry.after_or_at(from) || !entry.before(to) || !streams.admits(&entry.stream) {
                continue;
            }
            if found.entries.len() == MAX_HITS {
                found.more = true;
                return Ok(found);
            }
            found.entries.push(entry);
        }
    }
    Ok(found)
}

/// The lines whose text matches `pattern`, kept by `streams`, **newest
/// first**, from the days on or after `since` (every day when `None`): the
/// first [`MAX_HITS`] found, over at most [`MAX_DAYS`] days.
///
/// # Errors
///
/// A failure to read the directory or a day's file.
pub fn search(
    root: &Path,
    character: &str,
    pattern: &Pattern,
    streams: &Streams,
    since: Option<&str>,
) -> io::Result<Found> {
    let mut found = Found::default();
    let within: Vec<String> = days(root, character)?
        .into_iter()
        .filter(|day| since.is_none_or(|since| day.as_str() >= since))
        .collect();
    for day in within.iter().take(MAX_DAYS) {
        let hits = read_day(root, character, day)?
            .into_iter()
            .rev()
            .filter(|e| streams.admits(&e.stream) && pattern.matches(&e.text));
        for hit in hits {
            if found.entries.len() == MAX_HITS {
                found.more = true;
                return Ok(found);
            }
            found.entries.push(hit);
        }
    }
    found.more = within.len() > MAX_DAYS;
    Ok(found)
}

/// What an export wrote.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Exported {
    /// The file.
    pub path: std::path::PathBuf,
    /// Lines written.
    pub lines: usize,
    /// Days they came from.
    pub days: usize,
}

/// Write every line from the days `from` to `to` (both kept, `YYYY-MM-DD`)
/// kept by `streams` to one plain file at `out`, oldest first: `plan/25`
/// step 6, *"the caller sends range/streams/options; the owner does all
/// I/O"*. At most [`MAX_DAYS`] days, the newest dropped past that.
///
/// Each line is `YYYY-MM-DD HH:MM:SS.mmm [tag] text`, the day in front, since
/// an export may span many. `out` is written whole or not at all: beside its
/// name first, then renamed.
///
/// # Errors
///
/// A failure to read the log or to write `out`.
pub fn export(
    root: &Path,
    character: &str,
    (from, to): (&str, &str),
    streams: &Streams,
    out: &Path,
) -> io::Result<Exported> {
    use std::io::Write as _;
    let mut within: Vec<String> = days(root, character)?
        .into_iter()
        .filter(|day| day.as_str() >= from && day.as_str() <= to)
        .collect();
    within.reverse();
    within.truncate(MAX_DAYS);
    if let Some(parent) = out.parent() {
        fs::create_dir_all(parent)?;
    }
    let partial = out.with_extension("partial");
    let mut file = io::BufWriter::new(fs::File::create(&partial)?);
    let mut lines = 0;
    for day in &within {
        for entry in read_day(root, character, day)? {
            if streams.admits(&entry.stream) {
                writeln!(
                    file,
                    "{} {} [{}] {}",
                    entry.day, entry.at, entry.stream, entry.text
                )?;
                lines += 1;
            }
        }
    }
    file.into_inner()
        .map_err(io::IntoInnerError::into_error)?
        .sync_all()?;
    fs::rename(&partial, out)?;
    Ok(Exported {
        path: out.to_owned(),
        lines,
        days: within.len(),
    })
}

/// Where an export of `from` to `to` goes when the player names no place:
/// `exports/` in the character's log folder, which no read of the log looks
/// in.
#[must_use]
pub fn export_path(root: &Path, character: &str, (from, to): (&str, &str)) -> std::path::PathBuf {
    let name = writer::file_name(character, &format!("{from}_to_{to}"));
    writer::dir(root, character)
        .join("exports")
        .join(Path::new(&name).with_extension("txt"))
}
