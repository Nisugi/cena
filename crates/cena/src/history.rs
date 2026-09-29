//! `;history`, the player log read back on Hydra's command line (`plan/25`
//! step 3): the days kept, the tail, a time window, and search.
//!
//! The join only, as `loot.rs` is for the ledger: the reads are
//! `cena_session::player_log::reader`'s, and what is known here is where the
//! log is and how a read looks on the command line. Each read asks the writer
//! to flush first, then runs on a blocking task, since it is file I/O that
//! may cover a year (`plan/25` §5).
//!
//! **Not `;log`**, which is the word the player log was named by: `log.lic`
//! is elanthia-online's logging script (`reference/scripts/scripts/log.lic`),
//! and Hydra's words come before the player's scripts (`commands.rs`), so a
//! `;log` here would take that one away from everyone who runs it.
//!
//! What it shows is said with `say_unlogged`: said the ordinary way, every
//! search would write its hits into today's log for the next one to find.
//!
//! # BUILT, NOT RUN
//!
//! As `loot.rs` says: compiled and tested, never executed against the game.

use std::path::Path;
use std::sync::Arc;
use std::time::Duration;

use cena_session::SessionHandle;
use cena_session::command::claimant::Claimed;
use cena_session::notice::{Body, Notice, NoticeKind};
use cena_session::player_log::reader::{self, Entry, Found, MAX_HITS, Moment, Pattern, Streams};
use cena_session::player_log::writer;
use cena_session::player_log::{archive, retention};
use cena_ui::settings::size;

use crate::commands::Commands;

/// The words `;history` knows.
const USAGE: &[&str] = &[
    "history                 the days kept",
    "history tail [<n>]      the last lines (20)",
    "history last <span>     a span back from now: 90s, 15m, 2h, 1d; a bare number is minutes",
    "history day <day> [<from> [<to>]]   a day (YYYY-MM-DD, today, yesterday), or from HH:MM to HH:MM of it",
    "history search <text>   lines holding the text, any case, newest first; /<regex>/ for an expression",
    "history export <day> [<day>]   those days, one to the other, written to one text file",
    "Add in:<stream>[,<stream>] to any: in:thoughts, in:combat, in:cmd.",
];

/// The most lines a window or a search shows. The reader caps what it reads
/// at [`MAX_HITS`]; a screen of them is already more than anyone reads.
const SHOWN: usize = 100;

/// The most days `;history` lists.
const DAYS_SHOWN: usize = 14;

/// One `;history` command, parsed.
#[derive(Clone, Debug, PartialEq)]
enum Command {
    /// The usage.
    Help,
    /// The days kept.
    Days,
    /// The last N lines.
    Tail(usize, Streams),
    /// A span back from now.
    Last(Duration, Streams),
    /// A day, or part of one.
    Day {
        /// Which.
        day: Day,
        /// From, `HH:MM[:SS]`; the day's start when `None`.
        from: Option<String>,
        /// Up to, `HH:MM[:SS]`; the day's end when `None`.
        to: Option<String>,
        /// Which tags.
        streams: Streams,
    },
    /// Lines holding a text or matching an expression.
    Search(Query, Streams),
    /// Days from one to the other, to a file.
    Export(Day, Day, Streams),
}

/// A day as the player names it; resolved against the clock when run.
#[derive(Clone, Debug, PartialEq)]
enum Day {
    Today,
    Yesterday,
    Named(String),
}

impl Day {
    /// `YYYY-MM-DD`, on the player's clock.
    fn resolve(self) -> String {
        match self {
            Day::Today => cena_platform::date_dir(),
            Day::Yesterday => cena_platform::stamp_ago(Duration::from_hours(24)).0,
            Day::Named(day) => day,
        }
    }
}

/// What a search looks for, before it is compiled.
#[derive(Clone, Debug, PartialEq)]
enum Query {
    Literal(String),
    Regex(String),
}

