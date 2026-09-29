//! How long the player log is kept: **`plan/25` step 5**.
//!
//! Forever unless the player says otherwise (the author, 2026-09-29, D5), and
//! no cap on size (*"agree no size cap"*). With a number of days set, what is
//! wholly older than that goes: a day-file once its day is, an archive once
//! every day in it is. Never part of an archive.
//!
//! # Never a surprise
//!
//! `plan/25` §7: *"Retention never surprises. Deletion is previewed."* The
//! same [`doomed`] that [`prune`] deletes is what the *Player log* page and
//! `;history` show the player first, so what they are told is what goes. It
//! goes at the next login, not when the setting is changed.

use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use cena_platform::eastern;

use super::{archive, writer};

/// One file that retention removes.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Doomed {
    /// Where it is.
    pub path: PathBuf,
    /// Its newest day, `YYYY-MM-DD`: every day in it is this old or older.
    pub newest: String,
    /// Its size.
    pub bytes: u64,
}

/// The first day kept when `keep_days` days are kept counting `today`
/// (`YYYY-MM-DD`, the player's date); `None` to keep everything.
///
/// `keep_days` of 0 is *forever*, as the settings page says it.
#[must_use]
pub fn first_kept(today: &str, keep_days: u32) -> Option<String> {
    if keep_days == 0 {
        return None;
    }
    let today = eastern::parse(today)?;
    Some(eastern::format(eastern::civil_from_days(
        eastern::days_from_civil(today) - i64::from(keep_days) + 1,
    )))
}

/// What keeping `keep_days` days, counting `today`, removes: every day-file
/// and archive whose newest day is before the first day kept. Oldest first.
///
/// # Errors
///
/// A failure to read the directory or an archive's manifest.
pub fn doomed(
    root: &Path,
    character: &str,
    keep_days: u32,
    today: &str,
) -> io::Result<Vec<Doomed>> {
    let Some(first) = first_kept(today, keep_days) else {
        return Ok(Vec::new());
    };
    let size = |path: &Path| fs::metadata(path).map_or(0, |m| m.len());
    let mut doomed = Vec::new();
    for path in writer::days(root, character)? {
        if let Some(day) = writer::day_of(&path).filter(|day| *day < first) {
            doomed.push(Doomed {
                bytes: size(&path),
                newest: day,
                path,
            });
        }
    }
    for path in archive::archives(root, character)? {
        let newest = archive::names(&path)?
            .iter()
            .filter_map(|name| writer::piece(name).map(|(day, _)| day))
            .max();
        if let Some(newest) = newest.filter(|day| *day < first) {
            doomed.push(Doomed {
                bytes: size(&path),
                newest,
                path,
            });
        }
    }
    doomed.sort_by(|a, b| a.newest.cmp(&b.newest).then_with(|| a.path.cmp(&b.path)));
    Ok(doomed)
}

/// Remove what [`doomed`] names, and say what went.
///
/// # Errors
///
/// A failure to list or to remove; what was removed before it stays removed.
pub fn prune(root: &Path, character: &str, keep_days: u32, today: &str) -> io::Result<Vec<Doomed>> {
    let doomed = doomed(root, character, keep_days, today)?;
    for file in &doomed {
        fs::remove_file(&file.path)?;
    }
    Ok(doomed)
}

/// What [`doomed`] found, as one line a player reads: `nothing`, or how many
/// files, how much, and back to when.
#[must_use]
pub fn preview(doomed: &[Doomed]) -> String {
    let (Some(oldest), Some(newest)) = (doomed.first(), doomed.last()) else {
        return "Nothing is old enough to remove.".to_owned();
    };
    let bytes: u64 = doomed.iter().map(|d| d.bytes).sum();
    format!(
        "The next login removes {} file{} ({} KB), the days up to {} (the oldest {}).",
        doomed.len(),
        if doomed.len() == 1 { "" } else { "s" },
        bytes.div_ceil(1024),
        newest.newest,
        oldest.newest
    )
}
