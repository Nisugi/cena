//! `;loot`, the ledger's reports on Hydra's command line: `summary`,
//! `recent`, `boxes`, `creatures` and `cap` (`plan/34` Stage 3).
//!
//! The join only, as `hunt.rs` is for the hunt: the queries are
//! `cena_session::ledger::report`'s, the periods are [`period`]'s, and what is
//! known here is where the character's database is and how a report reads on
//! the command line. Reports open the database read-only for one command and
//! close it, on a blocking task, while the recorders keep writing.
//!
//! # BUILT, NOT RUN
//!
//! As `hunt.rs` says: compiled and tested, never executed against the game.
//! What it calls is tested in `cena-session`; the parsing and the periods are
//! tested here.

pub(crate) mod period;

use std::path::{Path, PathBuf};
use std::sync::Arc;

use cena_session::command::claimant::Claimed;
use cena_session::ledger::report::{Period, Reader};
use cena_session::{Notice, NoticeKind, SessionHandle};

use crate::commands::Commands;
use period::{date_label, time_label};

/// The words `;loot` knows.
const USAGE: &str = "loot [summary [today|month|<hours>]] | loot recent [<n>] [<type>] | \
                     loot boxes [<n>] | loot creatures [<n>] [today|<hours>] | \
                     loot cap [last|<YYYY-MM>]";

/// One `;loot` command, parsed.
#[derive(Clone, Debug, PartialEq)]
pub(crate) enum Command {
    /// `summary`, over a period.
    Summary(Span),
    /// `recent [n] [type]`.
    Recent {
        /// How many.
        limit: usize,
        /// A `gameobj` type to keep, if one was named.
        kind: Option<String>,
    },
    /// `boxes [n]`.
    Boxes(usize),
    /// `creatures [n] [period]`.
    Creatures {
        /// How many.
        limit: usize,
        /// Over which period.
        span: Span,
    },
    /// `cap [last|YYYY-MM]`.
    Cap(Month),
}

/// A period as the player names it; resolved against the clock when run.
#[derive(Clone, Debug, PartialEq)]
pub(crate) enum Span {
    /// Since midnight Eastern.
    Today,
    /// Since the first of the month.
    ThisMonth,
    /// The last N hours.
    Hours(i64),
}

/// A month as the player names it.
#[derive(Clone, Debug, PartialEq)]
pub(crate) enum Month {
    /// The one we are in.
    This,
    /// The one before it.
    Last,
    /// `YYYY-MM`.
    Named(i64, i64),
}

/// Parse a command line, without its symbol. `None` when the word is not
/// `loot`; `Some(Err)` when it is and the rest is not understood.
pub(crate) fn parse(line: &str) -> Option<Result<Command, String>> {
    let mut words = line.split_whitespace();
    if !words.next()?.eq_ignore_ascii_case("loot") {
        return None;
    }
    let rest: Vec<&str> = words.collect();
    let usage = |what: &str| format!("{what}. Usage: {USAGE}");
    let span = |word: Option<&&str>| -> Result<Span, String> {
        match word.map(|w| w.to_ascii_lowercase()) {
            None => Ok(Span::Hours(24)),
            Some(w) if w == "today" || w == "midnight" => Ok(Span::Today),
            Some(w) if w == "month" || w == "monthly" => Ok(Span::ThisMonth),
            Some(w) => w
                .parse::<i64>()
                .ok()
                .filter(|h| *h > 0)
                .map(Span::Hours)
                .ok_or_else(|| usage(&format!("`{w}` is not a period"))),
        }
    };
    let count = |word: Option<&&str>, default: usize| -> Option<usize> {
        word.map_or(Some(default), |w| w.parse().ok().filter(|n| *n > 0))
    };
    let command = match rest.first().map(|w| w.to_ascii_lowercase()).as_deref() {
        None | Some("summary") => span(rest.get(1)).map(Command::Summary),
        Some("recent") => {
            // `recent 5 gem`, `recent gem`, `recent 5`: a count is digits.
            let (limit, kind) = match rest.get(1) {
                Some(w) if w.chars().all(|c| c.is_ascii_digit()) => {
                    (count(Some(w), 20), rest.get(2))
                }
                other => (Some(20), other),
            };
            limit.map_or_else(
                || Err(usage("a count must be a positive number")),
                |limit| {
                    Ok(Command::Recent {
                        limit,
                        kind: kind.map(|k| singular(k)),
                    })
                },
            )
        }
        Some("boxes") => count(rest.get(1), 10)
            .map(Command::Boxes)
            .ok_or_else(|| usage("a count must be a positive number")),
        Some("creatures") => {
            let (limit, when) = match rest.get(1) {
                Some(w) if w.chars().all(|c| c.is_ascii_digit()) && rest.get(2).is_some() => {
                    (count(Some(w), 10), rest.get(2))
                }
                other => (Some(10), other),
            };
            match (limit, span(when)) {
                (Some(limit), Ok(span)) => Ok(Command::Creatures { limit, span }),
                (None, _) => Err(usage("a count must be a positive number")),
                (_, Err(e)) => Err(e),
            }
        }
        Some("cap" | "lootcap") => match rest.get(1).map(|w| w.to_ascii_lowercase()) {
            None => Ok(Command::Cap(Month::This)),
            Some(w) if matches!(w.as_str(), "last" | "previous" | "prev") => {
                Ok(Command::Cap(Month::Last))
            }
            Some(w) => w
                .split_once('-')
                .and_then(|(y, m)| Some((y.parse().ok()?, m.parse().ok()?)))
                .filter(|(_, m)| (1..=12).contains(m))
                .map(|(y, m)| Command::Cap(Month::Named(y, m)))
                .ok_or_else(|| usage(&format!("`{w}` is not a month; say `last` or `YYYY-MM`"))),
        },
        Some(other) => Err(usage(&format!("`{other}` is not a report"))),
    };
    Some(command)
}

