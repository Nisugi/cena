//! The `experience` command's report.
//!
//! # A block, not single lines
//!
//! Six lines, each carrying two facts in two columns:
//!
//! ```text
//!          Level: 100                        Fame: 1,453,539,090
//!     Experience: 43,904,921             Field Exp: 1,234/1,403
//!  Ascension Exp: 24,865,590         Recent Deaths: 0
//!      Total Exp: 68,770,511          Death's Sting: None
//!  Long-Term Exp: 493                        Deeds: 11
//!  Exp until lvl: 11,999,265
//! ```
//!
//! Read as a chunk for the reason `blocks.rs` gives: a chunk closed by a prompt
//! IS the report's boundary, so nothing has to remember which block is open.
//!
//! # Three numbers Lich states and discards
//!
//! `Experience:`, `Recent Deaths:` and `Exp until lvl:` are all matched by
//! Lich's patterns and none is captured (`infomon/parser.rb:17-21`). Rule 2.2
//! says nothing the wire states is dropped silently, so the first two are read
//! here. The third is deliberately not: see [`ExperienceReport`].

use super::vocabulary::DeathsSting;
use crate::state::chunks::Chunk;

/// What one `experience` report stated.
///
/// Every field `Option`, because the report's shape varies: a character with no
/// ascension experience has no `Ascension Exp:` line at all.
///
/// # `Exp until lvl` is not here
///
/// The wire states it and this drops it, which Rule 2.2 requires a reason for.
/// It is **derivable and volatile**: it is the remainder to the next level,
/// which `<progressBar id='nextLvlPB'>` already carries continuously and which
/// this report states once. Storing the snapshot beside the live value would
/// give a consumer two answers to one question, and the stale one would look
/// authoritative for being in the character's stored facts.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ExperienceReport {
    /// `Fame: -?[\d,]+`. Signed on the wire.
    pub fame: Option<i64>,
    /// `Experience: [\d,]+` -- the plain total Lich discards.
    pub experience: Option<u64>,
    /// `Field Exp: <current>/<max>`.
    pub field_experience: Option<(u32, u32)>,
    /// `Ascension Exp: [\d,]+`.
    pub ascension_experience: Option<u64>,
    /// `Recent Deaths: \d+` -- also discarded by Lich.
    pub recent_deaths: Option<u32>,
    /// `Total Exp: [\d,]+`.
    pub total_experience: Option<u64>,
    /// `Death's Sting: None|Light|...`.
    pub deaths_sting: Option<DeathsSting>,
    /// `Long-Term Exp: [\d,]+`.
    pub long_term_experience: Option<u32>,
    /// `Deeds: \d+`.
    pub deeds: Option<u32>,
}

impl ExperienceReport {
    /// Read an `experience` report out of a chunk, or `None` if it is not one.
    ///
    /// **A `Fame:` line is required**, which is Lich's own opener -- its
    /// comment on `parser.rb:16` says the pattern "serves as `ExprStart`". It is
    /// what stops a chunk that merely contains a `Deeds: 11`-shaped line from
    /// being read as a report.
    #[must_use]
    pub fn read(chunk: &Chunk) -> Option<Self> {
        let lines: Vec<String> = chunk
            .lines()
            .iter()
            .map(super::super::chunks::ChunkLine::text)
            .collect();
        if !lines.iter().any(|line| field(line, "Fame").is_some()) {
            return None;
        }
        let mut report = Self::default();
        for line in &lines {
            report.absorb(line);
        }
        Some(report)
    }

