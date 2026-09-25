//! `;multi`: multi.lic, a comma list repeated
//! (`reference/scripts/scripts/multi.lic`, 65 lines).
//!
//! `;multi 3,get gem,sell gem` sends `get gem` and `sell gem`, three times
//! over. The count may come last instead (`multi.lic:53`), and a list with
//! no comma at all is split on semicolons (`:50`). An entry that begins with
//! the command symbol is a Hydra command, run and waited for, where
//! multi.lic ran a script with `Script.run` (`:57-61`): `;multi 2,get
//! diamond in pouch,;sc 401,put diamond in sack`.
//!
//! # Where it differs, each on purpose
//!
//! - **Entries are trimmed, and an empty one is skipped.** multi.lic sent
//!   ` sell gem` as typed, and a blank line for `,,`; its help says "no
//!   spaces, just a comma" because of it.
//! - **A count neither first nor last is refused.** multi.lic took `.to_i`
//!   of the last entry (`:53`), which is `0` for `sell gem`, and so ran
//!   nothing and said nothing.
//! - **`;multi` inside `;multi` is refused.** Lich would not start a script
//!   already running; here the desk refuses a second batch of its kind, so
//!   the inner one would be refused on every round.
//! - **`;multi stop`** is Hydra's: Lich stopped it with `;kill multi`.

use super::line::Line;

/// What `;multi` was asked to do.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Command {
    /// Send the list this many times.
    Run(Multi),
    /// `;multi stop`: stop the one running.
    Stop,
    /// Bare `;multi`, or `;multi help`: say how.
    Help,
}

/// A list, and how many times it is sent.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Multi {
    /// How many times round.
    pub times: u32,
    /// The list, in order.
    pub lines: Vec<Line>,
}

/// How `;multi` is used, as its help says it (`multi.lic:24-45`).
pub const USAGE: &[&str] = &[
    "multi <times>,<command>,<command>,...   the commands, in order, <times> times over",
    "  multi 6,shake my jar,sell diamond",
    "  multi 10,order 1,buy,stow potion",
    "  multi 2,get diamond in pouch,;sc 401,put diamond in sack   (a Hydra command is run and waited for)",
    "multi stop   stop it",
];

/// Parse a command line, without its symbol. `None` when the word is not
/// `multi`; `Some(Err)` with the reason when it is, and cannot be run.
///
/// `symbol` is the character's command symbol, which marks an entry as a
/// Hydra command.
#[must_use]
pub fn parse(line: &str, symbol: char) -> Option<Result<Command, String>> {
    let line = line.trim();
    let (word, rest) = line.split_once(char::is_whitespace).unwrap_or((line, ""));
    if !word.eq_ignore_ascii_case("multi") {
        return None;
    }
    let rest = rest.trim();
    if rest.is_empty() || rest.eq_ignore_ascii_case("help") {
        return Some(Ok(Command::Help));
    }
    if rest.eq_ignore_ascii_case("stop") {
        return Some(Ok(Command::Stop));
    }
    Some(list(rest, symbol).map(Command::Run))
}

/// The list itself: `3,get gem,sell gem`.
fn list(rest: &str, symbol: char) -> Result<Multi, String> {
    // `:50`: a list with no comma uses semicolons.
    let separator = if rest.contains(',') { ',' } else { ';' };
    let mut entries: Vec<&str> = rest
        .split(separator)
        .map(str::trim)
        .filter(|entry| !entry.is_empty())
        .collect();
    let is_count = |entry: &&str| !entry.is_empty() && entry.bytes().all(|b| b.is_ascii_digit());
    // `:53`: the count first, else last.
    let count = if entries.first().is_some_and(is_count) {
        entries.remove(0)
    } else if entries.last().is_some_and(is_count) {
        entries.pop().unwrap_or_default()
    } else {
        return Err(format!(
            "Multi: say how many times, first or last: `multi 3,{}`.",
            entries.join(",")
        ));
    };
    let times: u32 = count
        .parse()
        .map_err(|_| format!("Multi: {count} is more times than can be counted."))?;
    if times == 0 {
        return Err("Multi: zero times is nothing to do.".to_owned());
    }
    if entries.is_empty() {
        return Err(format!(
            "Multi: {times} times what? `multi {times},get gem,sell gem`."
        ));
    }
    let lines = entries
        .into_iter()
        .map(|entry| entry_line(entry, symbol))
        .collect::<Result<Vec<Line>, String>>()?;
    Ok(Multi { times, lines })
}

/// One entry: a Hydra command when it starts with the symbol, else a line
/// for the game.
fn entry_line(entry: &str, symbol: char) -> Result<Line, String> {
    let Some(command) = entry.strip_prefix(symbol) else {
        return Ok(Line::Send(entry.to_owned()));
    };
    let command = command.trim();
    let word = command.split_whitespace().next().unwrap_or_default();
    if word.is_empty() {
        return Err(format!(
            "Multi: `{entry}` names no command after the {symbol}."
        ));
    }
    if word.eq_ignore_ascii_case("multi") {
        return Err(format!(
            "Multi: `{entry}` would run a multi inside this one, and one runs at a time."
        ));
    }
    Ok(Line::Hydra(command.to_owned()))
}