/// A `gameobj` type as the player may pluralise it: `gems` is `gem`, `boxes`
/// is `box`, and a type that is not plural (`jewelry`, `magic`) is itself.
fn singular(word: &str) -> String {
    let word = word.to_ascii_lowercase();
    if let Some(stem) = word.strip_suffix("xes") {
        format!("{stem}x")
    } else if word.ends_with("ss") || !word.ends_with('s') {
        word
    } else {
        word[..word.len() - 1].to_owned()
    }
}

/// Register `;loot` on the command line, reading `database`.
pub(crate) fn open(handle: &SessionHandle, commands: &Commands, database: PathBuf) {
    let handler = handle.clone();
    commands.loot(Arc::new(move |line: &str| {
        let command = match parse(line)? {
            Ok(command) => command,
            Err(why) => {
                handler.say(Notice::line(NoticeKind::Error, format!("Loot: {why}")));
                return Some(Claimed::Done);
            }
        };
        let (handle, database) = (handler.clone(), database.clone());
        tokio::task::spawn_blocking(move || {
            let now = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map_or(0, |d| i64::try_from(d.as_secs()).unwrap_or(i64::MAX));
            match run(&database, command, now) {
                Ok(lines) => handle.say(Notice::table(NoticeKind::Info, lines)),
                Err(why) => handle.say(Notice::line(NoticeKind::Error, format!("Loot: {why}"))),
            }
        });
        Some(Claimed::Done)
    }));
    eprintln!("[loot] ready: {USAGE}");
}

fn resolve(span: &Span, now: i64) -> (Period, String) {
    match span {
        Span::Today => (period::today(now), "since midnight".to_owned()),
        Span::ThisMonth => (period::this_month(now), "this month".to_owned()),
        Span::Hours(h) => (period::last_hours(now, *h), format!("the last {h} hours")),
    }
}

/// Run one command against the database and give the lines to say.
fn run(database: &Path, command: Command, now: i64) -> Result<Vec<String>, String> {
    if !database.is_file() {
        return Err(format!(
            "nothing recorded yet ({}). Recording is on with --record.",
            database.display()
        ));
    }
    let reader = Reader::open(database).map_err(|e| e.to_string())?;
    let lines = match command {
        Command::Summary(span) => {
            let (period, label) = resolve(&span, now);
            summary(&reader.summary(period).map_err(|e| e.to_string())?, &label)
        }
        Command::Recent { limit, kind } => recent(
            &reader
                .recent(limit, kind.as_deref())
                .map_err(|e| e.to_string())?,
            kind.as_deref(),
        ),
        Command::Boxes(limit) => boxes(&reader.boxes(limit).map_err(|e| e.to_string())?),
        Command::Creatures { limit, span } => {
            let (period, label) = resolve(&span, now);
            creatures(
                &reader.creatures(period, limit).map_err(|e| e.to_string())?,
                &label,
            )
        }
        Command::Cap(month) => {
            let period = match month {
                Month::This => period::this_month(now),
                Month::Last => period::last_month(now),
                Month::Named(y, m) => period::month(y, m),
            };
            cap(&reader.cap(period).map_err(|e| e.to_string())?, period)
        }
    };
    Ok(lines)
}

