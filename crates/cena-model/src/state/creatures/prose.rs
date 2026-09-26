//! Creature facts only a line of prose states: a kill that leaves no
//! corpse, a boss that leaves by a portal, and a boss's phase.
//!
//! # The gap this closes
//!
//! A hunt loots what [`CreatureInstance::corpse`] calls a corpse and fights
//! what [`CreatureInstance::valid_target`] allows, and both read the wire's
//! flags and hit points. Three endings come with no flag to say so:
//!
//! - **Implosion (720) vaporizes** its target: instant death, no body
//!   (`reference/wiki_clean/Implosion _720_.txt`, "Instant Vaporization
//!   Chance").
//! - **The Empyrean captain leaves by a portal**, a departure with no `<d>`
//!   direction, so `departure.rs` cannot see it and
//!   [`Creatures::vanished_unaccounted`] would report it as hiding.
//! - **The captain's corpse fades**, "leaving nothing behind", some seconds
//!   after it collapses.
//!
//! And one boss changes by phase: the cold wyrm on the ground, in the air,
//! or with its scales disrupting energy attacks.
//!
//! # Sources, under `reference/lich_repo_mirror/lib/`
//!
//! | fact | the line | from |
//! |---|---|---|
//! | [`Ending::Vaporized`] | `Rather abrupt decompression causes X to explode`, `Blast disperses the X into a fine mist`, `X inverts as the intense vacuum rips it to shreds` | `killcounter.lic:223`; the first also `grimcount.lic:42` |
//! | [`Ending::Portal`] | the captain, a `portal`, then `disappearing from sight`, `vanishing`, `winks away` or `dives into it` | `creaturewindow.lic:293` |
//! | [`Ending::Collapsed`] | the captain `collapses` | `creaturewindow.lic:294` |
//! | [`Ending::Faded`] | `fading away with a final shimmer`, `leaving nothing behind` | `creaturewindow.lic:294` |
//! | [`BossPhase`] | the cold wyrm `plummets toward the ground` ... `radiating wall of devastation`; `launches herself into the air`; `Corruscations of color` ... `disrupting the attack` | `creaturewindow.lic:1470-1474` |
//!
//! creaturewindow's comment (`:264-292`) is the evidence for the captain:
//! its flee wording is randomised per encounter, so the pattern keys on the
//! captain, the portal and a vanishing verb; and its death is two lines
//! about 35 seconds apart, the collapse and then the fading corpse. Its one
//! pattern for both is split here, because only the second leaves nothing.
//!
//! # Which creature
//!
//! The line's first bolded object (`ledger/text.rs`'s `creature`): bold is
//! the wire's mark for a creature. A captain or wyrm line that links none
//! falls back to the one creature in the room whose name holds the boss's;
//! none or two, and the line is not applied. A vaporization needs its link.
//! Only a creature the registry knows is changed, as with a departure.
//!
//! # Read on arrival, and a room listing outranks it
//!
//! `streams.rs` reads each line as it arrives, beside the hiding and group
//! readers and for their reason: order. A `room objs` listing that names the
//! creature after the line says it is here, and [`Creatures::apply_room_objs`]
//! clears its ending; the wire outranks an inference. A listing before the
//! line cannot. The phase is not cleared: the wyrm is listed on every
//! refresh and stays in the phase the last line stated.
//!
//! # What an ending changes
//!
//! - [`CreatureInstance::valid_target`] is false: dead or gone.
//! - [`CreatureInstance::corpse`] is true only for [`Ending::Collapsed`].
//! - A killed ending puts the creature on the death watch, and the sweep
//!   emits its `Fact::Dead` once, so the kill is counted though no `dead`
//!   flag may ever come.
//! - [`Ending::Portal`] is also a departure with no direction:
//!   [`Creatures::fled`] answers `Some(None)`.
//!
//! # UNVERIFIED
//!
//! - **Every line's wire shape.** No committed fixture carries one, and the
//!   log archive was not searched; the patterns are the scripts', and the
//!   tests are synthetic lines built from them.
//! - Whether a vaporized creature is ever listed again with `dead="1"`. If
//!   it is, the relisting rule makes it a corpse again.
//! - Whether the captain's corpse can be looted before it fades.
//!   [`Ending::Collapsed`] says it can.
//! - creaturewindow picks the wyrm's phase from its last 25 lines, grounded
//!   before airborne before shielded (`:1470-1474`, the window at
//!   `:942-943`). Here the latest line wins, the same answer whenever one
//!   phase line is in view; a deviation, recorded. The wiki's wording
//!   (`reference/wiki_clean/Silver-scaled cold wyrm.txt`) is older and says
//!   so, and is not ported.

