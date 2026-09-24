//! The flat YAML bigshot writes, read without a YAML library.
//!
//! A bigshot profile is Ruby's `YAML.dump` of one hash: under each key a
//! string, a boolean, a number or `[]`, and nothing nested. MEASURED over
//! the 32 profiles on the author's machine (the live Lich install's
//! `data/<game>/<character>/bigshot_profiles/*.yaml`, 2026-09-24): every
//! top-level line is `key: value`, `---`, or a continuation indented under
//! the line before it; the values are 1,487 single-quoted, 61 double-quoted,
//! 1,018 `false`, 137 `true`, 56 `[]`, 26 literal blocks (`|-`) and a
//! handful of bare numbers.
//!
//! That is the whole dialect, and reading it is sixty lines. A YAML crate
//! would read it too, and every other YAML document, which no profile is;
//! the simplest thing that works is this (`plan/05` §-1).
//!
//! # What a value becomes
//!
//! Text, always. Quotes come off, `''` inside single quotes is one `'`, the
//! usual escapes inside double quotes are resolved, a scalar folded over
//! several lines is joined with single spaces, a literal block keeps its
//! newlines, `true` and `false` stay those words, and `[]` is the empty
//! string. The importer decides what each key's text means.

use std::collections::BTreeMap;

/// Every key with its value as text. See the module docs for what a value
/// becomes.
///
/// # Errors
///
/// A line that is not `key: value`, `---`, a comment, blank, or indented
/// under a key. The message names the line.
pub fn read(text: &str) -> Result<BTreeMap<String, String>, String> {
    let lines: Vec<&str> = text.lines().collect();
    let mut out = BTreeMap::new();
    let mut i = 0;
    while let Some(line) = lines.get(i) {
        i += 1;
        let trimmed = line.trim_end();
        if trimmed.is_empty() || trimmed == "---" || trimmed.starts_with('#') {
            continue;
        }
        if line.starts_with([' ', '\t']) {
            return Err(format!("line {i}: indented text with no key above it"));
        }
        let (key, rest) = split_key(trimmed)
            .ok_or_else(|| format!("line {i}: expected `key: value`, found {trimmed:?}"))?;
        let value = if is_block(rest) {
            let (text, next) = block(&lines, i);
            i = next;
            text
        } else {
            let (text, next) = folded(rest, &lines, i);
            i = next;
            unquote(&text)
        };
        out.insert(key.to_owned(), value);
    }
    Ok(out)
}

/// `key: value` split at the first colon. The key has no spaces and the
/// colon is followed by a space or the end of the line.
fn split_key(line: &str) -> Option<(&str, &str)> {
    let (key, rest) = line.split_once(':')?;
    let key = key.trim();
    let well_formed = !key.is_empty()
        && !key.contains(char::is_whitespace)
        && (rest.is_empty() || rest.starts_with(' '));
    well_formed.then(|| (key, rest.trim()))
}

/// A literal or folded block indicator.
fn is_block(rest: &str) -> bool {
    matches!(rest, "|" | "|-" | "|+" | ">" | ">-" | ">+")
}

/// The indented lines after a block indicator, their common indentation
/// removed, joined with newlines. Returns the text and the next line to read.
fn block<'a>(lines: &[&'a str], mut i: usize) -> (String, usize) {
    let mut taken: Vec<&'a str> = Vec::new();
    while let Some(line) = lines.get(i) {
        if line.trim().is_empty() || line.starts_with([' ', '\t']) {
            taken.push(line);
            i += 1;
        } else {
            break;
        }
    }
    while taken.last().is_some_and(|l| l.trim().is_empty()) {
        taken.pop();
    }
    let indent = taken
        .iter()
        .filter(|l| !l.trim().is_empty())
        .map(|l| l.len() - l.trim_start().len())
        .min()
        .unwrap_or(0);
    let text = taken
        .iter()
        .map(|l| l.get(indent..).unwrap_or(""))
        .collect::<Vec<_>>()
        .join("\n");
    (text, i)
}

