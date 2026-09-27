//! A trigger: **when** something happens, **do** something (`plan/45`, M8).
//! Highlighting is one response among several.
//!
//! This is a trigger as the matcher takes it: named, typed and checked. The
//! file it comes from -- one `triggers.toml`, with categories, master
//! switches and per-character overrides -- is `cena_behavior::triggers`,
//! which hands each character its own list of these.
//!
//! # When
//!
//! | Source | In the file | Fires |
//! |---|---|---|
//! | a line's words | `text = "…"` or `regex = '…'` | on each finished line they match |
//! | what the model reads a line as | `event = "speech"` ([`LineEvent`], its words) | on each finished line that is one; beside `text` or `regex`, it narrows them |
//! | a condition | `condition = "!health_at_least 30"`, guard words | when they all become true |
//!
//! A line's trigger may be limited to one stream (`stream = "combat"`).
//! **`only_if`**, `only_if = "hidden"`, may sit on any trigger: guard words
//! that must all hold when it would fire. Not a *gate*, which is the
//! session's check at the moment it writes (the glossary).
//!
//! **The guard words are the hunt's** (author, `plan/45` §1 row 2: *"yes"*),
//! from [`crate::guard`], read against the character as it is when the line
//! or the condition is read. **The target** a word such as `stunned` asks
//! about is the character's current target, as the game's target list names
//! it. Five words are the hunt's alone and are refused here: `once`,
//! `once_here` and `every` read what a hunt's routine sent, and `splashy`
//! and `nomagic` read the map.
//!
//! **A condition is edge-triggered**, on `VellumFE`'s rules (`plan/45` §2a):
//! the first reading is taken silently, it fires only on false to true, and
//! once it has fired it must stay false for `rearm` seconds, 3 unless set,
//! before it fires again. A reading the game has not given is no reading.
//!
//! # Do
//!
//! | Group | Responses | In the file |
//! |---|---|---|
//! | **Look** | colour, background, bold; over the match, a capture group, or the line | `look = { color = "#ff4040", bold = true, span = "line" }` |
//! | **Text** | squelch; substitute (with `$1`); redirect to another stream | `squelch = true`, `substitute = "…"`, `redirect = { stream = "combat", copy = true }` |
//! | **Flag** | set a named flag, for a time or until cleared, or clear it; the guard word `flag "<name>"` reads it | `flag = { name = "rift", seconds = 30 }`, `flag = { name = "rift", clear = true }` |
//! | **Attention** | a sound; an OS notification; a banner on the character's pages ([`Attention`]) | `sound = "data.wav"`, `notify = true`, `alert = "$1 is here"`, `cooldown = 10` |
//!
//! Where an `event` stands without `text` or `regex`, the whole line is the
//! match. A condition has no line: it may set a flag and call for
//! attention, and nothing else. A trigger's attention comes at most once in
//! its `cooldown`, 3 seconds unless set, `VellumFE`'s
//! `DEFAULT_COOLDOWN_SECS` (`reference/VellumFE/src/core/alerts.rs:23`).
//!
//! None of the line's responses changes what the game said. The model's
//! scrollback, the chunk the classifiers read and the player log keep the
//! game's text; a response is a display fact carried with the published
//! line (`plan/45` §4).
//!
//! **Checked as it is read** (`check.rs`). A rule that cannot work is
//! refused with the reason, rather than kept and silently never firing: two
//! ways to match, a regex the `regex` crate cannot build (lookaround and
//! backreferences included, `plan/45` §8 item 6), a colour that is not
//! `#rrggbb`, a capture group the regex does not have, a word a trigger
//! cannot read, a condition with a line's response, or no response at all.
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

use crate::GameState;
use crate::guard::{Condition, Facts};
use crate::state::flags::{FlagChange, Until};

mod attention;
mod check;
mod edges;
mod event;
mod matcher;
mod respond;

pub use attention::{Attention, Cooldowns, Say};
pub use edges::Edges;
pub use event::LineEvent;
pub use matcher::{Hit, Matcher};
pub use respond::{Answer, Paint};

