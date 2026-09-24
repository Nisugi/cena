//! Parsing `data/crit_tables.tsv` into `CritEntry` values.
//!
//! Split from `crit.rs` under Rule 4.1 (`plan/05:352-353`) -- move code down,
//! do not raise the cap.
//!
//! # Why a data file and not a `const` array
//!
//! `plan/13:125` already answers this for this exact row of its port table:
//! crit tables are "static data; **ships as data files**". The measurement
//! that makes it more than a preference is rustfmt:
//!
//! ```text
//! 200 entries written one-per-line  ->  203 lines
//! the same file after `cargo fmt`   -> 4223 lines   (21x)
//! ```
//!
//! Extrapolated to 2,394 entries that is ~50,500 lines, which at the 400-line
//! default cap (`crates/cena-arch-tests/tests/file_rules.rs:43`) needs ~127
//! generated files. The cap is not the thing to move -- `plan/05:352-353` says
//! move code down, not raise the cap -- and 127 files of unreviewable
//! generated Rust is not "down". One TSV of 2,395 lines is one line per entry,
//! so a data fix is a one-line diff.
//!
//! The file is embedded with `include_str!`, which this workspace bans by
//! default. The ban's own message names this case and says how to lift it:
//! extend `harness::SOURCE_EXTENSIONS` so the file is scanned, rather than
//! routing around the scan. That is what was done -- see the amendment in
//! `crates/cena-arch-tests/tests/file_rules.rs`. The data file is scanned by
//! every rule in the suite, so it can smuggle nothing.

use super::entry::CritEntry;
use super::types::{DamageType, Location, Position, SecondaryWound, WoundLocation};

/// The shipped table data, one entry per line, tab separated.
///
/// `include_bytes!` and `include!` remain banned; only `include_str!` of a
/// **scanned** data file is permitted, which is the narrowing the arch test
/// asked for.
const CRIT_TABLES_TSV: &str = include_str!("../../data/crit_tables.tsv");

/// The column order the extractor writes, asserted against the file's header
/// so a reordered regeneration cannot silently shift every field by one.
const COLUMNS: [&str; 19] = [
    "type",
    "location",
    "rank",
    "damage",
    "position",
    "fatal",
    "stunned",
    "amputated",
    "crippled",
    "sleeping",
    "dazed",
    "limb_favored",
    "roundtime",
    "silenced",
    "slowed",
    "wound_rank",
    "secondary_location",
    "secondary_wound_rank",
    "pattern",
];

/// Why a line of the data file could not be read.
///
/// A `Result` rather than a panic: `plan/06` §1.5 calls the parser never
/// panicking non-negotiable, and `plan/12` §5.5 says a panic kills one session
/// rather than the process. The shipped file is generated and tested, so these
/// are reachable only through a bad regeneration -- but a bad regeneration is
/// exactly when a message naming the line is worth having.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LoadError {
    /// 1-based line number in the data file, counting the header.
    pub line: usize,
    /// What was wrong with that line: an empty file, a header mismatch, a
    /// wrong field count, or a field that is empty or would not parse. Names
    /// the offending field and its text where there is one.
    pub reason: String,
}

impl std::fmt::Display for LoadError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "crit_tables.tsv:{}: {}", self.line, self.reason)
    }
}

impl std::error::Error for LoadError {}

/// Parse the shipped data file.
///
/// Returns entries in the file's order, which the extractor sorts by
/// `(type, location, rank)`. `CritTables::from_entries` re-sorts defensively
/// rather than trusting that.
///
/// # Errors
///
/// Returns `LoadError` naming the line if the shipped file's header has
/// changed shape, or a row has the wrong field count, an out-of-domain enum
/// spelling, a non-numeric number or a boolean that is not `0`/`1`. In a
/// released build these are reachable only through a bad regeneration.
pub fn shipped_entries() -> Result<Vec<CritEntry>, LoadError> {
    parse_tsv(CRIT_TABLES_TSV)
}

