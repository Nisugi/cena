//! What the spell table did not keep, cut by `tools/extract_spell_extras.rb`
//! into `data/spell_extras.tsv` and joined to it by number (`plan/37` Stage 1).
//!
//! Each `<duration>`'s shape -- whether a cast **stacks** onto the time left
//! or **refreshes** it, whether it can be multicast, its ceiling, whether it
//! outlasts death and whether it runs in real time -- the `<spell>`'s own
//! `incant`, `stance` and `channel` flags, every `<cost>` as written, and the
//! `<cast-proc>`, Lich's Ruby for how the spell is cast when `incant` will not
//! do. The flags are yes-or-absent in the file, and `incant` is no-or-absent.

use std::collections::BTreeMap;

use super::{CastType, REC, UNIT};

const EXTRAS_TSV: &str = include_str!("../../data/spell_extras.tsv");

/// How another cast of a spell already up adds to it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Span {
    /// Its time is added to what is left, up to a ceiling.
    Stackable,
    /// It restarts the time.
    Refreshable,
}

/// One `<duration>`'s shape.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Shape {
    /// Which casting it describes.
    pub cast: CastType,
    /// Stacks or refreshes; `None` for neither (a cast while up does nothing).
    pub span: Option<Span>,
    /// Whether it can be multicast; `None` when the file does not say.
    pub multicastable: Option<bool>,
    /// The ceiling in minutes, as written (`max='250'`).
    pub max: Option<String>,
    /// Whether it outlasts death; `None` when the file does not say.
    pub persist_on_death: Option<bool>,
    /// It runs in real time, not game time.
    pub real_time: bool,
}

/// One `<cost>`, as written.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Cost {
    /// `mana`, `spirit`, `stamina` or `renew`.
    pub kind: String,
    /// The cost: a number, or Ruby for one.
    pub text: String,
    /// The file's own note on it, when it has one.
    pub fixme: Option<String>,
}

/// What the table did not keep about one spell.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Extras {
    /// `incant` casts it; `false` for the seventeen marked `incant='no'`.
    pub incant: bool,
    /// Wants an offensive stance to cast (`stance='yes'`).
    pub stance: bool,
    /// Can be channeled (`channel='yes'`).
    pub channel: bool,
    /// Each `<duration>`'s shape, in the file's order.
    pub shapes: Vec<Shape>,
    /// Every `<cost>`, in the file's order.
    pub costs: Vec<Cost>,
    /// Lich's `<cast-proc>`, when the spell has one.
    pub cast_proc: Option<String>,
    /// Every `<message>`, by type (`start`, `end`, ...), in the file's
    /// order: the table keeps one per type.
    pub messages: Vec<(String, String)>,
}

impl Default for Extras {
    fn default() -> Self {
        Self {
            incant: true,
            stance: false,
            channel: false,
            shapes: Vec::new(),
            costs: Vec::new(),
            cast_proc: None,
            messages: Vec::new(),
        }
    }
}

impl Extras {
    /// The shape of this casting, when the file gives one.
    #[must_use]
    pub fn shape(&self, cast: CastType) -> Option<&Shape> {
        self.shapes.iter().find(|s| s.cast == cast)
    }

    /// What the cast proc sends the game, read off it; `None` for a spell
    /// with no proc. See [`ProcLine`].
    #[must_use]
    pub fn proc_line(&self) -> Option<ProcLine> {
        self.cast_proc.as_deref().map(ProcLine::read)
    }
}

/// What a `<cast-proc>` sends the game: the first string it hands to
/// `fput`, `put`, `multifput` or `dothistimeout`, which is how every society
/// power's proc is written (`awk -F'\t' '$1>=9700 {print $7}'
/// data/spell_extras.tsv`). Lich runs the proc as Ruby; Hydra reads only the
/// line, so a proc's loops and waits (9918's `sign of wracking` until mana is
/// full) are not carried, only what it says.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ProcLine {
    /// This line. `targeted`: the proc appends the target to it
    /// (`"symbol of submission#{target}"`, `"smite#{target}"`), as
    /// ` #<id>` for a game object and ` <words>` otherwise.
    Sends {
        /// The line, without the target.
        line: String,
        /// The proc appends `#{target}`.
        targeted: bool,
    },
    /// It sends an empty line: `fput ""`, Kai's Strike (9808) and Symbol of
    /// Thought (9803), powers the Order of Voln made passive.
    Nothing,
    /// It sends nothing at all: 9920 (Sign of Hopelessness) is an `echo`
    /// asking what is wrong with whoever casts it.
    Refuses,
}