/// A scalar and the indented lines that continue it, joined with single
/// spaces. Returns the text and the next line to read.
fn folded(first: &str, lines: &[&str], mut i: usize) -> (String, usize) {
    let mut text = first.to_owned();
    while let Some(line) = lines.get(i) {
        if line.starts_with([' ', '\t']) && !line.trim().is_empty() {
            if !text.is_empty() {
                text.push(' ');
            }
            text.push_str(line.trim());
            i += 1;
        } else {
            break;
        }
    }
    (text, i)
}

/// Quotes off, escapes resolved, `[]` to nothing.
fn unquote(text: &str) -> String {
    if text == "[]" {
        return String::new();
    }
    if let Some(inner) = text.strip_prefix('\'').and_then(|t| t.strip_suffix('\'')) {
        return inner.replace("''", "'");
    }
    if let Some(inner) = text.strip_prefix('"').and_then(|t| t.strip_suffix('"')) {
        return unescape(inner);
    }
    text.to_owned()
}

/// Inside double quotes: `\n`, `\t`, and `\` before anything else is that
/// character itself.
fn unescape(inner: &str) -> String {
    let mut out = String::with_capacity(inner.len());
    let mut chars = inner.chars();
    while let Some(c) = chars.next() {
        if c != '\\' {
            out.push(c);
            continue;
        }
        match chars.next() {
            Some('n') => out.push('\n'),
            Some('t') => out.push('\t'),
            Some(other) => out.push(other),
            None => out.push('\\'),
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::read;

    #[test]
    fn the_shapes_bigshot_writes() {
        let text = "---\nresting_room_id: '29877'\nfried: '101'\nwander_wait: 0.3\n\
                    delay_loot: true\nnotes: ''\nboons_all: []\n\
                    monitor_safe_strings: \"a\\\"b\n  c\"\n\
                    wounded_eval: bleeding? || Char.percent_health <= 60\n  || !Injured.able_to_cast?\n";
        let read = read(text).unwrap();
        assert_eq!(read["resting_room_id"], "29877");
        assert_eq!(read["fried"], "101");
        assert_eq!(read["wander_wait"], "0.3");
        assert_eq!(read["delay_loot"], "true");
        assert_eq!(read["notes"], "");
        assert_eq!(read["boons_all"], "");
        assert_eq!(
            read["monitor_safe_strings"], "a\"b c",
            "a folded double-quoted scalar"
        );
        assert_eq!(
            read["wounded_eval"],
            "bleeding? || Char.percent_health <= 60 || !Injured.able_to_cast?",
            "a folded plain scalar"
        );
    }

    #[test]
    fn a_literal_block_keeps_its_lines() {
        let text = "wounded_eval: |-\n  (a <= 50) || b?\n    c\n\nbounty_eval: ''\n";
        let read = read(text).unwrap();
        assert_eq!(read["wounded_eval"], "(a <= 50) || b?\n  c");
        assert_eq!(read["bounty_eval"], "");
    }

    #[test]
    fn single_quotes_double_to_escape() {
        let read = read("x: 'Yertie''s Yowlp'\n").unwrap();
        assert_eq!(read["x"], "Yertie's Yowlp");
    }

    #[test]
    fn a_colon_inside_the_value_is_the_values() {
        let read = read("monitor_strings: 'a:b||c: d'\n").unwrap();
        assert_eq!(read["monitor_strings"], "a:b||c: d");
    }

    #[test]
    fn what_is_not_the_dialect_is_refused_by_line() {
        assert_eq!(
            read("a: 1\n  stray\nb: 2\n").unwrap()["a"],
            "1 stray",
            "an indented line under a key continues it"
        );
        let err = read("a: 1\nnot a key line\n").unwrap_err();
        assert!(err.starts_with("line 2:"), "{err}");
        let err = read("  indented first\n").unwrap_err();
        assert!(err.starts_with("line 1:"), "{err}");
    }
}
