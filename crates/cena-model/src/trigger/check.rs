//! A rule as the file writes it, checked into a [`Rule`]: what is refused,
//! and why (the `trigger` module docs).

use serde::Deserialize;

use super::{Flag, LineEvent, Look, Pattern, REARM, Redirect, Rule, Span, regex};
use crate::guard::{Condition, Fact, Guard, Measure};

/// A span as written: a word, or a group's number.
#[derive(Deserialize)]
#[serde(untagged)]
pub(super) enum RawSpan {
    Word(String),
    Group(i64),
}

impl TryFrom<RawSpan> for Span {
    type Error = String;

    fn try_from(raw: RawSpan) -> Result<Self, String> {
        match raw {
            RawSpan::Word(word) => match word.as_str() {
                "match" => Ok(Self::Match),
                "line" => Ok(Self::Line),
                _ => Err(format!(
                    "span `{word}` is not \"match\", \"line\" or a group's number"
                )),
            },
            RawSpan::Group(group) => usize::try_from(group)
                .ok()
                .filter(|&group| group > 0)
                .map(Self::Group)
                .ok_or_else(|| format!("span {group}: groups count from 1")),
        }
    }
}

/// A rule as the file writes it, before it is checked.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Raw {
    #[serde(default)]
    category: String,
    #[serde(default)]
    priority: i32,
    text: Option<String>,
    regex: Option<String>,
    event: Option<String>,
    condition: Option<String>,
    rearm: Option<u32>,
    only_if: Option<String>,
    case_sensitive: Option<bool>,
    whole_word: Option<bool>,
    stream: Option<String>,
    look: Option<Look>,
    #[serde(default)]
    squelch: bool,
    substitute: Option<String>,
    redirect: Option<Redirect>,
    flag: Option<Flag>,
}

impl TryFrom<Raw> for Rule {
    type Error = String;

    fn try_from(raw: Raw) -> Result<Self, String> {
        let only_if = guards("only_if", raw.only_if.as_deref())?;
        let condition = guards("condition", raw.condition.as_deref())?;
        if let Some(flag) = &raw.flag {
            checked_flag(flag)?;
        }
        let line_responds =
            raw.look.is_some() || raw.squelch || raw.substitute.is_some() || raw.redirect.is_some();
        if !condition.is_empty() {
            let reads_a_line = raw.text.is_some()
                || raw.regex.is_some()
                || raw.event.is_some()
                || raw.stream.is_some()
                || raw.case_sensitive.is_some()
                || raw.whole_word.is_some();
            if reads_a_line {
                return Err(
                    "a condition watches the character, not a line: it takes no \
                            `text`, `regex`, `event`, `stream`, `case_sensitive` or `whole_word`"
                        .into(),
                );
            }
            if line_responds {
                return Err("a condition has no line to colour, hide, change or move; \
                            it can set a `flag`"
                    .into());
            }
        } else if raw.rearm.is_some() {
            return Err("`rearm` is for a `condition`".into());
        }
        let case_sensitive = raw.case_sensitive.unwrap_or(false);
        let pattern = pattern(&raw, case_sensitive)?;
        let event = raw.event.as_deref().map(LineEvent::parse).transpose()?;
        if condition.is_empty() && pattern.is_none() && event.is_none() {
            return Err(
                "it has no `text`, `regex`, `event` or `condition`: nothing to fire on".into(),
            );
        }
        if pattern.is_none() && raw.case_sensitive.is_some() {
            return Err("`case_sensitive` is for `text` or `regex`".into());
        }
        if let Some(look) = &raw.look {
            let grouped = matches!(look.span, Span::Group(_));
            if grouped && !matches!(pattern, Some(Pattern::Regex(_))) {
                return Err("a group's span needs a `regex`; nothing else has groups".into());
            }
            if look.color.is_none() && look.background.is_none() && !look.bold {
                return Err("its look sets no colour, background or bold".into());
            }
        }
        if raw.redirect.as_ref().is_some_and(|r| r.stream.is_empty()) {
            return Err("its redirect names no stream".into());
        }
        if !line_responds && raw.flag.is_none() {
            return Err("it does nothing: no look, squelch, substitute, redirect or flag".into());
        }
        Ok(Self {
            category: raw.category,
            priority: raw.priority,
            pattern,
            event,
            condition,
            rearm: raw.rearm.unwrap_or(REARM),
            only_if,
            case_sensitive,
            stream: raw.stream.map(main_is_empty),
            look: raw.look,
            squelch: raw.squelch,
            substitute: raw.substitute,
            redirect: raw.redirect.map(|r| Redirect {
                stream: main_is_empty(r.stream),
                copy: r.copy,
            }),
            flag: raw.flag,
        })
    }
}

/// The pattern `text` or `regex` gives, if either does.
fn pattern(raw: &Raw, case_sensitive: bool) -> Result<Option<Pattern>, String> {
    match (&raw.text, &raw.regex) {
        (Some(_), Some(_)) => {
            Err("it has both `text` and `regex`; a trigger matches one way".into())
        }
        (Some(text), None) if text.is_empty() => Err("its `text` is empty".into()),
        (Some(text), None) => Ok(Some(Pattern::Literal {
            text: text.clone(),
            whole_word: raw.whole_word.unwrap_or(true),
        })),
        (None, _) if raw.whole_word.is_some() => {
            Err("`whole_word` is for `text`; a regex says `\\b` itself".into())
        }
        (None, None) => Ok(None),
        (None, Some(source)) => {
            let built = regex(source, case_sensitive)
                .map_err(|e| format!("its regex cannot be used: {e}"))?;
            if let Some(Look {
                span: Span::Group(group),
                ..
            }) = raw.look
                && group >= built.captures_len()
            {
                return Err(format!("span {group}: the regex has no group {group}"));
            }
            Ok(Some(Pattern::Regex(source.clone())))
        }
    }
}

/// A group of guard words, as `key` writes it; none when it is absent.
///
/// # Errors
///
/// A word the guards do not know, written as nothing, or one only a hunt can
/// read.
fn guards(key: &str, written: Option<&str>) -> Result<Vec<Condition>, String> {
    let Some(written) = written else {
        return Ok(Vec::new());
    };
    let group = Condition::parse_group(written).map_err(|e| format!("its `{key}`: {e}"))?;
    if group.is_empty() {
        return Err(format!("its `{key}` names no guard word"));
    }
    for condition in &group {
        let why = match condition.guard {
            Guard::Is(Fact::Once | Fact::OnceHere) | Guard::Amount(Measure::Every, _) => {
                "reads what a hunt's routine sent, and a trigger sends nothing"
            }
            Guard::Is(Fact::Splashy | Fact::NoMagic) => {
                "reads the map, which a trigger has not got"
            }
            _ => continue,
        };
        return Err(format!("its `{key}`: `{condition}` {why}"));
    }
    Ok(group)
}

/// A flag that can be set or cleared.
fn checked_flag(flag: &Flag) -> Result<(), String> {
    if flag.name.trim().is_empty() {
        return Err("its flag has no name".into());
    }
    if flag.clear && flag.seconds.is_some() {
        return Err("its flag is cleared, so it has no `seconds`".into());
    }
    if flag.seconds == Some(0) {
        return Err("its flag is set for 0 seconds, which sets nothing".into());
    }
    Ok(())
}

/// The model names the main stream `""`, and a player says `main`.
fn main_is_empty(stream: String) -> String {
    if stream == "main" {
        String::new()
    } else {
        stream
    }
}
