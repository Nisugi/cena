//! The player log's archives: **`plan/25` step 4**.
//!
//! Once an Eastern month (or a Sunday-to-Saturday Eastern week) is over, its
//! day-files become one archive, `<character>_2026-09.log.gz` (or
//! `<character>_week-2026-09-27.log.gz`). The author, 2026-09-29: monthly by
//! default, *"actual calendar month"*, weekly or off as the player chooses,
//! and *"backup system should be in eastern time like the server"*. The day-files
//! themselves stay on the player's clock; the writer cuts them wherever a week
//! or a month begins in Eastern ([`file_key`](super::writer::file_key)), so
//! every file belongs to exactly one archive.
//!
//! # The format: gzip members, a manifest first
//!
//! A gzip file may be several gzip streams one after another, and each may
//! carry a file name. An archive is:
//!
//! 1. a member named `manifest`: one line per member after it,
//!    `<file name>\t<compressed bytes>`;
//! 2. each day-file, compressed whole, under its own name.
//!
//! `zcat` reads it as every day-file in turn (the manifest's lines first),
//! so a player can still grep an archive by hand. The manifest is what lets a
//! reader skip to one day without inflating the month before it.
//!
//! # Nothing is lost on the way
//!
//! An archive is written beside its final name, read back member by member
//! and compared with the day-files it holds, and only then renamed into place;
//! the day-files are removed last. A crash at any point leaves either the
//! day-files or a whole archive (or both, which the reader handles: a
//! day-file wins over the same name in an archive).

use std::collections::BTreeMap;
use std::fs;
use std::io::{self, BufRead, BufReader, Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};

use cena_platform::eastern::{self, Date};

use super::writer;

/// How a character's closed days are kept.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Archive {
    /// One archive per Eastern calendar month. The author's default.
    #[default]
    Monthly,
    /// One archive per Sunday-to-Saturday Eastern week.
    Weekly,
    /// Every day-file stays plain text.
    Off,
}

impl Archive {
    /// The choices, as the settings file writes them.
    pub const ALL: [Archive; 3] = [Archive::Monthly, Archive::Weekly, Archive::Off];

    /// As the settings file writes it: `monthly`, `weekly`, `off`.
    #[must_use]
    pub const fn word(self) -> &'static str {
        match self {
            Archive::Monthly => "monthly",
            Archive::Weekly => "weekly",
            Archive::Off => "off",
        }
    }

    /// The choice a word names, ignoring case.
    #[must_use]
    pub fn from_word(word: &str) -> Option<Self> {
        Self::ALL
            .into_iter()
            .find(|choice| choice.word().eq_ignore_ascii_case(word.trim()))
    }

    /// The archive a stretch beginning on `stretch` belongs in: `2026-09`,
    /// `week-2026-09-27`; `None` when nothing is archived.
    #[must_use]
    pub fn period(self, stretch: Date) -> Option<String> {
        match self {
            Archive::Monthly => Some(format!("{:04}-{:02}", stretch.0, stretch.1)),
            Archive::Weekly => Some(format!(
                "week-{}",
                eastern::format(eastern::week_start(stretch))
            )),
            Archive::Off => None,
        }
    }
}

/// The name of the manifest member.
const MANIFEST: &str = "manifest";

/// What one sweep did.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Swept {
    /// Day-files now in an archive, and removed.
    pub files: usize,
    /// The archives written or added to, by name.
    pub archives: Vec<String>,
}