use std::sync::OnceLock;

use super::Creatures;
use super::instance::CreatureInstance;
use crate::state::chunks::ChunkLine;
use crate::state::ledger::text::{Pat, creature};

/// How a creature left the fight, when only prose says so.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Ending {
    /// Killed and destroyed at once, by Implosion: no corpse ever lies.
    Vaporized,
    /// Killed, and its corpse lies until it fades: the Empyrean captain's
    /// defeat.
    Collapsed,
    /// Its corpse faded away, leaving nothing behind.
    Faded,
    /// Left through a portal, alive: the Empyrean captain's retreat.
    Portal,
}

impl Ending {
    /// Did it die? All but [`Self::Portal`].
    #[must_use]
    pub const fn killed(self) -> bool {
        !matches!(self, Self::Portal)
    }

    /// Is there a body to loot? Only after [`Self::Collapsed`].
    #[must_use]
    pub const fn leaves_corpse(self) -> bool {
        matches!(self, Self::Collapsed)
    }
}

/// A boss's phase, as the last line about it stated. Only the cold wyrm
/// states one.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum BossPhase {
    /// Plummeted to the ground with a radiating wall of devastation.
    Grounded,
    /// Launched herself into the air.
    Airborne,
    /// Colour playing along her scales, disrupting the attack.
    Shielded,
}

/// What one line states about a creature.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Stated {
    /// It left the fight.
    Ended(Ending),
    /// It is in this phase now.
    Phase(BossPhase),
}

/// A creature fact one line states, and whom it is about.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Told {
    /// The creature's `exist` id, from the line's first bolded object.
    pub id: Option<i64>,
    /// The boss the pattern names, lowercased: for a line that links no
    /// creature, the one creature in the room whose name holds it. `None`
    /// for a vaporization, which needs its link.
    pub boss: Option<&'static str>,
    /// What the line states.
    pub stated: Stated,
}

/// The captain's name, as creaturewindow's patterns spell it, lowercased.
const CAPTAIN: &str = "battle-worn empyrean captain";
/// The wyrm, as creaturewindow's patterns name it.
const WYRM: &str = "cold wyrm";

struct Patterns {
    vaporized: Pat,
    portal: Pat,
    faded: Pat,
    collapsed: Pat,
    grounded: Pat,
    airborne: Pat,
    shielded: Pat,
}

fn patterns() -> &'static Patterns {
    static P: OnceLock<Patterns> = OnceLock::new();
    P.get_or_init(|| Patterns {
        // killcounter.lic:223's three alternatives, over the plain text.
        vaporized: Pat::new(
            r"Rather abrupt decompression causes .+ to explode|Blast disperses the .+ into a fine mist|inverts as the intense vacuum rips it to shreds",
        ),
        portal: Pat::new(
            r"(?i)battle-worn Empyrean captain.*?portal.*?(?:disappearing from sight|vanishing|wink(?:s|ing)? away|dives? into it)",
        ),
        // creaturewindow.lic:294, split: the fading corpse, then the fall.
        faded: Pat::new(
            r"(?i)battle-worn Empyrean captain.*?(?:fading away with a final shimmer|leaving nothing behind)",
        ),
        collapsed: Pat::new(r"(?i)battle-worn Empyrean captain.*?collapses"),
        grounded: Pat::new(
            r"(?i)cold wyrm plummets toward the ground.*radiating wall of devastation",
        ),
        airborne: Pat::new(r"(?i)cold wyrm's muscles bunch and she launches herself into the air"),
        shielded: Pat::new(
            r"(?i)Corruscations of color play along a silver-scaled cold wyrm's scaled hide.*disrupting the attack",
        ),
    })
}

