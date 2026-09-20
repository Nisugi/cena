//! Flare announce lines: weapon scripts, enchants, GEFs, flourishes.
//!
//! Ports `Definitions::Flares.parse` (`flares.rb:398-448`). Each def carries
//! three flags the processor routes on (`flares.rb:7-16`):
//!
//! - `damaging` -- a damage line is expected to follow; a buff flare
//!   (acuity, tailwind) never produces one;
//! - `aoe` -- false when the flare is single-target even if its parent is not;
//! - `spawns` -- the flare casts an imbedded spell as a SEPARATE attack
//!   (Blink), so what follows in the chunk is its child.
//!
//! **Timing is not declared.** Whether a flare resolves before its swing
//! (dispel gloves) or after (most) is positional: *"a flare seen with no open
//! attack event is held and claimed by the next swing"*. That is the
//! processor's job; this only says what the line is.
//!
//! # The flaring weapon is a link after `Your `
//!
//! `flares.rb:415`, `WEAPON_LINK = %r{Your <a exist="(?<id>\d+)" ...>}`: a 2p
//! flare names its weapon as a link, and the id is the join key for claiming
//! pre-flares by weapon. Hand-ported (`defs::HAND_PORTED`): the weapon is the
//! first link whose run follows the text `Your `.

use super::defs::defs;
use super::target::{self, Actor, Pick};
use crate::state::chunks::ChunkLine;

/// One flare announce line, classified.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FlareLine {
    /// The def name: `acid`, `ensorcell`, `blink`, `dispel_flux`, ...
    pub name: String,
    /// A damage line is expected to follow.
    pub damaging: bool,
    /// May strike more than one target.
    pub aoe: bool,
    /// Casts an imbedded spell as a separate attack.
    pub spawns: bool,
    /// The target named, as captured.
    pub target_text: Option<String>,
    /// That target, resolved to its link.
    pub target: Option<Actor>,
    /// Whose item flared, for third-person forms. `None` for our own.
    pub attacker: Option<Actor>,
    /// The flaring weapon, when the line links it (2p forms).
    pub weapon: Option<Actor>,
}

/// The link whose run follows `Your ` -- the weapon a 2p flare names.
fn weapon_link(line: &ChunkLine) -> Option<Actor> {
    let text = line.text();
    let mut at = 0;
    for run in &line.runs.runs {
        let start = at;
        at += run.text.len();
        if run.link.is_none() {
            continue;
        }
        if text[..start].ends_with("Your ") {
            return target::link_in(line, start..at, Pick::First);
        }
    }
    None
}

impl FlareLine {
    /// Classify one line as a flare announcement.
    #[must_use]
    pub fn classify(line: &ChunkLine) -> Option<Self> {
        let text = line.text();
        let (def, caps) = defs().first_match("flare", &text)?;
        // A lazy attacker capture can land on the FIRST-PERSON form ("from
        // your hands", boil_blood): that is ours, not an attacker.
        let attacker = caps.name("attacker").and_then(|m| {
            let who = m.as_str().trim();
            if matches!(
                who.to_ascii_lowercase().as_str(),
                "you" | "your" | "yourself"
            ) {
                return None;
            }
            target::link_in(line, m.range(), Pick::Last).or_else(|| Some(Actor::unlinked(who)))
        });
        let weapon = defs()
            .any_match("flare_weapon_link", &text)
            .then(|| weapon_link(line))
            .flatten();
        Some(Self {
            name: def.name.clone(),
            damaging: def.flag("damaging"),
            aoe: def.flag("aoe"),
            spawns: def.flag("spawns"),
            target_text: caps.name("target").map(|m| m.as_str().to_owned()),
            target: caps
                .name("target")
                .and_then(|m| target::link_in(line, m.range(), Pick::First)),
            attacker,
            weapon,
        })
    }

    /// Our own item fired, rather than a nearby player's or a creature's.
    #[must_use]
    pub const fn is_ours(&self) -> bool {
        self.attacker.is_none()
    }
}