/// Seconds a condition must stay false before it fires again, unless its
/// `rearm` says otherwise: `VellumFE`'s `DEFAULT_REARM_SECS`
/// (`reference/VellumFE/src/core/alerts.rs:37`).
pub const REARM: u32 = 3;

/// Seconds before a trigger's attention comes again, unless its `cooldown`
/// says otherwise: `VellumFE`'s `DEFAULT_COOLDOWN_SECS`.
pub const COOLDOWN: u32 = 3;

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
#[serde(try_from = "check::Raw")]
pub struct Rule {
    /// Its group in the file, set by the editor; the file is written sorted
    /// by it (author, `plan/45` §1 row 3). Empty when none is given.
    pub category: String,
    /// Higher goes first when two looks overlap; ties go in file order.
    pub priority: i32,
    /// What it matches in a finished line; `None` when an `event` alone
    /// says which lines, and for a condition.
    pub pattern: Option<Pattern>,
    /// What the model must read the line as.
    pub event: Option<LineEvent>,
    /// Guard words that fire it when they all become true. Not empty makes
    /// it a condition, which watches the character and not a line.
    pub condition: Vec<Condition>,
    /// Seconds a condition must stay false before it fires again.
    pub rearm: u32,
    /// Guard words that must all hold when it would fire.
    pub only_if: Vec<Condition>,
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
    /// Set or clear a flag.
    pub flag: Option<Flag>,
    /// Play this sound: a file in the sounds folder, or a path.
    pub sound: Option<String>,
    /// Say this as an OS notification.
    pub notify: Option<Say>,
    /// Show this as a banner on the character's pages.
    pub alert: Option<Say>,
    /// Seconds before its attention comes again.
    pub cooldown: u32,
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
#[serde(try_from = "check::RawSpan")]
pub enum Span {
    /// What the pattern matched (`span = "match"`, the default); the whole
    /// line where an `event` alone matched it.
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

/// A flag set or cleared: a name the guard word `flag "<name>"` reads.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Flag {
    /// Its name, matched ignoring case.
    pub name: String,
    /// Seconds it stays set, by the game's clock; `None` until cleared.
    pub seconds: Option<u32>,
    /// Clear it instead.
    #[serde(default)]
    pub clear: bool,
}

impl Flag {
    /// The change this makes to the flags at game second `now`.
    #[must_use]
    pub fn change(&self, now: Option<u32>) -> FlagChange {
        let until = if self.clear {
            None
        } else {
            Some(match (self.seconds, now) {
                (None, _) => Until::Cleared,
                (Some(seconds), Some(now)) => Until::Second(now.saturating_add(seconds)),
                (Some(_), None) => Until::Unknown,
            })
        };
        FlagChange {
            name: self.name.clone(),
            until,
        }
    }
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

impl Rule {
    /// Whether it is a condition, which watches the character and not a line.
    #[must_use]
    pub const fn is_condition(&self) -> bool {
        !self.condition.is_empty()
    }

    /// Whether every word of its `only_if` holds for `state`; with none, it
    /// does. The target is the one the game's target list names, and a word
    /// the game has not answered does not hold.
    #[must_use]
    pub fn only_if_holds(&self, state: &GameState) -> bool {
        let facts = Facts::new(state, state.targeting.current());
        self.only_if
            .iter()
            .all(|condition| condition.holds(&facts) == Some(true))
    }
}

impl Default for Rule {
    /// A rule that does nothing yet: a line's, with nothing to match. What a
    /// trigger is built on in code; the file's are checked.
    fn default() -> Self {
        Self {
            category: String::new(),
            priority: 0,
            pattern: None,
            event: None,
            condition: Vec::new(),
            rearm: REARM,
            only_if: Vec::new(),
            case_sensitive: false,
            stream: None,
            look: None,
            squelch: false,
            substitute: None,
            redirect: None,
            flag: None,
            sound: None,
            notify: None,
            alert: None,
            cooldown: COOLDOWN,
        }
    }
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