/// `1,234,567`.
fn silver(n: i64) -> String {
    let digits = n.unsigned_abs().to_string();
    let mut out = String::with_capacity(digits.len() + digits.len() / 3 + 1);
    for (i, c) in digits.chars().enumerate() {
        if i > 0 && (digits.len() - i).is_multiple_of(3) {
            out.push(',');
        }
        out.push(c);
    }
    if n < 0 {
        out.insert(0, '-');
    }
    out
}

fn summary(s: &cena_session::ledger::report::Summary, label: &str) -> Vec<String> {
    let mut lines = vec![
        format!("Loot, {label}:"),
        format!(
            "  searched {} corpses for {} silver; opened {} boxes for {}; {} skins; {} items",
            s.searches,
            silver(s.silvers_search),
            s.boxes_opened,
            silver(s.silvers_boxes),
            s.skins,
            s.items
        ),
    ];
    if !s.sales.is_empty() {
        let sold: Vec<String> = s
            .sales
            .iter()
            .map(|(who, total)| format!("{who} {}", silver(*total)))
            .collect();
        lines.push(format!("  sold: {}", sold.join(", ")));
    }
    if s.pool_fees + s.pool_tips > 0 {
        lines.push(format!(
            "  locksmith: {} in fees, {} in tips",
            silver(s.pool_fees),
            silver(s.pool_tips)
        ));
    }
    if s.deposits + s.withdrawals > 0 {
        lines.push(format!(
            "  bank: deposited {}, withdrew {}",
            silver(s.deposits),
            silver(s.withdrawals)
        ));
    }
    if s.bounty_points + s.bounty_silver + s.bounty_experience > 0 {
        lines.push(format!(
            "  bounty: {} points, {} experience, {} silver",
            silver(s.bounty_points),
            silver(s.bounty_experience),
            silver(s.bounty_silver)
        ));
    }
    lines
}

fn recent(items: &[cena_session::ledger::report::Item], kind: Option<&str>) -> Vec<String> {
    if items.is_empty() {
        return vec![kind.map_or_else(
            || "Loot: nothing recorded yet.".to_owned(),
            |k| format!("Loot: no {k} recorded yet."),
        )];
    }
    let mut lines = vec![format!(
        "Loot, the last {} {}:",
        items.len(),
        kind.unwrap_or("items")
    )];
    for item in items {
        let fate = match (&item.sold, &item.refused, &item.appraised) {
            (Some(sold), _, _) => format!(
                "sold {} to {}",
                silver(*sold),
                item.sold_to.as_deref().unwrap_or("someone")
            ),
            (None, Some(refused), _) => refused.replace('_', " "),
            (None, None, Some(value)) => format!("worth ~{}", silver(*value)),
            (None, None, None) => String::new(),
        };
        lines.push(
            format!("  {} {:<40} {fate}", time_label(item.at), item.name)
                .trim_end()
                .to_owned(),
        );
    }
    lines
}

fn boxes(rows: &[cena_session::ledger::report::BoxRow]) -> Vec<String> {
    if rows.is_empty() {
        return vec!["Loot: no boxes recorded yet.".to_owned()];
    }
    let mut lines = vec![format!("Loot, the last {} boxes:", rows.len())];
    for b in rows {
        let from = b
            .source
            .as_deref()
            .map_or(String::new(), |s| format!(" off {s}"));
        let fate = match (b.silvers, b.pool_dropped_at, b.returned_at) {
            (Some(silvers), _, _) => {
                format!("opened: {} silver, {} items", silver(silvers), b.contents)
            }
            (None, Some(_) | None, Some(_)) => "back from the pool, unopened".to_owned(),
            (None, Some(_), None) => "in the pool".to_owned(),
            (None, None, None) => "unopened".to_owned(),
        };
        lines.push(format!("  {} {}{from}: {fate}", time_label(b.at), b.name));
    }
    lines
}

