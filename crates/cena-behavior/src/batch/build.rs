//! One item's lines, from `;foreach`'s commands: what foreach.lic does to
//! each command as it comes to it (`reference/scripts/scripts/foreach.lic:1936-2126`).
//!
//! Pure, and done for every item before anything is sent, so a command that
//! cannot be made -- a `waitre` whose pattern does not compile, `container`
//! for an item on the ground -- stops the run before its first line rather
//! than halfway through the items.
//!
//! # The words filled in (`:1936-1941`)
//!
//! `item` becomes `#<id>`, `noun` the item's noun, `name` its name and
//! `container` `#<the container's id>`, each as a whole word and in any case,
//! in that order, as Lich fills them.
//!
//! # The conveniences, in Lich's order
//!
//! A Hydra command (`;sc 704 item`) is run and waited for. `move [what] to
//! <where>` is one `_drag` when both ends are ids, or `get` then `put ... in`
//! (`place` for the ground); `return` puts it back where it was found;
//! `unmark X` is `mark X remove`. The waits and `sleep`, `echo` are
//! [`Line`]s of their own. Anything else goes to the game as it stands.
//!
//! Three of Lich's are simpler here: `move` does not learn the container's
//! id from the first `put` to `_drag` the rest (`:2002-2005`), which saved a
//! command per item; it sends its `get` even for an item already in hand,
//! where Lich looked (`:1976`), and the game says so; and `fastmove` is
//! `move`, since every line here waits for its prompt anyway.

use std::time::Duration;

use regex::{NoExpand, Regex};

use super::line::{Line, Pool};
use super::pick::{Candidate, Place};

/// The lines for one item, or why the commands cannot be made for it.
///
/// # Errors
///
/// A `waitre` that is not a pattern, or `container` for an item that is not
/// in one.
pub fn lines(
    commands: &[String],
    place: &Place,
    item: &Candidate,
    symbol: char,
) -> Result<Vec<Line>, String> {
    let mut out = Vec::new();
    for template in commands {
        let command = filled(template, place, item)?;
        out.extend(line_for(&command, place, item, symbol)?);
    }
    Ok(out)
}

/// A command with `item`, `noun`, `name` and `container` filled in.
fn filled(template: &str, place: &Place, item: &Candidate) -> Result<String, String> {
    let word = |w: &str| Regex::new(&format!(r"(?i)\b{w}\b")).map_err(|e| e.to_string());
    let reference = format!("#{}", item.id);
    let mut command = word("item")?
        .replace_all(template, NoExpand(&reference))
        .into_owned();
    command = word("noun")?
        .replace_all(&command, NoExpand(&item.noun))
        .into_owned();
    command = word("name")?
        .replace_all(&command, NoExpand(&item.name))
        .into_owned();
    let container = word("container")?;
    if container.is_match(&command) {
        let Place::Container { id, .. } = place else {
            return Err(format!(
                "`{template}` names the container, and items on the ground are in none."
            ));
        };
        command = container
            .replace_all(&command, NoExpand(&format!("#{id}")))
            .into_owned();
    }
    Ok(command)
}

