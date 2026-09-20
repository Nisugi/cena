//! Spell wear-off lines, pinned to a spell number.
//!
//! Ports `Definitions::SpellLosses.parse` and the resolution in
//! `Parser.parse_spell_loss` (`spell_losses.rb`, `parser.rb:274-290`).
//!
//! # These are not statuses, and they apply to players too
//!
//! *"Third-person wear-off lines of ordinary spell-circle spells, keyed by
//! spell number -- NOT creature statuses, and deliberately not new status
//! vocabulary"* (`spell_losses.rb:6-8`). ~90% ride behind a dispel flare (a
//! creature shedding its self-buffs); the rest are natural expiry, *"including
//! on player characters in view"*. So the subject resolves through **any**
//! link, negative player ids included, and falls back to the captured name for
//! a plain-text log.
//!
//! # Conversion discipline
//!
//! *"a line is only added here once it is pinned to a specific spell"* -- by
//! the wiki's first-person wording, or by cross-log timestamp pairing (how
//! 1109 was proven). Unpinned wear-offs stay residue by design. That is a
//! def-file rule, carried into the data unchanged.

use super::defs::defs;
use super::target::{self, Actor, Pick};
use crate::state::chunks::ChunkLine;

/// One spell wear-off line, classified.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SpellLoss {
    /// The spell number: 107, 1109, 1204, ...
    ///
    /// **`None` for the game's generic wear-off** -- *"X appears somehow
    /// different."* / *"X seems slightly different."* -- which says a spell
    /// ended without saying which. Lich keeps that def with `spell: nil` and
    /// `spell_name: 'unknown'` (`spell_losses.rb:107`) rather than guessing,
    /// and so does this.
    pub spell: Option<u16>,
    /// Its name: `Spirit Warding II`, `Empathic Focus`, or `unknown`.
    pub spell_name: String,
    /// Who lost it: a creature, a player, or a bare name.
    pub target: Actor,
}

impl SpellLoss {
    /// Classify one line as a spell wear-off.
    #[must_use]
    pub fn classify(line: &ChunkLine) -> Option<Self> {
        let text = line.text();
        let (def, caps) = defs().first_match("spell_loss", &text)?;
        let m = caps.name("target")?;
        let target = target::link_in(line, m.range(), Pick::First)
            .unwrap_or_else(|| Actor::unlinked(m.as_str()));
        Some(Self {
            spell: def.name.parse().ok(),
            spell_name: def.extra("spell_name").unwrap_or("").to_owned(),
            target,
        })
    }
}
