//! The learned menu dictionary on disk: one global supplemental TSV.
//!
//! # Why one file, rewritten whole
//!
//! > **AUTHOR, 2026-09-20:** *"The file is global. Everyone gets the same
//! > command list. Can we do a supplemental tsv, and when another update comes
//! > in it overwrites the tsv file with the existing + new and use new
//! > timestamp?"*
//!
//! That is what this does, and the shape rules out the alternative that was
//! considered first. An append-only log, or a fragment written per push, turns
//! "what does this coordinate mean now?" into a question you answer by reading
//! every file in order. Rewriting one file with `existing + new` means the file
//! always *is* the answer.
//!
//! The merge is [`LearnedCommands`]'s own -- additive, and a repeated
//! coordinate replaces -- so disk and memory cannot disagree about it.
//!
//! # Global, not per character
//!
//! [`character_store`] is keyed by instance and character because stats are.
//! This is not: a coordinate's meaning is a fact about the **game**, and every
//! character on the account receives the same push. Per-character files would
//! write identical rows N times and let one character's copy go stale while
//! another's is current.
//!
//! [`character_store`]: crate::character_store
//!
//! # The format is the shipped table's
//!
//! Four tab-separated columns under a `coord label command category` header,
//! byte-for-byte what `cena-model/data/menu_commands.tsv` uses, so folding
//! this file back into the shipped table is a concatenate-and-sort rather than
//! a translation. The version line is a leading `#` comment, which the reader
//! of either file skips.
//!
//! # Why this is in `cena-session` and not `cena-model`
//!
//! `cena-model` is barred from file I/O by an architecture test. The model
//! owns the merge; this owns the bytes.

//! # The staleness chain, and what it can and cannot decide
//!
//! > **AUTHOR, 2026-09-20:** *"when a new version is released with those
//! > updates tsv'd into the internal tsv, the internal tsv timestamp gets
//! > updated and that's what a new update gets checked against first, then it
//! > gets checked against the fragment, then rewrite fragment if it's new
//! > timestamp?"*
//!
//! That is the right order, and [`prune`] implements it: the shipped table's
//! version is the baseline, the supplemental file's is what has been learned
//! since, and a row is only worth keeping in the file if the shipped table
//! does not already hold it.
//!
//! **One correction to the shape of it.** The checks cannot gate *whether we
//! receive* an update, because the client never asks: no observed traffic
//! sends a version up, and the two captured pushes arrived unprompted 85 lines
//! after `<endSetup/>`. So the chain is not a request filter. It runs
//! **after** a push lands, to decide what the file should now contain --
//! which is the same work, minus a round trip we have no evidence exists.
//!
//! The consequence is that [`LearnedCommands::novel`] does the deciding, not a
//! timestamp comparison: a row matching the shipped table is dropped from the
//! file whatever the versions say. A timestamp tells you a file is *old*; it
//! cannot tell you *which rows* a newer release absorbed. Comparing the rows
//! answers that exactly, so the version is recorded for a person reading the
//! file and is not load-bearing for correctness.

use std::fmt::Write as _;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use cena_model::{LearnedCommands, MenuCommand};

/// The supplemental dictionary's filename inside the data directory.
pub const FILE_NAME: &str = "menu_commands_learned.tsv";

/// The header every file carries, matching the shipped table's.
const HEADER: &str = "coord\tlabel\tcommand\tcategory";

/// The prefix of the version comment line.
const VERSION_PREFIX: &str = "# cmdtimestamp\t";

/// The path the supplemental dictionary lives at.
#[must_use]
pub fn store_path(dir: &Path) -> PathBuf {
    dir.join(FILE_NAME)
}

/// Read the supplemental dictionary.
///
/// **A missing file is not an error**: it is the ordinary state of an install
/// whose shipped table is still current, which is the state measured today --
/// the rows the server pushes are byte-identical to the ones shipped, so
/// `novel()` reports nothing and this file is never created. An empty
/// [`LearnedCommands`] is the right answer for it.
///
/// Malformed rows are skipped rather than failing the load. A row needs four
/// columns and a non-empty coordinate; anything else cannot be looked up, and
/// refusing the whole file over one bad line would discard every good one.
///
/// # Errors
///
/// Propagates an I/O error other than "not found" -- an unreadable file is a
/// real problem and differs from an absent one.
pub fn load(dir: &Path) -> io::Result<LearnedCommands> {
    let text = match fs::read_to_string(store_path(dir)) {
        Ok(text) => text,
        Err(err) if err.kind() == io::ErrorKind::NotFound => return Ok(LearnedCommands::default()),
        Err(err) => return Err(err),
    };
    Ok(parse(&text))
}