/// What one filled-in command does (`:1963-2126`).
fn line_for(
    command: &str,
    place: &Place,
    item: &Candidate,
    symbol: char,
) -> Result<Vec<Line>, String> {
    let reference = format!("#{}", item.id);
    if let Some(hydra) = command.strip_prefix(symbol) {
        return Ok(vec![Line::Hydra(hydra.trim().to_owned())]);
    }
    if let Some(moved) = moving(command, &reference) {
        return Ok(moved);
    }
    let lower = command.to_ascii_lowercase();
    let (verb, rest) = lower
        .split_once(char::is_whitespace)
        .map_or((lower.as_str(), ""), |(v, r)| (v, r.trim()));
    let original_rest = command
        .split_once(char::is_whitespace)
        .map_or("", |(_, r)| r.trim());
    let pool = match verb {
        "waitmana" | "waitmp" => Some(Pool::Mana),
        "waithealth" | "waithp" => Some(Pool::Health),
        "waitspirit" | "waitsp" => Some(Pool::Spirit),
        "waitstamina" | "waitst" => Some(Pool::Stamina),
        _ => None,
    };
    if let Some(pool) = pool
        && let Ok(amount) = rest.parse::<i32>()
    {
        return Ok(vec![Line::WaitVital(pool, amount)]);
    }
    // `/^sleep (\d*\.?\d+)$/`: not a number, and it is the game's.
    if verb == "sleep"
        && let Some(time) = seconds(rest)
    {
        return Ok(vec![Line::Sleep(time)]);
    }
    let line = match verb {
        "echo" if !rest.is_empty() => Line::Echo(original_rest.to_owned()),
        "waitrt" | "waitrt?" if rest.is_empty() => Line::WaitRt,
        "waitcastrt" | "waitcastrt?" if rest.is_empty() => Line::WaitCastRt,
        "waitfor" if !rest.is_empty() => Line::WaitFor(original_rest.to_owned()),
        "waitre" if !rest.is_empty() => Line::WaitRe(waitre(original_rest)?),
        "return" if rest.is_empty() || rest == "item" || rest == reference => {
            Line::Send(match place {
                Place::Container { id, .. } => format!("put {reference} in #{id}"),
                Place::Ground => format!("place {reference}"),
            })
        }
        "unmark" if !rest.is_empty() => Line::Send(format!("mark {original_rest} remove")),
        _ => Line::Send(command.to_owned()),
    };
    Ok(vec![line])
}

/// `[f[ast]]move|mv [what] to <where>` (`COMMAND_PATTERNS[:move]`, `:875`),
/// as lines; `None` when the command is not one.
fn moving(command: &str, reference: &str) -> Option<Vec<Line>> {
    let shape = Regex::new(r"(?i)^(?:f(?:ast)?)?(?:move|mv)\s+(?:(.+)\s+)?to\s+(.*)$").ok()?;
    let found = shape.captures(command)?;
    let what = found.get(1).map_or(reference, |m| m.as_str().trim());
    let place = found.get(2).map_or("", |m| m.as_str().trim());
    let ground = place.eq_ignore_ascii_case("ground") || place.eq_ignore_ascii_case("floor");
    let by_id = what.starts_with('#');
    Some(if ground && by_id {
        vec![Line::Send(format!("_drag {what} drop"))]
    } else if ground {
        vec![
            Line::Send(format!("get {what}")),
            Line::Send(format!("place {what}")),
        ]
    } else if by_id && place.starts_with('#') {
        vec![Line::Send(format!("_drag {what} {place}"))]
    } else {
        vec![
            Line::Send(format!("get {what}")),
            Line::Send(format!("put {what} in {place}")),
        ]
    })
}

/// `waitre`'s pattern: `/pattern/flags`, or the text itself (`:2078-2087`).
/// Only an `i` flag means anything here, as it is the only one the two
/// dialects share a letter for.
fn waitre(text: &str) -> Result<String, String> {
    let (body, flags) = text
        .strip_prefix('/')
        .and_then(|inner| inner.rsplit_once('/'))
        .unwrap_or((text, ""));
    let source = if flags.contains('i') {
        format!("(?i){body}")
    } else {
        body.to_owned()
    };
    Regex::new(&source).map_err(|e| format!("waitre {text} is not a pattern: {e}"))?;
    Ok(source)
}

/// `sleep`'s seconds, `0.5` or `2`: digits with at most one point
/// (`:2116`).
fn seconds(text: &str) -> Option<Duration> {
    let digits = text.chars().all(|c| c.is_ascii_digit() || c == '.')
        && text.chars().filter(|c| *c == '.').count() <= 1
        && !text.ends_with('.');
    if !digits {
        return None;
    }
    text.parse::<f64>()
        .ok()
        .and_then(|s| Duration::try_from_secs_f64(s).ok())
}