/// Parse a command line, without its symbol. `None` when the word is not
/// `history`; `Some(Err)` when it is and the rest is not understood.
fn parse(line: &str) -> Option<Result<Command, String>> {
    let mut words = line.split_whitespace();
    if !words.next()?.eq_ignore_ascii_case("history") {
        return None;
    }
    let (streams, rest): (Vec<&str>, Vec<&str>) = words.partition(|w| {
        w.get(..3)
            .is_some_and(|head| head.eq_ignore_ascii_case("in:"))
    });
    let streams = Streams::only(
        streams
            .iter()
            .flat_map(|w| w[3..].split(','))
            .filter(|s| !s.is_empty())
            .map(str::to_owned),
    );
    let usage = |what: String| format!("{what}. Say `history help` for the words.");
    let command = match rest.first().map(|w| w.to_ascii_lowercase()).as_deref() {
        None | Some("days") => Ok(Command::Days),
        Some("help") => Ok(Command::Help),
        Some("tail") => match rest.get(1) {
            None => Ok(Command::Tail(20, streams)),
            Some(n) => n
                .parse::<usize>()
                .ok()
                .filter(|n| (1..=MAX_HITS).contains(n))
                .map(|n| Command::Tail(n, streams))
                .ok_or_else(|| usage(format!("`{n}` is not a count from 1 to {MAX_HITS}"))),
        },
        Some("last") => rest
            .get(1)
            .and_then(|w| span(w))
            .map(|d| Command::Last(d, streams))
            .ok_or_else(|| usage("say how far back: 90s, 15m, 2h, 1d".to_owned())),
        Some("day") => day_command(&rest[1..], streams).map_err(usage),
        Some("export") => {
            let day = |word: Option<&&str>| match word.map(|w| w.to_ascii_lowercase()).as_deref() {
                Some("today") => Ok(Day::Today),
                Some("yesterday") => Ok(Day::Yesterday),
                Some(named) if is_day(named) => Ok(Day::Named(named.to_owned())),
                Some(other) => Err(usage(format!("`{other}` is not a day; say YYYY-MM-DD"))),
                None => Err(usage("say which day to export from".to_owned())),
            };
            day(rest.get(1)).and_then(|from| {
                let to = if rest.get(2).is_some() {
                    day(rest.get(2))?
                } else {
                    from.clone()
                };
                Ok(Command::Export(from, to, streams))
            })
        }
        Some("search") => {
            let text = rest[1..].join(" ");
            if text.is_empty() {
                Err(usage("say what to search for".to_owned()))
            } else if let Some(expr) = text
                .strip_prefix('/')
                .and_then(|t| t.strip_suffix('/'))
                .filter(|e| !e.is_empty())
            {
                Ok(Command::Search(Query::Regex(expr.to_owned()), streams))
            } else {
                Ok(Command::Search(Query::Literal(text), streams))
            }
        }
        Some(other) => Err(usage(format!("`{other}` is not a history word"))),
    };
    Some(command)
}

/// `90s`, `15m`, `2h`, `1d`, or minutes when bare.
fn span(word: &str) -> Option<Duration> {
    let word = word.to_ascii_lowercase();
    let (number, unit) = match word.find(|c: char| !c.is_ascii_digit()) {
        Some(at) => word.split_at(at),
        None => (word.as_str(), "m"),
    };
    let n: u64 = number.parse().ok().filter(|n| *n > 0)?;
    let seconds = match unit {
        "s" => 1,
        "m" => 60,
        "h" => 3600,
        "d" => 86_400,
        _ => return None,
    };
    Some(Duration::from_secs(n.checked_mul(seconds)?))
}

/// `day <day> [<from> [<to>]]`.
fn day_command(words: &[&str], streams: Streams) -> Result<Command, String> {
    let day = match words.first().map(|w| w.to_ascii_lowercase()).as_deref() {
        None | Some("today") => Day::Today,
        Some("yesterday") => Day::Yesterday,
        Some(named) if is_day(named) => Day::Named(named.to_owned()),
        Some(other) => return Err(format!("`{other}` is not a day; say YYYY-MM-DD")),
    };
    let time = |word: Option<&&str>| -> Result<Option<String>, String> {
        match word {
            None => Ok(None),
            Some(w) if is_time(w) => Ok(Some((*w).to_owned())),
            Some(w) => Err(format!("`{w}` is not a time; say HH:MM")),
        }
    };
    Ok(Command::Day {
        day,
        from: time(words.get(1))?,
        to: time(words.get(2))?,
        streams,
    })
}

/// `YYYY-MM-DD`.
fn is_day(word: &str) -> bool {
    word.len() == 10
        && word.bytes().enumerate().all(|(i, b)| {
            if i == 4 || i == 7 {
                b == b'-'
            } else {
                b.is_ascii_digit()
            }
        })
}

/// `HH:MM` or `HH:MM:SS`, as the log writes them, so they compare as text.
fn is_time(word: &str) -> bool {
    (word.len() == 5 || word.len() == 8)
        && word.bytes().enumerate().all(|(i, b)| {
            if i % 3 == 2 {
                b == b':'
            } else {
                b.is_ascii_digit()
            }
        })
}

