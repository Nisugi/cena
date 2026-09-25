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
        }
    }
}

impl Extras {
    /// The shape of this casting, when the file gives one.
    #[must_use]
    pub fn shape(&self, cast: CastType) -> Option<&Shape> {
        self.shapes.iter().find(|s| s.cast == cast)
    }
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
    Some((
        number,
        Extras {
            incant,
            stance,
            channel,
            shapes,
            costs,
            cast_proc,
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
