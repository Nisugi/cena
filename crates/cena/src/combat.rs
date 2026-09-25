//! `;combat`, the combat recorder's reports on Hydra's command line: the
//! latest hunt, one hunt, the hunts listed, the aggregate, the abilities
//! rollup and the recent attacks (`plan/34` Stage 4).
//!
//! The join only, as `loot.rs` is for the ledger: the queries are
//! `cena_session::combat_recorder::report`'s, over the same read-only
//! `Reader` and the same file. `combat_stats.lic`'s further reports -- the
//! creature tree, flare, defense and HP analytics -- are not here yet.
//!
//! # BUILT, NOT RUN
//!
//! Compiled and tested, never executed against the game (`hunt.rs`).

use std::path::{Path, PathBuf};
use std::sync::Arc;

use cena_session::combat_recorder::report::{Abilities, AttackRow, HuntReport, HuntRow, Scope};
use cena_session::command::claimant::Claimed;
use cena_session::ledger::report::Reader;
use cena_session::{Notice, NoticeKind, SessionHandle};

use crate::commands::Commands;
use crate::loot::period::time_label;

/// The words `;combat` knows.
const USAGE: &str = "combat [<id>] | combat hunts [<n>] | combat all | combat last <n> | \
                     combat abilities [<id>|all|last <n>] | combat attacks [<n>]";

/// One `;combat` command, parsed.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum Command {
    /// The hunt report over a scope.
    Hunt(Scope),
    /// The last hunts listed.
    Hunts(usize),
    /// Damage by ability over a scope.
    Abilities(Scope),
    /// The latest hunt's last attacks.
    Attacks(usize),
}

/// Parse a command line, without its symbol. `None` when the word is not
/// `combat`; `Some(Err)` when it is and the rest is not understood.
pub(crate) fn parse(line: &str) -> Option<Result<Command, String>> {
    let mut words = line.split_whitespace();
    if !words.next()?.eq_ignore_ascii_case("combat") {
        return None;
    }
    let rest: Vec<String> = words.map(str::to_ascii_lowercase).collect();
    let usage = |what: &str| format!("{what}. Usage: {USAGE}");
    let count = |word: Option<&String>, default: usize| -> Result<usize, String> {
        word.map_or(Ok(default), |w| {
            w.parse()
                .ok()
                .filter(|n| *n > 0)
                .ok_or_else(|| usage("a count must be a positive number"))
        })
    };
    let scope = |words: &[String]| -> Result<Scope, String> {
        match words.first().map(String::as_str) {
            None => Ok(Scope::Latest),
            Some("all") => Ok(Scope::All),
            Some("last") => count(words.get(1), 10).map(Scope::Last),
            Some(w) => w
                .parse()
                .map(Scope::Hunt)
                .map_err(|_| usage(&format!("`{w}` is not a hunt id"))),
        }
    };
    let command = match rest.first().map(String::as_str) {
        None | Some("all" | "last") => scope(&rest).map(Command::Hunt),
        Some("hunts" | "sessions") => count(rest.get(1), 10).map(Command::Hunts),
        Some("abilities" | "ability") => scope(&rest[1..]).map(Command::Abilities),
        Some("attacks" | "attack") => count(rest.get(1), 15).map(Command::Attacks),
        Some(w) if w.chars().all(|c| c.is_ascii_digit()) => scope(&rest).map(Command::Hunt),
        Some(other) => Err(usage(&format!("`{other}` is not a report"))),
    };
    Some(command)
}

/// Register `;combat` on the command line, reading `database`.
pub(crate) fn open(handle: &SessionHandle, commands: &Commands, database: PathBuf) {
    let handler = handle.clone();
    commands.combat(Arc::new(move |line: &str| {
        let command = match parse(line)? {
            Ok(command) => command,
            Err(why) => {
                handler.say(Notice::line(NoticeKind::Error, format!("Combat: {why}")));
                return Some(Claimed::Done);
            }
        };
        let (handle, database) = (handler.clone(), database.clone());
        tokio::task::spawn_blocking(move || match run(&database, &command) {
            Ok(lines) => handle.say(Notice::table(NoticeKind::Info, lines)),
            Err(why) => handle.say(Notice::line(NoticeKind::Error, format!("Combat: {why}"))),
        });
        Some(Claimed::Done)
    }));
    eprintln!("[combat] ready: {USAGE}");
}

/// Run one command against the database and give the lines to say.
fn run(database: &Path, command: &Command) -> Result<Vec<String>, String> {
    if !database.is_file() {
        return Err(format!(
            "nothing recorded yet ({}). Recording is on with --record.",
            database.display()
        ));
    }
    let reader = Reader::open(database).map_err(|e| e.to_string())?;
    let none = || "no hunts recorded yet.".to_owned();
    let lines = match command {
        Command::Hunt(scope) => match reader.hunt(scope).map_err(|e| e.to_string())? {
            Some(report) => hunt(&report, scope),
            None => return Err(missing(scope, none)),
        },
        Command::Hunts(n) => hunts(&reader.hunts(*n).map_err(|e| e.to_string())?),
        Command::Abilities(scope) => match reader.abilities(scope).map_err(|e| e.to_string())? {
            Some(report) => abilities(&report, scope),
            None => return Err(missing(scope, none)),
        },
        Command::Attacks(n) => attacks(&reader.recent_attacks(*n).map_err(|e| e.to_string())?),
    };
    Ok(lines)
}