/// Register `;history` on `character`'s command line.
pub(crate) fn open(handle: &SessionHandle, commands: &Commands, character: &str, game: &str) {
    let handler = handle.clone();
    let character = character.to_owned();
    let login = format!("{game}:{character}");
    commands.history(Arc::new(move |line: &str| {
        let command = match parse(line)? {
            Ok(command) => command,
            Err(why) => {
                handler.say(Notice::line(NoticeKind::Error, format!("History: {why}")));
                return Some(Claimed::Done);
            }
        };
        let (handle, character, login) = (handler.clone(), character.clone(), login.clone());
        tokio::spawn(async move {
            // What the writer holds is the last second of play, which is the
            // part a player reading back is most likely to want.
            handle.flush_player_log().await;
            let read = tokio::task::spawn_blocking(move || {
                let keep_days = crate::general::log_settings(
                    &cena_session::character_store::data_dir(),
                    &login,
                )
                .keep_days
                .unwrap_or(0);
                run(&writer::root(), &character, keep_days, command)
            })
            .await;
            match read {
                Ok(Ok(lines)) => handle.say_unlogged(Notice {
                    kind: NoticeKind::Info,
                    body: Body::Lines(lines),
                }),
                Ok(Err(why)) => {
                    handle.say(Notice::line(NoticeKind::Error, format!("History: {why}")));
                }
                Err(_) => handle.say(Notice::line(
                    NoticeKind::Error,
                    "History: the read stopped before it finished.",
                )),
            }
        });
        Some(Claimed::Done)
    }));
}

/// Run one command against `character`'s log under `root`, and give the
/// lines to say.
///
/// `keep_days` is how many days the character's settings keep (0, forever),
/// for the day list to say what the next login removes.
fn run(
    root: &Path,
    character: &str,
    keep_days: u32,
    command: Command,
) -> Result<Vec<String>, String> {
    let io = |e: std::io::Error| e.to_string();
    match command {
        Command::Help => Ok(USAGE.iter().map(|&l| l.to_owned()).collect()),
        Command::Days => days(root, character, keep_days).map_err(io),
        Command::Tail(count, streams) => {
            let entries = reader::tail(root, character, count, &streams).map_err(io)?;
            if entries.is_empty() {
                return Ok(vec![format!(
                    "History: nothing kept for {character}{}.",
                    of(&streams)
                )]);
            }
            let mut lines = vec![format!(
                "History, the last {} lines{}:",
                entries.len(),
                of(&streams)
            )];
            lines.extend(entries.iter().map(|e| render(e, true)));
            Ok(lines)
        }
        Command::Last(ago, streams) => {
            let (day, at) = cena_platform::stamp_ago(ago);
            let from = Moment::new(day, at);
            let found = reader::window(
                root,
                character,
                &from,
                &end_of(cena_platform::date_dir()),
                &streams,
            )
            .map_err(io)?;
            let spans_days = from.day != cena_platform::date_dir();
            Ok(shown(
                &found,
                &format!(
                    "History since {} {}{}",
                    from.day,
                    clock(&from.at),
                    of(&streams)
                ),
                spans_days,
            ))
        }
        Command::Day {
            day,
            from,
            to,
            streams,
        } => {
            let day = day.resolve();
            let start = Moment::new(day.clone(), from.clone().unwrap_or_default());
            let end = to
                .clone()
                .map_or_else(|| end_of(day.clone()), |to| Moment::new(day.clone(), to));
            let found = reader::window(root, character, &start, &end, &streams).map_err(io)?;
            let label = match (from, to) {
                (None, _) => format!("History, {day}"),
                (Some(from), None) => format!("History, {day} from {from}"),
                (Some(from), Some(to)) => format!("History, {day} {from} to {to}"),
            };
            Ok(shown(&found, &format!("{label}{}", of(&streams)), false))
        }
        Command::Export(from, to, streams) => {
            let (mut from, mut to) = (from.resolve(), to.resolve());
            if from > to {
                std::mem::swap(&mut from, &mut to);
            }
            let out = reader::export_path(root, character, (&from, &to));
            let done = reader::export(root, character, (&from, &to), &streams, &out).map_err(io)?;
            Ok(vec![format!(
                "History: {} lines from {} days{} written to {}",
                done.lines,
                done.days,
                of(&streams),
                done.path.display()
            )])
        }
        Command::Search(query, streams) => {
            let (pattern, said) = match &query {
                Query::Literal(text) => (Pattern::literal(text), format!("\"{text}\"")),
                Query::Regex(expr) => (Pattern::regex(expr), format!("/{expr}/")),
            };
            let pattern = pattern.map_err(|e| format!("not an expression: {e}"))?;
            let found = reader::search(root, character, &pattern, &streams, None).map_err(io)?;
            Ok(shown(
                &found,
                &format!("History, lines with {said}{}, newest first", of(&streams)),
                true,
            ))
        }
    }
}