/// The creature fact a line states, if it states one. `text` is the line's
/// plain text, rendered once by the caller.
#[must_use]
pub fn read(line: &ChunkLine, text: &str) -> Option<Told> {
    let p = patterns();
    let (boss, stated) = if p.vaporized.is_match(text) {
        (None, Stated::Ended(Ending::Vaporized))
    } else if p.portal.is_match(text) {
        (Some(CAPTAIN), Stated::Ended(Ending::Portal))
    } else if p.faded.is_match(text) {
        (Some(CAPTAIN), Stated::Ended(Ending::Faded))
    } else if p.collapsed.is_match(text) {
        (Some(CAPTAIN), Stated::Ended(Ending::Collapsed))
    } else if p.grounded.is_match(text) {
        (Some(WYRM), Stated::Phase(BossPhase::Grounded))
    } else if p.airborne.is_match(text) {
        (Some(WYRM), Stated::Phase(BossPhase::Airborne))
    } else if p.shielded.is_match(text) {
        (Some(WYRM), Stated::Phase(BossPhase::Shielded))
    } else {
        return None;
    };
    let id = creature(line).and_then(|c| c.id.parse().ok());
    Some(Told { id, boss, stated })
}

impl Creatures {
    /// Apply what one line of prose states about a creature ([`read`]).
    /// The creature's id when it applied.
    pub(crate) fn read_prose(&mut self, line: &ChunkLine, text: &str) -> Option<i64> {
        let told = read(line, text)?;
        let id = match told.id {
            Some(id) => id,
            None => self.only_in_room(told.boss?)?,
        };
        let creature = self.instances.get_mut(&id)?;
        match told.stated {
            Stated::Phase(phase) => creature.phase = Some(phase),
            Stated::Ended(ending) => {
                creature.ending = Some(ending);
                if ending == Ending::Portal {
                    self.note_fled(id, None);
                }
                if ending.killed() {
                    self.watch_for_death(id);
                }
            }
        }
        Some(id)
    }

    /// The one creature in the room whose name holds `boss`, if exactly one
    /// does.
    fn only_in_room(&self, boss: &str) -> Option<i64> {
        let mut found = self
            .in_room()
            .filter(|c| c.name.to_ascii_lowercase().contains(boss))
            .map(|c| c.id);
        let first = found.next()?;
        found.next().is_none().then_some(first)
    }
}

impl CreatureInstance {
    /// **How it left the fight**, when a line of prose said so.
    ///
    /// `None`: no line did, the ordinary case, and the flags and the roster
    /// answer. A room listing that names it again clears it.
    #[must_use]
    pub const fn ending(&self) -> Option<Ending> {
        self.ending
    }

    /// **Its boss phase**, as the last line about it stated.
    ///
    /// `None`: no phase line seen, which is not [`BossPhase::Grounded`].
    /// Whether it is flying is also a `<crtrStatus>` status, when the wire
    /// sends one ([`StatusName::Flying`](crate::StatusName::Flying)).
    #[must_use]
    pub const fn phase(&self) -> Option<BossPhase> {
        self.phase
    }

    /// A room listing named it: it is here, whatever a line said.
    pub(super) const fn relisted(&mut self) {
        self.ending = None;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_pattern_compiles() {
        let p = patterns();
        assert!(
            [
                &p.vaporized,
                &p.portal,
                &p.faded,
                &p.collapsed,
                &p.grounded,
                &p.airborne,
                &p.shielded
            ]
            .iter()
            .all(|pat| pat.compiled())
        );
    }
}
