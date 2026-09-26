//! A trigger: **when** a finished line matches, **do** something with it
//! (`plan/45`, M8). Highlighting is one response among several.
//!
//! This is a trigger as the matcher takes it: named, typed and checked. The
//! file it comes from -- one `triggers.toml`, with categories, master
//! switches and per-character overrides -- is `cena_behavior::triggers`,
//! which hands each character its own list of these.
//!
//! Stage 1 has one source, a finished line's text, and two groups of
//! response (`plan/45` §3b):
//!
//! | Group | Responses | In the file |
//! |---|---|---|
//! | **Look** | colour, background, bold; over the match, a capture group, or the line | `look = { color = "#ff4040", bold = true, span = "line" }` |
//! | **Text** | squelch; substitute (with `$1`); redirect to another stream | `squelch = true`, `substitute = "…"`, `redirect = { stream = "combat", copy = true }` |
//!
//! None of them changes what the game said. The model's scrollback, the
//! chunk the classifiers read and the player log keep the game's text; a
//! response is a display fact carried with the published line (`plan/45` §4).
//!
//! **Checked as it is read.** A rule that cannot work is refused with the
//! reason, rather than kept and silently never firing: two ways to match or
//! none, a regex the `regex` crate cannot build (lookaround and
//! backreferences included, `plan/45` §8 item 6), a colour that is not
//! `#rrggbb`, a capture group the regex does not have, or no response at all.
//!
//! **Case**: a pattern ignores case unless `case_sensitive = true`. Wrayth's
//! highlights ignore case unless marked `case="y"` (the author's reading,
//! `plan/45` §1 row 4), and every Wrayth string is a trigger here once
//! imported. Wizard FE, Wrayth's predecessor, documents the same default: its
//! "Case Sensitive" box, *"turned on, tells the Wizard to only match with this
//! string if the incoming text matches exactly"*
//! (`reference/wiki_clean/Wizard _front end_.txt:575`). `VellumFE` defaults
//! the other way (`case_insensitive`,
//! `reference/VellumFE/src/config/highlights.rs:176`); the field is named for
//! what it turns on, so neither default can be misread.
//!
//! **Words**: a literal matches only as whole words unless
//! `whole_word = false`: `John` does not hit `Johnny`. Wizard FE, Wrayth's
//! predecessor, has the same default, which its "Not on Word Boundary" box
//! turns off (`:563-573`), and `VellumFE` checks a boundary for its literals
//! too (`src/core/highlight_engine.rs:423-437`).
//!
//! **The boundary is only needed where the literal's own edge is a letter,
//! a digit or `_`**, which is where a regex's `\b` would sit. Wizard FE and
//! `VellumFE` require one on both sides whatever the edge, and that is a
//! pitfall both have recorded: Wizard FE's own example is `SEND[`, which
//! misses `SEND[BigWizard]` until the box is ticked, and a Saga player's
//! imported `[DemsDen] ` -- the trailing space deliberate, to match
//! `[DemsDen] ...` and not `[DemsDen]No Space` -- stopped matching when
//! Saga's import turned whole words on
//! (`reference/discord/saga-thread.txt:21114-21116`, `:21198`). Here both
//! match as their writers meant, and `whole_word = false` is for matching
//! inside a word. CLAUDE'S CALL, to confirm (`plan/45` §5d); Wrayth's own
//! rule is UNVERIFIED. A regex says `\b` itself.

use serde::Deserialize;
use std::fmt;

mod matcher;

pub use matcher::{Hit, Matcher};

/// A trigger, by its name in the file, and what it does.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Trigger {
    /// Its name: unique in the file, and what `;trigger` addresses.
    pub name: String,
    /// The rest of it.
    pub rule: Rule,
}

/// When a trigger fires and what it does, checked.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(try_from = "Raw")]
pub struct Rule {
    /// Its group in the file, set by the editor; the file is written sorted
    /// by it (author, `plan/45` §1 row 3). Empty when none is given.
    pub category: String,
    /// Higher goes first when two looks overlap; ties go in file order.
    pub priority: i32,
    /// What it matches in a finished line.
    pub pattern: Pattern,
    /// Whether case matters.
    pub case_sensitive: bool,
    /// The one stream it looks at, `""` for the main one; `None` for all.
    pub stream: Option<String>,
    /// How what it matched looks.
    pub look: Option<Look>,
    /// Hide the line.
    pub squelch: bool,
    /// Put this in place of the match; with a regex, `$1` is a group.
    pub substitute: Option<String>,
    /// Show the line on another stream.
    pub redirect: Option<Redirect>,
}

/// What a trigger matches in a line's text.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Pattern {
    /// These words (`text = "…"`).
    Literal {
        /// The words.
        text: String,
        /// Only where no letter, digit or `_` touches them (the module docs).
        whole_word: bool,
    },
    /// A regular expression (`regex = '…'`), in the `regex` crate's syntax.
    Regex(String),
}

/// Colour, background and weight, over part of a line.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Look {
    /// The text's colour.
    pub color: Option<Color>,
    /// Behind the text.
    pub background: Option<Color>,
    /// Bold.
    #[serde(default)]
    pub bold: bool,
    /// Which part of the line.
    #[serde(default)]
    pub span: Span,
}