/// The end of `day`: `24` sorts after every stamp the day can hold.
fn end_of(day: String) -> Moment {
    Moment::new(day, "24")
}

/// `the days kept`, newest first, with each file's size.
fn days(root: &Path, character: &str, keep_days: u32) -> std::io::Result<Vec<String>> {
    let days = reader::days(root, character)?;
    let Some(oldest) = days.last() else {
        return Ok(vec![format!("History: nothing kept for {character} yet.")]);
    };
    let usage = archive::usage(root, character)?;
    let mut lines = vec![
        format!(
            "History for {character}: {} days, back to {oldest}; {} in all, {} plain and {} archived.",
            usage.days,
            size(usage.total()),
            size(usage.plain),
            size(usage.archived)
        ),
        format!("  {}", writer::dir(root, character).display()),
    ];
    // A day may be two files (a cut where an Eastern week or month begins),
    // or in an archive, where only the whole archive has a size.
    let plain = writer::days(root, character)?;
    for day in days.iter().take(DAYS_SHOWN) {
        let sizes: Vec<u64> = plain
            .iter()
            .filter(|path| writer::day_of(path).as_deref() == Some(day.as_str()))
            .map(|path| std::fs::metadata(path).map_or(0, |m| m.len()))
            .collect();
        if sizes.is_empty() {
            lines.push(format!("  {day}  archived"));
        } else {
            let size: u64 = sizes.iter().sum();
            lines.push(format!("  {day}  {:>6} KB", size.div_ceil(1024)));
        }
    }
    if days.len() > DAYS_SHOWN {
        lines.push(format!("  and {} more", days.len() - DAYS_SHOWN));
    }
    if keep_days > 0 {
        let doomed = retention::doomed(root, character, keep_days, &cena_platform::date_dir())?;
        lines.push(format!(
            "Kept {keep_days} days. {}",
            retention::preview(&doomed)
        ));
    }
    Ok(lines)
}

/// A window's or a search's lines under `label`: the first [`SHOWN`] of
/// them, and how many more there were.
fn shown(found: &Found, label: &str, with_day: bool) -> Vec<String> {
    if found.entries.is_empty() {
        return vec![format!("{label}: nothing.")];
    }
    let mut lines = vec![format!("{label}:")];
    lines.extend(
        found
            .entries
            .iter()
            .take(SHOWN)
            .map(|e| render(e, with_day)),
    );
    let left = found.entries.len().saturating_sub(SHOWN);
    if left > 0 || found.more {
        let more = if found.more {
            format!("{left} more, and past {MAX_HITS} the read stopped")
        } else {
            format!("{left} more")
        };
        lines.push(format!(
            "  ... {more}; narrow it with a time or in:<stream>."
        ));
    }
    lines
}

/// One line as the command line shows it: `[06:47:12][main] You see a rock.`,
/// the day first when a read can cover more than one.
fn render(entry: &Entry, with_day: bool) -> String {
    let at = clock(&entry.at);
    if with_day {
        format!("{} {at} [{}] {}", entry.day, entry.stream, entry.text)
    } else {
        format!("{at} [{}] {}", entry.stream, entry.text)
    }
}

/// `HH:MM:SS`, without the milliseconds a reader does not need.
fn clock(at: &str) -> &str {
    at.get(..8).unwrap_or(at)
}