impl ProcLine {
    /// Read a proc as written.
    #[must_use]
    pub fn read(proc: &str) -> Self {
        let Some(quoted) = first_sent(proc) else {
            return Self::Refuses;
        };
        let targeted = quoted.contains("#{target}");
        let line = quoted.replace("#{target}", "").trim().to_owned();
        if line.is_empty() {
            Self::Nothing
        } else {
            Self::Sends { line, targeted }
        }
    }
}

/// The string literal handed to the first `fput`, `put`, `multifput` or
/// `dothistimeout` in `proc`, quotes off.
fn first_sent(proc: &str) -> Option<&str> {
    const SENDS: [&str; 4] = ["dothistimeout", "multifput", "fput", "put"];
    let word_start = |at: usize| {
        proc[..at]
            .chars()
            .next_back()
            .is_none_or(|c| !(c.is_ascii_alphanumeric() || c == '_'))
    };
    proc.char_indices().find_map(|(at, _)| {
        let verb = SENDS
            .iter()
            .find(|verb| proc[at..].starts_with(*verb) && word_start(at))?;
        let rest = proc[at + verb.len()..].trim_start_matches([' ', '(']);
        let quote = rest.chars().next().filter(|c| *c == '"' || *c == '\'')?;
        let body = &rest[1..];
        body.find(quote).map(|end| &body[..end])
    })
}

/// Every spell's extras, by number.
pub(super) fn read() -> BTreeMap<u16, Extras> {
    EXTRAS_TSV
        .lines()
        .filter(|line| !line.starts_with('#') && !line.starts_with("number\t"))
        .filter_map(read_row)
        .collect()
}

fn read_row(line: &str) -> Option<(u16, Extras)> {
    let mut field = line.split('\t');
    let number = field.next()?.parse().ok()?;
    let incant = field.next()? != "no";
    let stance = field.next()? == "yes";
    let channel = field.next()? == "yes";
    let shapes = field
        .next()?
        .split(REC)
        .filter(|cell| !cell.is_empty())
        .filter_map(read_shape)
        .collect();
    let costs = field
        .next()?
        .split(REC)
        .filter(|cell| !cell.is_empty())
        .filter_map(|cell| {
            let mut part = cell.split(UNIT);
            Some(Cost {
                kind: part.next()?.to_owned(),
                text: part.next()?.to_owned(),
                fixme: part.next().filter(|f| !f.is_empty()).map(str::to_owned),
            })
        })
        .collect();
    let cast_proc = field.next().filter(|p| !p.is_empty()).map(str::to_owned);
    let messages = field
        .next()
        .unwrap_or_default()
        .split(REC)
        .filter_map(|cell| cell.split_once(UNIT))
        .map(|(kind, text)| (kind.to_owned(), text.to_owned()))
        .collect();
    Some((
        number,
        Extras {
            incant,
            stance,
            channel,
            shapes,
            costs,
            cast_proc,
            messages,
        },
    ))
}

/// `cast-type~span~multicastable~max~persist-on-death~real-time`.
fn read_shape(cell: &str) -> Option<Shape> {
    let mut part = cell.split(UNIT);
    let cast = match part.next()? {
        "target" => CastType::Target,
        _ => CastType::SelfCast,
    };
    let span = match part.next()? {
        "stackable" => Some(Span::Stackable),
        "refreshable" => Some(Span::Refreshable),
        _ => None,
    };
    let yes_no = |value: &str| match value {
        "yes" => Some(true),
        "no" => Some(false),
        _ => None,
    };
    let multicastable = yes_no(part.next()?);
    let max = part.next().filter(|m| !m.is_empty()).map(str::to_owned);
    let persist_on_death = yes_no(part.next()?);
    let real_time = part.next() == Some("yes");
    Some(Shape {
        cast,
        span,
        multicastable,
        max,
        persist_on_death,
        real_time,
    })
}
