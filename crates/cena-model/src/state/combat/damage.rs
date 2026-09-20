//! Damage lines: `... and hit for 20 points of damage!` and its many shapes.
//!
//! Ports `Definitions::Damage.parse` (`damage.rb`). Four lists in one order --
//! basic, spell, environmental, fallback -- with the broad fallbacks last so
//! the specific patterns win their better target captures first.
//!
//! # The gate is a substring, and it is Lich's
//!
//! *"every damage pattern contains 'damage' except the maneuver 'N hits!'
//! form. The substring check skips the regex scan on the ~95% of lines that
//! can't match"* (`damage.rb:104-108`). Kept as written: it is a correctness
//! statement about the patterns, not a performance guess, and the test that
//! every damage pattern contains one of the two substrings makes it one.

use super::defs::defs;
use crate::state::chunks::ChunkLine;

/// One damage line, classified.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DamageLine {
    /// Points of damage.
    pub amount: u32,
    /// Which damage list matched: `basic`, `spell`, `environmental`,
    /// `fallback`.
    pub list: String,
    /// The target the line names, as captured, where the shape names one.
    pub target_text: Option<String>,
}

/// The substrings every damage pattern contains one of.
pub const GATE: [&str; 2] = ["damage", " hits!"];

impl DamageLine {
    /// Classify one line as a damage line.
    #[must_use]
    pub fn classify(line: &ChunkLine) -> Option<Self> {
        let text = line.text();
        if !GATE.iter().any(|g| text.contains(g)) {
            return None;
        }
        let (def, caps) = defs().first_match("damage", &text)?;
        Some(Self {
            amount: caps.name("damage")?.as_str().parse().ok()?,
            list: def.name.clone(),
            target_text: caps.name("target").map(|m| m.as_str().to_owned()),
        })
    }
}
