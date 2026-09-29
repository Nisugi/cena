//! The reads a log window asks for, and running one: blocking file I/O over
//! `cena_session::player_log`, never on the window's thread.

use std::path::Path;
use std::sync::{Arc, Mutex};

use cena_session::player_log::archive::{self, Usage};
use cena_session::player_log::reader::{self, Entry, Exported, Found, Pattern, Streams};
use cena_session::player_log::writer;

/// A read the window wants done.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum Ask {
    /// The days kept, and the disk they take.
    Days,
    /// One day's lines.
    Day(String),
    /// A search: the text, and whether it is a regular expression.
    Search(String, bool),
    /// An export of these days, `from` to `to`, of these tags.
    Export(String, String, Streams),
}

/// What a read answered.
#[derive(Clone, Debug)]
pub(crate) enum Reply {
    /// The days kept, and the disk used.
    Days(Result<(Days, Usage), String>),
    /// A day's lines.
    Day(String, Result<Vec<Entry>, String>),
    /// What a search found.
    Found(Result<Found, String>),
    /// What an export wrote.
    Exported(Result<Exported, String>),
}

/// The days kept, newest first, each with its plain size (`None`: archived).
pub(crate) type Days = Vec<(String, Option<u64>)>;

/// Where answers land for the window to take: shared with the read.
pub(crate) type Inbox = Arc<Mutex<Vec<Reply>>>;

/// Do `ask` against `character`'s log under `root`. Blocking file I/O: never
/// on the window's thread.
pub(crate) fn run(root: &Path, character: &str, ask: Ask) -> Reply {
    let text = |e: std::io::Error| e.to_string();
    match ask {
        Ask::Days => Reply::Days(days(root, character).map_err(text)),
        Ask::Day(day) => {
            let lines = reader::read_day(root, character, &day).map_err(text);
            Reply::Day(day, lines)
        }
        Ask::Search(query, regex) => Reply::Found(
            if regex {
                Pattern::regex(&query)
            } else {
                Pattern::literal(&query)
            }
            .and_then(|pattern| {
                reader::search(root, character, &pattern, &Streams::all(), None)
                    .map_err(|e| e.to_string())
            }),
        ),
        Ask::Export(from, to, streams) => {
            let (from, to) = if from <= to { (from, to) } else { (to, from) };
            let out = reader::export_path(root, character, (&from, &to));
            Reply::Exported(
                reader::export(root, character, (&from, &to), &streams, &out).map_err(text),
            )
        }
    }
}

/// The days kept with their plain sizes, and the disk used.
fn days(root: &Path, character: &str) -> std::io::Result<(Days, Usage)> {
    let plain = writer::days(root, character)?;
    let days = reader::days(root, character)?
        .into_iter()
        .map(|day| {
            let sizes: Vec<u64> = plain
                .iter()
                .filter(|path| writer::day_of(path).as_deref() == Some(day.as_str()))
                .map(|path| std::fs::metadata(path).map_or(0, |m| m.len()))
                .collect();
            let size = (!sizes.is_empty()).then(|| sizes.iter().sum());
            (day, size)
        })
        .collect();
    Ok((days, archive::usage(root, character)?))
}