fn missing(scope: &Scope, none: impl Fn() -> String) -> String {
    match scope {
        Scope::Hunt(id) => format!("no hunt #{id}."),
        _ => none(),
    }
}

/// `4m05s`, or `open`.
fn duration(seconds: Option<f64>) -> String {
    let Some(seconds) = seconds else {
        return "open".to_owned();
    };
    let whole = seconds.round().max(0.0);
    // Whole seconds of a hunt: far inside i64.
    #[expect(
        clippy::cast_possible_truncation,
        reason = "rounded, non-negative seconds"
    )]
    let s = whole as i64;
    if s >= 3600 {
        format!("{}h{:02}m", s / 3600, (s % 3600) / 60)
    } else if s >= 60 {
        format!("{}m{:02}s", s / 60, s % 60)
    } else {
        format!("{s}s")
    }
}

fn scope_label(scope: &Scope, report: &HuntReport) -> String {
    match scope {
        Scope::Latest | Scope::Hunt(_) => report
            .hunts
            .first()
            .map_or_else(|| "hunt".to_owned(), |h| format!("hunt #{}", h.id)),
        Scope::All => format!("all {} hunts", report.hunts.len()),
        Scope::Last(_) => format!("the last {} hunts", report.hunts.len()),
    }
}

fn per_hour(n: i64, hunt_time: f64) -> String {
    if hunt_time > 0.0 {
        // A count over hours: the count is far inside f64's exact range.
        #[expect(clippy::cast_precision_loss, reason = "a count of events")]
        let rate = 3600.0 * n as f64 / hunt_time;
        format!("{rate:.1}")
    } else {
        "-".to_owned()
    }
}

/// Damage per hit to one decimal, or `-` for no hits.
fn average(damage: i64, hits: i64) -> String {
    if hits > 0 {
        // Damage totals and hit counts are far inside f64's exact range.
        #[expect(clippy::cast_precision_loss, reason = "game counts")]
        let avg = damage as f64 / hits as f64;
        format!("{avg:.1}")
    } else {
        "-".to_owned()
    }
}

fn hunt(r: &HuntReport, scope: &Scope) -> Vec<String> {
    let mut lines = vec![format!(
        "Combat, {}{}:",
        scope_label(scope, r),
        r.character
            .as_deref()
            .map_or(String::new(), |c| format!(" ({c})"))
    )];
    if let [one] = r.hunts.as_slice() {
        let ended = one
            .ended_at
            .map_or_else(|| "open".to_owned(), |_| duration(Some(r.hunt_time)));
        lines.push(format!(
            "  started {}, last event {}, {ended}",
            time_label(one.started_at),
            time_label(one.last_event.unwrap_or(one.started_at))
        ));
    } else if let (Some(first), Some(last)) = (r.hunts.first(), r.hunts.last()) {
        lines.push(format!(
            "  {} to {}; {} hunting in all",
            time_label(first.started_at),
            time_label(last.last_event.unwrap_or(last.started_at)),
            duration(Some(r.hunt_time))
        ));
    }
    lines.push(format!(
        "  attacks {}  sequences {}  inbound {}  kills {}  assists {}  dealt {}  taken {}",
        r.attacks, r.sequences, r.inbound, r.kills, r.assists, r.dealt, r.taken
    ));
    if r.hunts.len() > 1 {
        lines.push(format!(
            "  per hour: {} kills, {} damage, {} attacks, {} taken",
            per_hour(r.kills, r.hunt_time),
            per_hour(r.dealt, r.hunt_time),
            per_hour(r.attacks, r.hunt_time),
            per_hour(r.taken, r.hunt_time)
        ));
    }
    if !r.creatures.is_empty() {
        lines.push(format!(
            "  {:<24} {:>5} {:>5} {:>6} {:>5} {:>7} {:>8}",
            "creature", "seen", "kills", "others", "alive", "attacks", "damage"
        ));
        for k in &r.creatures {
            lines.push(format!(
                "  {:<24} {:>5} {:>5} {:>6} {:>5} {:>7} {:>8}",
                k.noun, k.seen, k.kills, k.other_dead, k.alive, k.attacks, k.damage
            ));
        }
    }
    if r.hunts.len() > 1 && r.hunts.len() <= 20 {
        lines.push("  per hunt:".to_owned());
        for h in &r.hunts {
            lines.push(hunt_line(h));
        }
    }
    lines
}

fn hunt_line(h: &HuntRow) -> String {
    let length = h.ended_at.map(|e| e - h.started_at);
    format!(
        "  #{:<4} {}  {:>7}  attacks {:>4}  kills {:>3}  damage {:>6}",
        h.id,
        time_label(h.started_at),
        duration(length),
        h.attacks,
        h.kills,
        h.damage
    )
}