/// Build a [`LearnedCommands`] from a supplemental file's text.
fn parse(text: &str) -> LearnedCommands {
    let mut learned = LearnedCommands::default();
    let mut rows = Vec::new();
    for line in text.lines() {
        if let Some(version) = line.strip_prefix(VERSION_PREFIX) {
            learned.set_version(version.trim());
            continue;
        }
        if line.starts_with('#') || line.is_empty() || line == HEADER {
            continue;
        }
        let mut cols = line.split('\t');
        let (Some(coord), Some(label), Some(command), Some(category)) =
            (cols.next(), cols.next(), cols.next(), cols.next())
        else {
            continue;
        };
        if coord.is_empty() {
            continue;
        }
        rows.push(MenuCommand {
            coord: coord.to_owned(),
            label: label.to_owned(),
            command: command.to_owned(),
            category: category.to_owned(),
        });
    }
    learned.absorb(&rows);
    learned
}

/// Write the supplemental dictionary, replacing whatever was there.
///
/// The caller passes the **accumulated** set -- what [`load`] returned with the
/// new push applied on top -- because that is what "existing + new" means and
/// what makes one file sufficient. [`merge_and_save`] does that sequence.
///
/// Only [`LearnedCommands::novel`] rows are written. A row identical to the
/// shipped table is noise in a file whose purpose is to show what the shipped
/// table is missing, and writing it would mean a fold-back reviewer reads 1,106
/// rows to find the two that changed.
///
/// # Errors
///
/// Propagates a failure to create the directory, write the temporary file, or
/// rename it over the target.
pub fn save(dir: &Path, learned: &LearnedCommands) -> io::Result<PathBuf> {
    let path = store_path(dir);
    fs::create_dir_all(dir)?;

    let mut text = String::new();
    if let Some(version) = learned.version() {
        text.push_str(VERSION_PREFIX);
        text.push_str(version);
        text.push('\n');
    }
    text.push_str(HEADER);
    text.push('\n');
    for row in learned.novel() {
        let _ = writeln!(
            text,
            "{}\t{}\t{}\t{}",
            row.coord, row.label, row.command, row.category
        );
    }

    // Temp file in the SAME directory so the rename is atomic, for the reason
    // `character_store::save` states at length: a cross-device rename degrades
    // to copy-then-delete. That comment also records honestly that this
    // property is enforced by review rather than by a test, because a unit
    // test cannot crash the process between the truncate and the write. The
    // same is true here.
    let temp = path.with_extension("tsv.tmp");
    fs::write(&temp, text)?;
    fs::rename(&temp, &path)?;
    Ok(path)
}

/// Fold a session's learned rows into the file: load, merge, write back.
///
/// This is the author's sequence -- *"it overwrites the tsv file with the
/// existing + new and use new timestamp"* -- in one call, so a caller cannot
/// perform half of it and write a file that drops what was already there.
///
/// The session's version wins when it has one, because it came from a
/// `<cmdtimestamp>` the server sent during this session and the file's came
/// from an older one.
///
/// **Nothing is written when there is nothing novel to record.** An install
/// whose shipped table is current never grows a file, which is the state
/// measured today.
///
/// # Errors
///
/// Propagates the load's and the save's I/O errors.
pub fn merge_and_save(dir: &Path, session: &LearnedCommands) -> io::Result<Option<PathBuf>> {
    let mut merged = load(dir)?;
    let rows: Vec<MenuCommand> = session.all().cloned().collect();
    merged.absorb(&rows);
    if let Some(version) = session.version() {
        merged.set_version(version);
    }
    if merged.novel().next().is_none() {
        return Ok(None);
    }
    save(dir, &merged).map(Some)
}

/// Drop rows a newer shipped table has since absorbed.
///
/// The second half of the author's chain: after a release folds learned rows
/// into `menu_commands.tsv`, the supplemental file still holds its copies of
/// them, and they are now noise. This removes exactly those, leaving the rows
/// the shipped table still does not have.
///
/// Returns the file's new path, or `None` when nothing survives -- in which
/// case the file is **deleted**, because an empty supplemental file states
/// "there are learned rows" and there are not.
///
/// # Errors
///
/// Propagates the load's, save's, and removal's I/O errors.
pub fn prune(dir: &Path) -> io::Result<Option<PathBuf>> {
    let stored = load(dir)?;
    if stored.is_empty() {
        return Ok(None);
    }
    let mut kept = LearnedCommands::default();
    let rows: Vec<MenuCommand> = stored.novel().cloned().collect();
    kept.absorb(&rows);
    if let Some(version) = stored.version() {
        kept.set_version(version);
    }
    if kept.is_empty() {
        let path = store_path(dir);
        match fs::remove_file(&path) {
            Ok(()) => {}
            Err(err) if err.kind() == io::ErrorKind::NotFound => {}
            Err(err) => return Err(err),
        }
        return Ok(None);
    }
    save(dir, &kept).map(Some)
}
