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
/// - the **date** is in the directory (`YYYY-MM-DD`, see [`date_dir`]) and in
///   the filename (see [`file_stamp`])
/// - the **zone** is the machine's, and one session does not cross zones
///
/// `logxml.lic` defaults to `%F %T %Z` -- full date, time and zone -- but it
/// offers that as a user-supplied `--timestamp` string rather than a considered
/// default, and its own filenames already carry the date too.
pub const TIME_FORMAT: &str = "%H:%M:%S%.3f";
// NOT a format string anything passes to a formatter. [`line_time`] builds the
// same shape by hand, because `jiff` needs no strftime pass to print four
// integers. This exists to NAME the format in one place, so the doc above and
// the hand-rolled builder cannot drift apart silently -- and `line_time`'s doc
// links here for exactly that reason. Review (PL-8) read it as dead surface,
// which is fair: a constant nothing reads looks like one. The test below is
// what makes it load-bearing instead of decorative.

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

/// Writes before the caller should roll to a new file. **A default, not a
/// rule** -- author's call, "I guess that should be editable."
///
/// 30,000 -- "somewhere around 1mb" -- from `logxml.lic` and `log.lic`, which
/// both use it and both let the user override it with `--lines`. Two decades
/// of operational experience chose this number; there is no reason to pick a
/// different one and every reason to match the files already on disk.
///
/// # This counts WRITES, and Lich counts LINES. They are not the same unit.
///
/// The name says "lines" because Lich's does, and for Lich that is accurate:
/// it rolls per logged line, so 30,000 of them land near the ~1 MB its
/// authors measured.
///
/// Cena's counter lives in [`crate::sink::SessionSink::wire`], which is called
/// **once per read chunk**, not once per line. A chunk is whatever the socket
/// returned, bounded by the session's read buffer:
///
/// ```text
/// $ grep -n 'const READ_BUF' crates/cena-session/src/actor.rs
/// 147:const READ_BUF: usize = 8 * 1024;
/// ```
///
/// So the worst case is 30,000 x 8,192 = **245,760,000 bytes (~234 MiB)** per
/// part, not ~1 MB. The real figure sits between the two -- a chunk is usually
/// far smaller than the buffer, and a busy line of combat is one small read --
/// but the CEILING is what a rotation bound is for, and this one is 234x the
/// number quoted beside it. UNVERIFIED against a real session; the ceiling is
/// arithmetic, not measurement.
///
/// Left as a write count rather than changed to a line count, because the
/// `.bytes` file's whole purpose is that chunk boundaries are preserved
/// verbatim (`SessionSink::wire`) -- counting lines would mean scanning for
/// newlines the format deliberately does not impose.
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

#[cfg(test)]
mod tests {
    use super::*;

    /// [`TIME_FORMAT`] and [`line_time`] describe the same shape.
    ///
    /// `line_time` formats by hand, so the constant is documentation that
    /// nothing executes -- exactly the kind that goes stale silently. Review
    /// finding PL-8 called the constant dead surface. It is not dead; it was
    /// merely unchecked, which looks the same from outside.
    ///
    /// This walks the strftime string and asserts the output matches it field
    /// for field, so editing either one alone fails here.
    #[test]
    fn line_time_matches_the_format_it_documents() {
        assert_eq!(
            TIME_FORMAT, "%H:%M:%S%.3f",
            "if the format changes, the expectations below must change with it"
        );
        let t = line_time();
        let (hms, millis) = t.split_once('.').expect("%.3f means a `.` separator");
        let fields: Vec<&str> = hms.split(':').collect();
        assert_eq!(fields.len(), 3, "%H:%M:%S is three fields: {t:?}");
        for (field, name) in fields.iter().zip(["%H", "%M", "%S"]) {
            assert_eq!(field.len(), 2, "{name} is zero-padded to 2 digits: {t:?}");
            assert!(
                field.bytes().all(|b| b.is_ascii_digit()),
                "{name} is digits: {t:?}"
            );
        }
        assert_eq!(millis.len(), 3, "%.3f is exactly 3 digits: {t:?}");
        assert!(
            millis.bytes().all(|b| b.is_ascii_digit()),
            "%.3f is digits: {t:?}"
        );
        assert!(
            !t.contains('-') && !t.contains('Z') && !t.contains('+'),
            "the format is TIME ONLY -- no date, no zone (author, 2026-09-18): {t:?}"
        );
    }

    /// The date really is in the directory and the filename, as [`TIME_FORMAT`]
    /// justifies omitting it on the grounds that it is.
    ///
    /// That justification cited `YYYY/MM` -- `logxml.lic`'s nesting, not
    /// Cena's. [`date_dir`] is one flat level. A doc that reasons from the
    /// wrong layout is the citation rot `plan/05` §-2 exists to stop, so the
    /// shapes are pinned rather than described.
    #[test]
    fn the_date_is_where_the_time_format_says_it_is() {
        let dir = date_dir();
        let parts: Vec<&str> = dir.split('-').collect();
        assert_eq!(
            parts.len(),
            3,
            "date_dir is flat YYYY-MM-DD, not nested: {dir:?}"
        );
        assert!(
            !dir.contains('/') && !dir.contains('\\'),
            "one level, so a day's captures list in one listing: {dir:?}"
        );
        assert_eq!(parts[0].len(), 4, "YYYY: {dir:?}");
        assert_eq!(parts[1].len(), 2, "MM: {dir:?}");
        assert_eq!(parts[2].len(), 2, "DD: {dir:?}");

        let stamp = file_stamp();
        let (date, time) = stamp
            .split_once('_')
            .unwrap_or_else(|| panic!("file_stamp is YYYY-MM-DD_HH-MM-SS: {stamp:?}"));
        assert_eq!(
            date, dir,
            "the filename's date must agree with the directory"
        );
        assert_eq!(
            time.split('-').count(),
            3,
            "HH-MM-SS, `-` because `:` is illegal on Windows: {stamp:?}"
        );
        assert!(
            !time.contains(':'),
            "`:` is not a legal Windows filename character: {stamp:?}"
        );
    }
}