fn creatures(rows: &[cena_session::ledger::report::CreatureRow], label: &str) -> Vec<String> {
    if rows.is_empty() {
        return vec![format!("Loot: no corpses searched {label}.")];
    }
    let mut lines = vec![format!("Loot by creature, {label}:")];
    for c in rows {
        lines.push(format!(
            "  {:<36} {:>4} searched  {:>10} silver  {:>3} items",
            c.name,
            c.searches,
            silver(c.silvers),
            c.items
        ));
    }
    lines
}

fn cap(c: &cena_session::ledger::report::Cap, period: Period) -> Vec<String> {
    let mut lines = vec![
        format!(
            "Loot against the cap, {} to {}:",
            date_label(period.since),
            date_label(period.until - 1.0)
        ),
        format!(
            "  {} corpses, {} boxes, {} skins; silver: {} loose, {} from boxes, {} bounty",
            c.searches,
            c.boxes,
            c.skins,
            silver(c.silvers_loose),
            silver(c.silvers_boxes),
            silver(c.bounty_silver)
        ),
    ];
    let mut estimated = 0;
    let mut realised = 0;
    for (kind, count, est, real) in &c.items {
        estimated += est;
        realised += real;
        lines.push(format!(
            "  {kind:<14} {count:>4}  est {:>12}  sold {:>12}",
            silver(*est),
            silver(*real)
        ));
    }
    if !c.items.is_empty() {
        lines.push(format!(
            "  items: est {} in all, {} realised",
            silver(estimated),
            silver(realised)
        ));
    }
    if c.too_valuable > 0 {
        lines.push(format!(
            "  {} items the gem shop called too valuable, unsold",
            c.too_valuable
        ));
    }
    lines
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parsed(line: &str) -> Command {
        parse(line).expect("a loot line").expect("understood")
    }

    #[test]
    fn the_words_parse() {
        assert_eq!(parsed("loot"), Command::Summary(Span::Hours(24)));
        assert_eq!(parsed("loot summary today"), Command::Summary(Span::Today));
        assert_eq!(parsed("loot summary 6"), Command::Summary(Span::Hours(6)));
        assert_eq!(
            parsed("loot recent 5 gems"),
            Command::Recent {
                limit: 5,
                kind: Some("gem".to_owned())
            }
        );
        assert_eq!(
            parsed("loot recent boxes"),
            Command::Recent {
                limit: 20,
                kind: Some("box".to_owned())
            }
        );
        assert_eq!(parsed("loot boxes 3"), Command::Boxes(3));
        assert_eq!(
            parsed("loot creatures 5 today"),
            Command::Creatures {
                limit: 5,
                span: Span::Today
            }
        );
        assert_eq!(
            parsed("loot creatures month"),
            Command::Creatures {
                limit: 10,
                span: Span::ThisMonth
            }
        );
        assert_eq!(parsed("loot cap"), Command::Cap(Month::This));
        assert_eq!(parsed("loot cap last"), Command::Cap(Month::Last));
        assert_eq!(
            parsed("loot cap 2025-12"),
            Command::Cap(Month::Named(2025, 12))
        );
    }

    #[test]
    fn other_words_are_not_loots_and_bad_ones_are_told() {
        assert_eq!(parse("hunt list"), None);
        assert!(parse("loot nosuch").is_some_and(|r| r.is_err()));
        assert!(parse("loot boxes zero").is_some_and(|r| r.is_err()));
        assert!(parse("loot cap 2025-13").is_some_and(|r| r.is_err()));
    }

    #[test]
    fn silver_is_grouped() {
        assert_eq!(silver(0), "0");
        assert_eq!(silver(999), "999");
        assert_eq!(silver(1_000), "1,000");
        assert_eq!(silver(1_234_567), "1,234,567");
    }

    #[test]
    fn a_missing_database_is_said_not_panicked() {
        let missing = std::env::temp_dir().join("cena-no-such-ledger.db");
        let err = run(&missing, Command::Boxes(1), 0).expect_err("no file");
        assert!(err.contains("nothing recorded yet"), "{err}");
    }
}