fn hunts(rows: &[HuntRow]) -> Vec<String> {
    if rows.is_empty() {
        return vec!["Combat: no hunts recorded yet.".to_owned()];
    }
    let mut lines = vec![format!("Combat, the last {} hunts:", rows.len())];
    lines.extend(rows.iter().map(hunt_line));
    lines
}

fn abilities(a: &Abilities, scope: &Scope) -> Vec<String> {
    let what = match scope {
        Scope::Latest => "the latest hunt".to_owned(),
        Scope::Hunt(id) => format!("hunt #{id}"),
        Scope::All => "all hunts".to_owned(),
        Scope::Last(n) => format!("the last {n} hunts"),
    };
    let mut lines = vec![format!("Combat by ability, {what}:")];
    if a.abilities.is_empty() {
        lines.push("  no attacks of ours recorded.".to_owned());
    } else {
        lines.push(format!(
            "  {:<28} {:>5} {:>6} {:>5} {:>8} {:>6} {:>5} {:>5}",
            "ability", "tries", "landed", "hits", "damage", "avg", "crits", "fatal"
        ));
        for row in &a.abilities {
            let name = row
                .parent
                .as_deref()
                .map_or_else(|| row.name.clone(), |p| format!("{} via {p}", row.name));
            let avg = average(row.damage, row.hits);
            lines.push(format!(
                "  {name:<28} {:>5} {:>6} {:>5} {:>8} {avg:>6} {:>5} {:>5}",
                row.attempts, row.landed, row.hits, row.damage, row.crits, row.fatal
            ));
        }
    }
    if !a.flares.is_empty() {
        lines.push("  flares:".to_owned());
        for f in &a.flares {
            lines.push(format!(
                "  {:<28} {:>5} procs {:>5} hits {:>8} damage {:>3} fatal",
                f.name, f.procs, f.hits, f.damage, f.fatal
            ));
        }
    }
    if !a.inbound.is_empty() {
        lines.push("  against you:".to_owned());
        for i in &a.inbound {
            lines.push(format!(
                "  {:<28} by {:<20} {:>4}x  {:>6} damage  {}",
                i.name,
                i.attacker.as_deref().unwrap_or("?"),
                i.count,
                i.damage,
                i.outcomes
            ));
        }
    }
    lines
}

fn attacks(rows: &[AttackRow]) -> Vec<String> {
    if rows.is_empty() {
        return vec!["Combat: no attacks recorded this hunt.".to_owned()];
    }
    let mut lines = vec![format!(
        "Combat, the last {} attacks, newest first:",
        rows.len()
    )];
    for a in rows {
        lines.push(format!(
            "  #{:<6} {} {:<24}{} {:>5} {:<8} {} flares  {}",
            a.id,
            time_label(a.at),
            a.name,
            if a.echo { " (echo)" } else { "" },
            a.damage,
            a.outcome.as_deref().unwrap_or("hit"),
            a.flares,
            a.target.as_deref().unwrap_or("")
        ));
    }
    lines
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parsed(line: &str) -> Command {
        parse(line).expect("a combat line").expect("understood")
    }

    #[test]
    fn the_words_parse() {
        assert_eq!(parsed("combat"), Command::Hunt(Scope::Latest));
        assert_eq!(parsed("combat 7"), Command::Hunt(Scope::Hunt(7)));
        assert_eq!(parsed("combat all"), Command::Hunt(Scope::All));
        assert_eq!(parsed("combat last 3"), Command::Hunt(Scope::Last(3)));
        assert_eq!(parsed("combat hunts"), Command::Hunts(10));
        assert_eq!(parsed("combat hunts 5"), Command::Hunts(5));
        assert_eq!(
            parsed("combat abilities"),
            Command::Abilities(Scope::Latest)
        );
        assert_eq!(
            parsed("combat abilities all"),
            Command::Abilities(Scope::All)
        );
        assert_eq!(
            parsed("combat abilities last 2"),
            Command::Abilities(Scope::Last(2))
        );
        assert_eq!(
            parsed("combat abilities 4"),
            Command::Abilities(Scope::Hunt(4))
        );
        assert_eq!(parsed("combat attacks"), Command::Attacks(15));
        assert_eq!(parsed("combat attacks 3"), Command::Attacks(3));
    }

    #[test]
    fn other_words_are_not_combats_and_bad_ones_are_told() {
        assert_eq!(parse("loot boxes"), None);
        assert!(parse("combat nosuch").is_some_and(|r| r.is_err()));
        assert!(parse("combat last zero").is_some_and(|r| r.is_err()));
        assert!(parse("combat hunts -1").is_some_and(|r| r.is_err()));
    }

    #[test]
    fn durations_read() {
        assert_eq!(duration(None), "open");
        assert_eq!(duration(Some(42.4)), "42s");
        assert_eq!(duration(Some(245.0)), "4m05s");
        assert_eq!(duration(Some(3_720.0)), "1h02m");
    }

    #[test]
    fn a_missing_database_is_said_not_panicked() {
        let missing = std::env::temp_dir().join("cena-no-such-combat.db");
        let err = run(&missing, &Command::Hunts(1)).expect_err("no file");
        assert!(err.contains("nothing recorded yet"), "{err}");
    }
}