/// Archive every day-file of `character` whose period is over at Unix time
/// `now`, as `choice` says. Nothing for [`Archive::Off`].
///
/// The file being written now is never touched: it is in the current
/// stretch, so its period is not over.
///
/// # Errors
///
/// A failure to list, read, write or verify. What was already archived stays
/// archived; a period that failed keeps its day-files.
pub fn sweep(root: &Path, character: &str, choice: Archive, now: i64) -> io::Result<Swept> {
    let Some(current) = choice.period(eastern::stretch_start(eastern::date(now))) else {
        return Ok(Swept::default());
    };
    let mut periods: BTreeMap<String, Vec<PathBuf>> = BTreeMap::new();
    for path in writer::days(root, character)? {
        let Some(name) = path.file_name().and_then(|n| n.to_str()) else {
            continue;
        };
        let Some((_, stretch)) = writer::piece(name) else {
            continue;
        };
        if let Some(period) = choice.period(stretch).filter(|p| *p != current) {
            periods.entry(period).or_default().push(path);
        }
    }
    let mut swept = Swept::default();
    for (period, files) in periods {
        let archive = path(root, character, &period);
        write(&archive, &files)?;
        for file in &files {
            fs::remove_file(file)?;
        }
        swept.files += files.len();
        swept.archives.push(file_name(character, &period));
    }
    Ok(swept)
}

/// Where the archive for `period` is.
#[must_use]
pub fn path(root: &Path, character: &str, period: &str) -> PathBuf {
    writer::dir(root, character).join(file_name(character, period))
}

fn file_name(character: &str, period: &str) -> String {
    format!("{}.gz", writer::file_name(character, period))
}

/// Every archive `character` has.
///
/// # Errors
///
/// A failure to read the directory; none at all is not one.
pub fn archives(root: &Path, character: &str) -> io::Result<Vec<PathBuf>> {
    let entries = match fs::read_dir(writer::dir(root, character)) {
        Ok(entries) => entries,
        Err(err) if err.kind() == io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(err) => return Err(err),
    };
    let mut paths: Vec<PathBuf> = entries
        .filter_map(Result::ok)
        .map(|e| e.path())
        .filter(|p| {
            p.file_name()
                .and_then(|n| n.to_str())
                .is_some_and(|n| n.ends_with(".log.gz"))
        })
        .collect();
    paths.sort_unstable();
    Ok(paths)
}

/// What one character's player log takes on disk (`plan/25` step 7).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Usage {
    /// Plain day-files, in bytes.
    pub plain: u64,
    /// Archives, in bytes.
    pub archived: u64,
    /// Days with anything kept, plain or archived.
    pub days: usize,
}

impl Usage {
    /// Plain and archived together.
    #[must_use]
    pub const fn total(self) -> u64 {
        self.plain + self.archived
    }
}

/// What `character`'s player log takes on disk.
///
/// # Errors
///
/// A failure to read the directory or an archive's manifest.
pub fn usage(root: &Path, character: &str) -> io::Result<Usage> {
    let size = |path: &PathBuf| fs::metadata(path).map_or(0, |m| m.len());
    Ok(Usage {
        plain: writer::days(root, character)?.iter().map(size).sum(),
        archived: archives(root, character)?.iter().map(size).sum(),
        days: super::reader::days(root, character)?.len(),
    })
}

/// The names of the day-files an archive holds, from its manifest alone.
///
/// # Errors
///
/// A failure to read it, or an archive not in this format.
pub fn names(archive: &Path) -> io::Result<Vec<String>> {
    let mut reader = BufReader::new(fs::File::open(archive)?);
    Ok(manifest(&mut reader)?
        .into_iter()
        .map(|(name, _)| name)
        .collect())
}

/// The day-files in `archive` whose names `wanted` accepts, each as its name
/// and its text, in the archive's order. Skips the others unread.
///
/// # Errors
///
/// A failure to read it, or an archive not in this format.
pub fn read(archive: &Path, wanted: impl Fn(&str) -> bool) -> io::Result<Vec<(String, Vec<u8>)>> {
    let mut reader = BufReader::new(fs::File::open(archive)?);
    let members = manifest(&mut reader)?;
    let mut found = Vec::new();
    for (name, length) in members {
        if wanted(&name) {
            let start = reader.stream_position()?;
            found.push((name, member(&mut reader)?));
            // A member ends where the manifest says, whatever the decoder
            // read ahead.
            reader.seek(SeekFrom::Start(start + length))?;
        } else {
            reader.seek_relative(i64::try_from(length).map_err(io::Error::other)?)?;
        }
    }
    Ok(found)
}