/// Parse TSV text in the extractor's format.
///
/// Public to the crate so the parity test can feed it a deliberately corrupted
/// copy and assert the failure, which is what stops this being decoration
/// (`plan/05` §0).
///
/// # Errors
///
/// Returns `LoadError` naming the 1-based line number and what was wrong with
/// it. See `shipped_entries` for the cases.
pub fn parse_tsv(text: &str) -> Result<Vec<CritEntry>, LoadError> {
    let mut lines = text.lines().enumerate();

    let (_, header) = lines.next().ok_or_else(|| LoadError {
        line: 0,
        reason: "file is empty; expected a header row".to_owned(),
    })?;
    let header_fields: Vec<&str> = header.split('\t').collect();
    if header_fields != COLUMNS {
        return Err(LoadError {
            line: 1,
            reason: format!("header is {header_fields:?}, expected {COLUMNS:?}"),
        });
    }

    let mut entries = Vec::new();
    for (index, line) in lines {
        let number = index + 1;
        if line.is_empty() {
            continue;
        }
        entries.push(parse_row(line, number)?);
    }
    Ok(entries)
}

fn parse_row(line: &str, number: usize) -> Result<CritEntry, LoadError> {
    let fields: Vec<&str> = line.split('\t').collect();
    if fields.len() != COLUMNS.len() {
        return Err(LoadError {
            line: number,
            reason: format!("{} fields, expected {}", fields.len(), COLUMNS.len()),
        });
    }
    let fail = |reason: String| LoadError {
        line: number,
        reason,
    };

    let damage_type = DamageType::parse(fields[0])
        .ok_or_else(|| fail(format!("unknown damage type {:?}", fields[0])))?;
    let location = Location::parse(fields[1])
        .ok_or_else(|| fail(format!("unknown location {:?}", fields[1])))?;

    let position = if fields[4].is_empty() {
        None
    } else {
        Some(
            Position::parse(fields[4])
                .ok_or_else(|| fail(format!("unknown position {:?}", fields[4])))?,
        )
    };

    // A secondary wound is present iff both its columns are; one without the
    // other is a malformed regeneration, not an entry with a default.
    let secondary_wound = match (fields[16].is_empty(), fields[17].is_empty()) {
        (true, true) => None,
        (false, false) => {
            let wound_location = WoundLocation::parse(fields[16]).ok_or_else(|| {
                fail(format!("unknown secondary wound location {:?}", fields[16]))
            })?;
            Some(SecondaryWound {
                location: wound_location,
                wound_rank: number_field(fields[17], "secondary_wound_rank", &fail)?,
            })
        }
        _ => {
            return Err(fail(
                "secondary wound has a location or a rank but not both".to_owned(),
            ));
        }
    };

    let pattern = fields[18];
    if pattern.is_empty() {
        return Err(fail("empty pattern".to_owned()));
    }

    Ok(CritEntry {
        damage_type,
        location,
        rank: number_field(fields[2], "rank", &fail)?,
        damage: number_field(fields[3], "damage", &fail)?,
        position,
        fatal: boolean_field(fields[5], "fatal", &fail)?,
        stunned: number_field(fields[6], "stunned", &fail)?,
        amputated: boolean_field(fields[7], "amputated", &fail)?,
        crippled: boolean_field(fields[8], "crippled", &fail)?,
        sleeping: boolean_field(fields[9], "sleeping", &fail)?,
        dazed: boolean_field(fields[10], "dazed", &fail)?,
        limb_favored: boolean_field(fields[11], "limb_favored", &fail)?,
        roundtime: number_field(fields[12], "roundtime", &fail)?,
        silenced: boolean_field(fields[13], "silenced", &fail)?,
        slowed: boolean_field(fields[14], "slowed", &fail)?,
        wound_rank: number_field(fields[15], "wound_rank", &fail)?,
        secondary_wound,
        pattern: pattern.to_owned(),
    })
}

/// `0` or `1` only. A permissive reader (`"true"`, `"yes"`, anything
/// non-empty) would let a shifted column read as a plausible boolean instead
/// of failing, which is the failure mode the field-count check above exists to
/// catch.
fn boolean_field(
    text: &str,
    field: &str,
    fail: &impl Fn(String) -> LoadError,
) -> Result<bool, LoadError> {
    match text {
        "0" => Ok(false),
        "1" => Ok(true),
        other => Err(fail(format!("{field} is {other:?}, expected 0 or 1"))),
    }
}

fn number_field<T: std::str::FromStr>(
    text: &str,
    field: &str,
    fail: &impl Fn(String) -> LoadError,
) -> Result<T, LoadError> {
    text.parse()
        .map_err(|_| fail(format!("{field} is {text:?}, which is not a number")))
}