/// ` in thoughts, death` when a read keeps only some tags.
fn of(streams: &Streams) -> String {
    if *streams == Streams::all() {
        String::new()
    } else {
        format!(" in {}", streams.names().join(", "))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parsed(line: &str) -> Command {
        parse(line).expect("a history line").expect("understood")
    }

    #[test]
    fn the_words_parse() {
        assert_eq!(parsed("history"), Command::Days);
        assert_eq!(parsed("history help"), Command::Help);
        assert_eq!(parsed("history tail"), Command::Tail(20, Streams::all()));
        assert_eq!(
            parsed("history tail 50 in:thoughts,death"),
            Command::Tail(50, Streams::only(["thoughts", "death"]))
        );
        assert_eq!(
            parsed("history last 15m"),
            Command::Last(Duration::from_mins(15), Streams::all())
        );
        assert_eq!(
            parsed("history last 10"),
            Command::Last(Duration::from_mins(10), Streams::all()),
            "a bare number is minutes"
        );
        assert_eq!(
            parsed("history day 2026-09-21 06:00 06:30"),
            Command::Day {
                day: Day::Named("2026-09-21".to_owned()),
                from: Some("06:00".to_owned()),
                to: Some("06:30".to_owned()),
                streams: Streams::all(),
            }
        );
        assert_eq!(
            parsed("history search In:combat Kobold dies"),
            Command::Search(
                Query::Literal("Kobold dies".to_owned()),
                Streams::only(["combat"])
            ),
            "in: is taken out wherever it is, and the text keeps its case"
        );
        assert_eq!(
            parsed("history search /^You gain \\d+/"),
            Command::Search(Query::Regex("^You gain \\d+".to_owned()), Streams::all())
        );
    }

    #[test]
    fn other_words_are_not_history_and_bad_ones_are_told() {
        assert_eq!(parse("loot"), None);
        assert_eq!(parse("historyx"), None);
        for bad in [
            "history nosuch",
            "history tail 0",
            "history tail many",
            "history last",
            "history last 5w",
            "history day 21-09-2026",
            "history day today 6pm",
            "history search",
            "history search in:main",
        ] {
            assert!(parse(bad).is_some_and(|r| r.is_err()), "{bad}");
        }
    }

    /// A search against a real day-file, through the same `run` the command
    /// uses, and a bad expression told rather than panicked.
    #[test]
    fn a_search_reads_the_log_and_a_bad_expression_is_told() {
        let root = std::env::temp_dir().join(format!("cena-history-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        let path = writer::day_path(&root, "Nisugi", "2026-09-21");
        std::fs::create_dir_all(path.parent().expect("a parent")).expect("mkdir");
        std::fs::write(
            &path,
            "[06:47:12.481][main] You see a rock.\n[06:47:13.000][main] A kobold dies.\n",
        )
        .expect("write");

        let lines = run(&root, "Nisugi", 0, parsed("history search ROCK")).expect("run");
        assert_eq!(
            lines,
            [
                "History, lines with \"ROCK\", newest first:",
                "2026-09-21 06:47:12 [main] You see a rock."
            ]
        );
        let lines = run(
            &root,
            "Nisugi",
            0,
            parsed("history day 2026-09-21 06:47:13"),
        )
        .expect("run");
        assert_eq!(lines[1], "06:47:13 [main] A kobold dies.");
        assert!(run(&root, "Nisugi", 0, parsed("history search /(/")).is_err());
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn export_names_its_days_and_writes_them() {
        assert_eq!(
            parsed("history export 2026-09-01 2026-09-07 in:thoughts"),
            Command::Export(
                Day::Named("2026-09-01".to_owned()),
                Day::Named("2026-09-07".to_owned()),
                Streams::only(["thoughts"])
            )
        );
        assert_eq!(
            parsed("history export yesterday"),
            Command::Export(Day::Yesterday, Day::Yesterday, Streams::all())
        );
        assert!(parse("history export").is_some_and(|r| r.is_err()));
        assert!(parse("history export sometime").is_some_and(|r| r.is_err()));

        let root = std::env::temp_dir().join(format!("cena-history-export-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        let path = writer::day_path(&root, "Nisugi", "2026-09-21");
        std::fs::create_dir_all(path.parent().expect("a parent")).expect("mkdir");
        std::fs::write(
            &path,
            "[06:47:12.481][main] You see a rock.
",
        )
        .expect("write");
        // Named backwards, the days are put in order.
        let said = run(
            &root,
            "Nisugi",
            0,
            parsed("history export 2026-09-22 2026-09-21"),
        )
        .expect("run");
        assert!(
            said[0].starts_with("History: 1 lines from 1 days"),
            "{said:?}"
        );
        let out = reader::export_path(&root, "Nisugi", ("2026-09-21", "2026-09-22"));
        assert_eq!(
            std::fs::read_to_string(out).expect("the export"),
            "2026-09-21 06:47:12.481 [main] You see a rock.
"
        );
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn more_than_a_screen_says_how_many_were_left_out() {
        let entry = Entry {
            day: "2026-09-21".to_owned(),
            at: "06:00:00.000".to_owned(),
            stream: "main".to_owned(),
            text: "x".to_owned(),
        };
        let found = Found {
            entries: vec![entry; SHOWN + 3],
            more: false,
        };
        let lines = shown(&found, "History", false);
        assert_eq!(lines.len(), 1 + SHOWN + 1);
        assert!(
            lines.last().is_some_and(|l| l.contains("3 more")),
            "{lines:?}"
        );
    }
}