/// The manifest, read from the start of an archive, leaving `reader` at the
/// first member after it.
fn manifest(reader: &mut BufReader<fs::File>) -> io::Result<Vec<(String, u64)>> {
    let text = String::from_utf8(member(reader)?).map_err(io::Error::other)?;
    text.lines()
        .map(|line| {
            let (name, length) = line
                .split_once('\t')
                .ok_or_else(|| io::Error::other(format!("not a manifest line: {line}")))?;
            let length = length.parse().map_err(io::Error::other)?;
            Ok((name.to_owned(), length))
        })
        .collect()
}

/// One gzip member, inflated, leaving `reader` just after it.
fn member<R: BufRead>(reader: &mut R) -> io::Result<Vec<u8>> {
    let mut decoder = flate2::bufread::GzDecoder::new(reader);
    let mut bytes = Vec::new();
    decoder.read_to_end(&mut bytes)?;
    Ok(bytes)
}

/// One gzip member holding `bytes` under `name`.
fn compress(name: &str, bytes: &[u8]) -> io::Result<Vec<u8>> {
    let mut encoder = flate2::GzBuilder::new()
        .filename(name)
        .write(Vec::new(), flate2::Compression::default());
    encoder.write_all(bytes)?;
    encoder.finish()
}

/// Write `files` into `archive`, keeping what it already holds, and prove
/// the result before it takes the archive's name.
fn write(archive: &Path, files: &[PathBuf]) -> io::Result<()> {
    let mut members: Vec<(String, Vec<u8>)> = Vec::new();
    let mut sources: Vec<(String, Vec<u8>)> = Vec::new();
    for file in files {
        let name = file
            .file_name()
            .and_then(|n| n.to_str())
            .ok_or_else(|| io::Error::other("a day-file with no name"))?
            .to_owned();
        let bytes = fs::read(file)?;
        members.push((name.clone(), compress(&name, &bytes)?));
        sources.push((name, bytes));
    }
    sources.sort_by(|a, b| a.0.cmp(&b.0));
    // What the archive already holds is kept as it is, compressed, unless a
    // day-file of the same name replaces it: the day-file is the newer.
    if archive.exists() {
        let mut reader = BufReader::new(fs::File::open(archive)?);
        let mut kept = Vec::new();
        for (name, length) in manifest(&mut reader)? {
            let mut bytes = vec![0; usize::try_from(length).map_err(io::Error::other)?];
            reader.read_exact(&mut bytes)?;
            if sources.iter().all(|(source, _)| *source != name) {
                kept.push((name, bytes));
            }
        }
        kept.append(&mut members);
        members = kept;
    }
    members.sort_by(|a, b| a.0.cmp(&b.0));
    let mut listed = String::new();
    for (name, bytes) in &members {
        use std::fmt::Write as _;
        let _ = writeln!(listed, "{name}\t{}", bytes.len());
    }

    let partial = archive.with_extension("gz.partial");
    {
        let mut out = io::BufWriter::new(fs::File::create(&partial)?);
        out.write_all(&compress(MANIFEST, listed.as_bytes())?)?;
        for (_, bytes) in &members {
            out.write_all(bytes)?;
        }
        out.into_inner()
            .map_err(io::IntoInnerError::into_error)?
            .sync_all()?;
    }
    let names: Vec<&str> = sources.iter().map(|(name, _)| name.as_str()).collect();
    let back = read(&partial, |name| names.contains(&name))?;
    if back.len() != sources.len()
        || back
            .iter()
            .zip(&sources)
            .any(|((a, got), (b, want))| a != b || got != want)
    {
        let _ = fs::remove_file(&partial);
        return Err(io::Error::other(format!(
            "{} did not read back as written; the day-files are kept",
            archive.display()
        )));
    }
    fs::rename(&partial, archive)
}