/// The part of a line a look covers.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Deserialize)]
#[serde(try_from = "RawSpan")]
pub enum Span {
    /// What the pattern matched (`span = "match"`, the default).
    #[default]
    Match,
    /// The whole line (`span = "line"`), as Wrayth's `line="y"`.
    Line,
    /// One capture group of a regex, counting from 1 (`span = 2`).
    Group(usize),
}

/// A colour, written `#rrggbb`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(try_from = "String")]
pub struct Color {
    /// Red.
    pub red: u8,
    /// Green.
    pub green: u8,
    /// Blue.
    pub blue: u8,
}

/// Where a redirected line goes.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Redirect {
    /// The stream it is shown on.
    pub stream: String,
    /// Shown on both; otherwise moved.
    #[serde(default)]
    pub copy: bool,
}

/// The regex `source` builds, ignoring case unless `case_sensitive`: the one
/// way a trigger's regex is built, for the check and the matcher alike.
///
/// # Errors
///
/// The `regex` crate cannot build it.
pub fn regex(source: &str, case_sensitive: bool) -> Result<regex::Regex, regex::Error> {
    regex::RegexBuilder::new(source)
        .case_insensitive(!case_sensitive)
        .build()
}

impl fmt::Display for Color {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "#{:02x}{:02x}{:02x}", self.red, self.green, self.blue)
    }
}

impl TryFrom<String> for Color {
    type Error = String;

    fn try_from(text: String) -> Result<Self, String> {
        let wrong = || format!("`{text}` is not a colour: write it #rrggbb");
        let hex = text.strip_prefix('#').ok_or_else(wrong)?;
        if hex.len() != 6 || !hex.bytes().all(|b| b.is_ascii_hexdigit()) {
            return Err(wrong());
        }
        let byte = |at: usize| u8::from_str_radix(&hex[at..at + 2], 16).map_err(|_| wrong());
        Ok(Self {
            red: byte(0)?,
            green: byte(2)?,
            blue: byte(4)?,
        })
    }
}

/// A span as written: a word, or a group's number.
#[derive(Deserialize)]
#[serde(untagged)]
enum RawSpan {
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
struct Raw {
    #[serde(default)]
    category: String,
    #[serde(default)]
    priority: i32,
    text: Option<String>,
    regex: Option<String>,
    #[serde(default)]
    case_sensitive: bool,
    whole_word: Option<bool>,
    stream: Option<String>,
    look: Option<Look>,
    #[serde(default)]
    squelch: bool,
    substitute: Option<String>,
    redirect: Option<Redirect>,
}

impl TryFrom<Raw> for Rule {
    type Error = String;

    fn try_from(raw: Raw) -> Result<Self, String> {
        let pattern = match (raw.text, raw.regex) {
            (Some(_), Some(_)) => {
                return Err("it has both `text` and `regex`; a trigger matches one way".into());
            }
            (None, None) => return Err("it has no `text` or `regex`: nothing to match".into()),
            (Some(text), None) if text.is_empty() => return Err("its `text` is empty".into()),
            (Some(text), None) => Pattern::Literal {
                text,
                whole_word: raw.whole_word.unwrap_or(true),
            },
            (None, Some(_)) if raw.whole_word.is_some() => {
                return Err("`whole_word` is for `text`; a regex says `\\b` itself".into());
            }
            (None, Some(source)) => {
                let built = regex(&source, raw.case_sensitive)
                    .map_err(|e| format!("its regex cannot be used: {e}"))?;
                if let Some(Look {
                    span: Span::Group(group),
                    ..
                }) = raw.look
                    && group >= built.captures_len()
                {
                    return Err(format!("span {group}: the regex has no group {group}"));
                }
                Pattern::Regex(source)
            }
        };
        if let Some(look) = &raw.look {
            if matches!(pattern, Pattern::Literal { .. }) && matches!(look.span, Span::Group(_)) {
                return Err("a group's span needs a `regex`; `text` has no groups".into());
            }
            if look.color.is_none() && look.background.is_none() && !look.bold {
                return Err("its look sets no colour, background or bold".into());
            }
        }
        if raw.redirect.as_ref().is_some_and(|r| r.stream.is_empty()) {
            return Err("its redirect names no stream".into());
        }
        let responds =
            raw.look.is_some() || raw.squelch || raw.substitute.is_some() || raw.redirect.is_some();
        if !responds {
            return Err("it does nothing: no look, squelch, substitute or redirect".into());
        }
        Ok(Self {
            category: raw.category,
            priority: raw.priority,
            pattern,
            case_sensitive: raw.case_sensitive,
            stream: raw.stream.map(main_is_empty),
            look: raw.look,
            squelch: raw.squelch,
            substitute: raw.substitute,
            redirect: raw.redirect.map(|r| Redirect {
                stream: main_is_empty(r.stream),
                copy: r.copy,
            }),
        })
    }
}

/// The model names the main stream `""`, and a player says `main`.
fn main_is_empty(stream: String) -> String {
    if stream == "main" {
        String::new()
    } else {
        stream
    }
}