    /// Take whatever facts one line carries. Both columns, independently.
    fn absorb(&mut self, line: &str) {
        if let Some(value) = field(line, "Fame") {
            self.fame = signed(value);
        }
        // `Experience:` before `Ascension Exp:` would be wrong -- `field`
        // matches on a whole label, so they cannot collide, but the order is
        // stated here so a future looser matcher does not introduce it.
        if let Some(value) = field(line, "Experience") {
            self.experience = number(value);
        }
        if let Some(value) = field(line, "Field Exp")
            && let Some((current, max)) = value.split_once('/')
            && let (Some(current), Some(max)) = (number(current), number(max))
        {
            self.field_experience = Some((
                u32::try_from(current).unwrap_or(u32::MAX),
                u32::try_from(max).unwrap_or(u32::MAX),
            ));
        }
        if let Some(value) = field(line, "Ascension Exp") {
            self.ascension_experience = number(value);
        }
        if let Some(value) = field(line, "Recent Deaths") {
            self.recent_deaths = number(value).and_then(|n| u32::try_from(n).ok());
        }
        if let Some(value) = field(line, "Total Exp") {
            self.total_experience = number(value);
        }
        if let Some(value) = field(line, "Death's Sting") {
            self.deaths_sting = DeathsSting::parse(value);
        }
        if let Some(value) = field(line, "Long-Term Exp") {
            self.long_term_experience = number(value).and_then(|n| u32::try_from(n).ok());
        }
        if let Some(value) = field(line, "Deeds") {
            self.deeds = number(value).and_then(|n| u32::try_from(n).ok());
        }
    }
}

impl super::Experience {
    /// Fold an `experience` report: each number it stated replaces the one
    /// held, and one it did not state is left alone.
    ///
    /// Nothing is persisted, so nothing is marked taught -- see
    /// `Character::consume_chunk`. Moved down from `character.rs` under Rule
    /// 4.1 when the character's `<playerID>` took that file to its cap.
    pub(super) fn absorb(&mut self, report: &ExperienceReport) {
        if let Some(fame) = report.fame {
            self.fame = Some(fame);
        }
        if let Some(value) = report.experience {
            self.experience = Some(value);
        }
        if let Some((current, max)) = report.field_experience {
            self.field_experience = Some(current);
            self.field_experience_max = Some(max);
        }
        if let Some(value) = report.ascension_experience {
            self.ascension_experience = Some(value);
        }
        if let Some(value) = report.recent_deaths {
            self.recent_deaths = Some(value);
        }
        if let Some(value) = report.total_experience {
            self.total_experience = Some(value);
        }
        if let Some(sting) = report.deaths_sting {
            self.deaths_sting = Some(sting);
        }
        if let Some(value) = report.long_term_experience {
            self.long_term_experience = Some(value);
        }
        if let Some(value) = report.deeds {
            self.deeds = Some(value);
        }
    }
}

/// The value after `<label>: `, up to the next column or the line's end.
///
/// The report is two columns of `Label: value` separated by runs of spaces, so
/// a value ends at two spaces or at the end of the line. One space is not
/// enough: `Death's Sting: None` has a space inside its label.
fn field<'a>(line: &'a str, label: &str) -> Option<&'a str> {
    let at = line.find(label)?;
    // **`": "` is the whole boundary check**, and a word-boundary guard on the
    // left was written, mutation-tested, and REMOVED as unreachable.
    //
    // The worry was a label matching inside a longer one. It cannot happen with
    // these labels: each is followed by `": "` at its real site, and no label
    // is a colon-prefix of another. `Experience` contains `Exp`, but nothing
    // searches for a bare `Exp`; `Recent Deaths` looks like it contains `Deeds`
    // and does not. Two attempts to write a test that failed without the guard
    // both passed, so it was a branch no input could reach -- Rule -1, and
    // Rule 0's "a rule that is not enforced is a wish".
    //
    // If a label is ever added that IS a colon-prefix of another, this is the
    // line to fix, and `at > 0 && !line[..at].ends_with(char::is_whitespace)`
    // is what to put back.
    let rest = line[at + label.len()..].strip_prefix(": ")?;
    // A value ends at two spaces -- the column gap -- or the line's end. One
    // space is not enough: `Death's Sting` has a space inside its label, and
    // Lich's own patterns put `\s+` between the columns.
    Some(rest.split("  ").next()?.trim())
}

/// `"1,453,539,090"` -> `1453539090`. The shared, strict reader: see
/// `state/numbers.rs` for why `"12a3"` is `None` rather than `123`.
fn number(text: &str) -> Option<u64> {
    crate::state::numbers::grouped(text)
}

/// Same, keeping a leading `-`.
fn signed(text: &str) -> Option<i64> {
    crate::state::numbers::grouped(text)
}
