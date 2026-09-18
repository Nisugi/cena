//! Where the wire log goes, how big its files get, and whether they carry
//! timestamps.
//!
//! **All three are defaults, not rules** -- author's call, 2026-09-18: *"It
//! should have a default location that can be changed later"* and *"I guess
//! that should be editable."*
//!
//! Environment variables rather than a config file, because `plan/12` §7.1
//! puts config in the Out column for M1 and an env var is the smallest thing
//! that honours "changeable" without inventing a format a real config would
//! have to replace. Every one of these moves into config when config exists.
//!
//! | Variable | Controls | Default |
//! |---|---|---|
//! | `CENA_LOG_DIR` | where logs are written | `./logs` |
//! | `CENA_LOG_LINES` | lines before rotating | 30,000 |
//! | `CENA_LOG_TIMESTAMPS` | stamp the bytes file | off |

use std::path::PathBuf;

/// The timestamp format: **time only**.
///
/// Author's call, 2026-09-18: *"timestamps should just be time, not time zone
/// or date."* Both are already known from context and repeating them is waste
/// at 30,000 lines a file:
///
/// - the **date** is in the directory (`YYYY/MM`) and in the filename
/// - the **zone** is the machine's, and one session does not cross zones
///
/// `logxml.lic` defaults to `%F %T %Z` -- full date, time and zone -- but it
/// offers that as a user-supplied `--timestamp` string rather than a considered
/// default, and its own filenames already carry the date too.
pub const TIME_FORMAT: &str = "%H:%M:%S%.3f";

/// The environment variable that names the log directory.
///
/// **Configuration, not a constant.** The author's call was *"It should have a
/// default location that can be changed later"*, and there is no config file
/// yet -- `plan/12` §7.1 puts config in the Out column for M1. An environment
/// variable is the smallest thing that honours "changeable" without inventing
/// a config format a later one would have to replace.
///
/// It also keeps a machine-specific absolute path out of a shipped constant.
/// The first version of this hardcoded the author's own development path, and
/// `game_names_outside_game_modules_are_flagged` went red on it -- correctly,
/// though for a different reason than the one that matters here.
pub const LOG_DIR_ENV: &str = "CENA_LOG_DIR";

/// Where logs go when [`LOG_DIR_ENV`] is unset: `./logs` beside the binary.
///
/// Relative deliberately. An absolute default is right for exactly one machine
/// and silently wrong everywhere else.
pub const DEFAULT_LOG_DIR: &str = "logs";

/// The configured log directory, or the default.
///
/// The author's development path is set with, in PowerShell:
///
/// ```text
/// $env:CENA_LOG_DIR = "E:\Gemstone\data\cena_logs"
/// ```
#[must_use]
pub fn log_dir() -> PathBuf {
    std::env::var_os(LOG_DIR_ENV).map_or_else(|| PathBuf::from(DEFAULT_LOG_DIR), PathBuf::from)
}

/// Lines before the caller should roll to a new file. **A default, not a
/// rule** -- author's call, "I guess that should be editable."
///
/// 30,000 -- "somewhere around 1mb" -- from `logxml.lic` and `log.lic`, which
/// both use it and both let the user override it with `--lines`. Two decades
/// of operational experience chose this number; there is no reason to pick a
/// different one and every reason to match the files already on disk.
///
/// That ~1 MB is **their** measurement, of their format. This file wraps
/// client input in markers and writes inbound verbatim, so it should land in
/// the same neighbourhood -- UNVERIFIED until a real session is measured.
///
/// Overridable via [`ROTATE_ENV`], and it moves into config when config
/// exists.
pub const ROTATE_AFTER_LINES: u64 = 30_000;

/// Environment override for [`ROTATE_AFTER_LINES`].
pub const ROTATE_ENV: &str = "CENA_LOG_LINES";

/// Set to `1` to stamp each line of the **bytes** file with a wall-clock time.
///
/// # Off by default, and the reason is stronger here than it is for Lich
///
/// `logxml.lic` and `log.lic` both default timestamps off and take
/// `--timestamp="%F %T %Z"` to turn them on, so this matches the precedent.
/// But for Cena there is a second reason: **the bytes file is replay input.**
///
/// `plan/12` criterion 7 replays a recording and asserts it reproduces itself.
/// [`Recorder`](crate::record::Recorder)'s own docs call a wall clock "the
/// single easiest way to make a replay non-deterministic" -- two captures of
/// the same session would differ on every line, and a diff of two runs would
/// be pure noise.
///
/// So this exists for the case where someone is chasing a timing question and
/// knowingly trades replay-diffability for it. The `.log` events file is
/// timestamped unconditionally: nothing replays it.
pub const BYTES_TIMESTAMP_ENV: &str = "CENA_LOG_TIMESTAMPS";

/// Whether the bytes file should carry per-line timestamps.
#[must_use]
pub fn bytes_timestamps_enabled() -> bool {
    std::env::var(BYTES_TIMESTAMP_ENV).is_ok_and(|v| v.trim() == "1")
}

/// The configured rotation threshold, or the default.
///
/// An unparseable value falls back rather than failing: a typo in an env var
/// must not stop a session logging.
#[must_use]
pub fn rotate_after_lines() -> u64 {
    std::env::var(ROTATE_ENV)
        .ok()
        .and_then(|v| v.trim().parse::<u64>().ok())
        .filter(|n| *n > 0)
        .unwrap_or(ROTATE_AFTER_LINES)
}

/// Opens a run of client input in the bytes file.
///
/// Byte-identical to `logxml.lic`'s marker. That script wrote the 49.55 GB
/// corpus this project's fixtures are cut from, so a Cena capture stays
/// readable by whatever already reads those.
pub(super) const CLIENT_OPEN: &[u8] = b"<!-- CLIENT -->";

/// Closes a run of client input. See [`CLIENT_OPEN`].
pub(super) const CLIENT_CLOSE: &[u8] = b"<!-- ENDCLIENT -->";

/// The stamp that names a session's files: `YYYY-MM-DD_HH-MM-SS`.
///
/// Author's call, 2026-09-18: *"file names are usually saved with date and
/// time start."* Same shape as `logxml.lic`'s
/// (`2026-09-18_15-49-13.xml`), so a directory of Cena captures sorts and
/// reads like the corpus beside it.
///
/// `-` rather than `:` in the time: `:` is not a legal filename character on
/// Windows, which is the development platform.
///
/// This is also why [`TIME_FORMAT`] carries no date -- it is already here, and
/// repeating it on 30,000 lines is waste.
#[must_use]
pub fn file_stamp() -> String {
    let now = jiff::Zoned::now();
    format!(
        "{:04}-{:02}-{:02}_{:02}-{:02}-{:02}",
        now.year(),
        now.month(),
        now.day(),
        now.hour(),
        now.minute(),
        now.second()
    )
}

/// The date directory a session's files live under: `YYYY-MM-DD`.
///
/// `logxml.lic` nests `year/month`; this is one level, because a Cena session
/// produces two files rather than a continuous stream and a day's worth stays
/// readable in one listing. Revisit when a day's directory stops being
/// scannable.
#[must_use]
pub fn date_dir() -> String {
    let now = jiff::Zoned::now();
    format!("{:04}-{:02}-{:02}", now.year(), now.month(), now.day())
}

/// Wall-clock time for one log line: `HH:MM:SS.mmm`, per [`TIME_FORMAT`].
#[must_use]
pub fn line_time() -> String {
    let now = jiff::Zoned::now();
    format!(
        "{:02}:{:02}:{:02}.{:03}",
        now.hour(),
        now.minute(),
        now.second(),
        now.millisecond()
    )
}
