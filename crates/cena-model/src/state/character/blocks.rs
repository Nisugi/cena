//! Reading a command report out of a completed chunk.
//!
//! M3 step 4, **rewritten as a chunk consumer** (author's call, 2026-09-19):
//!
//! > *"The combat tracker parser, it chunks items and then parses the blob it
//! > chunked. Could it not use the same concept? same parser even?"*
//!
//! The concept, yes -- see `state/chunks.rs` for the three places Lich uses the
//! prompt as a boundary. The *parser*, no: Lich's combat parser re-scans raw XML
//! to recover `exist`, `noun` and bold, and four of its recorded bugs come from
//! exactly that. Cena's frames carry those typed, so a chunk here holds parsed
//! lines and this file needs no pattern for markup at all.
//!
//! # What that removed
//!
//! The first version of this file owned a buffer and a `Block` state machine:
//! lines were offered one at a time, an opener set `open = Block::Info`, and a
//! terminator committed. All of it is gone. **The chunk is already the report's
//! boundary**, so "which block is open" is answered by looking at the chunk's
//! own lines rather than remembered across them.
//!
//! What remains is a pure function of one chunk, which is a better shape for
//! the same reason a classifier is: same chunk in, same answer out, and nothing
//! left dirty when a connection drops mid-report.

use super::stats::{Identity, Stat, StatKind, StatLine};
use crate::state::chunks::Chunk;

/// A complete `info` report read from one chunk.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InfoReport {
    /// The stat lines, in wire order, each with whether its enhanced column
    /// arrived bolded.
    pub stats: Vec<(StatKind, StatLine, bool)>,
    /// Race, profession, gender and age.
    pub identity: Identity,
}

impl InfoReport {
    /// Read an `info` report out of a chunk, or `None` if it is not one.
    ///
    /// **The header is required.** `Name: ... Race: ... Profession: ...` is
    /// `info`'s first line of prose and Lich's own opener
    /// (`infomon/parser.rb:10`'s `CharRaceProf`). Without it, a chunk that
    /// merely contains a stat-shaped line -- a player typing one into a channel
    /// -- is not a report. That is the rule `state.rs:306-309` records for the
    /// idle warning: *"a player can say anything"*.
    ///
    /// **Nothing is returned for a chunk with no stat lines.** A caller must
    /// not be handed an empty update to apply.
    #[must_use]
    pub fn read(chunk: &Chunk) -> Option<Self> {
        let mut identity = None;
        let mut stats = Vec::new();

        for line in chunk.lines() {
            if let Some(found) = classify_identity(&line.text()) {
                // A second header in one chunk means two reports ran with no
                // prompt between. The later one wins, as it would if they had
                // arrived in separate chunks.
                identity = Some(found);
                stats.clear();
                continue;
            }
            if identity.is_none() {
                continue;
            }
            if let Some((stat, bolded)) =
                StatLine::classify_with_bold(&line.text(), &line.bold_refs())
            {
                stats.push((stat.kind, stat, bolded));
                continue;
            }
            if let (Some((gender, age)), Some(id)) =
                (classify_gender_age(&line.text()), identity.as_mut())
            {
                id.gender = Some(gender);
                id.age = Some(age);
            }
        }

        let identity = identity?;
        if stats.is_empty() {
            return None;
        }
        Some(Self { stats, identity })
    }

    /// Fold one stat line into a [`Stat`], preserving columns the line did not
    /// carry.
    ///
    /// **`normal` is only overwritten by a line that had it.** `info` sends two
    /// columns and `info full` three, so a plain `info` run after an `info full`
    /// must not erase the base values it never mentioned. That is the difference
    /// between "unknown" and "unchanged".
    #[must_use]
    pub fn merge_into(line: &StatLine, bolded: bool, previous: Stat) -> Stat {
        Stat {
            normal: line.normal().or(previous.normal),
            ascended: line.ascended().or(previous.ascended),
            enhanced: line.enhanced().or(previous.enhanced),
            // **Replaced, not `or`-ed.** Bold is a per-report observation: a
            // stat that stopped arriving bolded stopped being enhanced, because
            // the item was removed or paused. Carrying the old `true` forward
            // would make an expired enhancive permanent.
            enhanced_is_bolded: bolded,
        }
    }
}

/// Classify `info`'s opening line, which is also where race and profession live.
///
/// ```text
/// Name: Ashryn Race: Half-Elf  Profession: Ranger (shown as: Hero)
/// ```
///
/// # Three things deliberately not taken from this line
///
/// * **The name.** `infomon/parser.rb:237` says so outright: *"name captured
///   here, but do not rely on it - use XML instead"*. `<playerID>` and the
///   `<a exist>` link are authoritative.
/// * **`(shown as: Hero)`.** A title, not a profession, and Lich strips it too.
/// * **Anything at all, while Shroud of Deception is up.** The spell falsifies
///   race, profession, gender and age, and Lich refuses to store them while
///   spell 1212 is active (`parser.rb:243`, `:249`) -- while still storing the
///   numbers, which the shroud does not touch. **This function cannot see the
///   effect list, so the caller enforces it**; see `Character::consume_chunk`.
fn classify_identity(line: &str) -> Option<Identity> {
    let rest = line.strip_prefix("Name: ")?;
    let (_name, rest) = rest.split_once(" Race: ")?;
    let (race, profession) = rest.split_once(" Profession: ")?;
    let race = race.trim();
    // `(shown as: X)` and `(not shown)` are display titles; the profession is
    // whatever precedes the parenthesis.
    let profession = profession
        .split_once('(')
        .map_or(profession, |(before, _)| before)
        .trim();
    if race.is_empty() || profession.is_empty() {
        return None;
    }
    Some(Identity {
        race: Some(race.to_owned()),
        profession: Some(profession.to_owned()),
        ..Identity::default()
    })
}

/// Classify `info`'s second line: gender and age.
///
/// ```text
/// Gender: Male    Age: 36    Expr: 43,904,921    Level:  100
/// ```
///
/// **`Expr:` and `Level:` are read and discarded**, as Lich discards them
/// (`parser.rb:246`: *"level captured here, but do not rely on it - use XML
/// instead"*). They change continuously and `info` is a snapshot;
/// `<dialogData id='expr'>` carries the live values.
fn classify_gender_age(line: &str) -> Option<(String, u32)> {
    let rest = line.strip_prefix("Gender: ")?;
    let (gender, rest) = rest.split_once(" Age: ")?;
    let age = crate::state::numbers::grouped(rest.split_whitespace().next()?)?;
    let gender = gender.trim();
    if gender.is_empty() {
        return None;
    }
    Some((gender.to_owned(), age))
}
