//! The ways home: which are available, and whether one worked.
//!
//! Ports the **reading half** of `gemstone/fog.rb` (269 lines). MEASURED over
//! its 21 entry points: **13 pure, 8 sending** — the same split `resolve.rs`
//! records for stash and `bank.rs` for bank, and for the same reason. Sending
//! `SPIRIT GUIDE` and waiting out roundtime is a behavior; knowing what you
//! could send is a question about the character.
//!
//! Lich's own header draws the line in the same place:
//!
//! > *"Policy stays with the caller: which method a profile picks, whether to
//! > fog at all, and any custom command list are the script's."*
//!
//! # Five ways out of the field
//!
//! Three are spells and two are society abilities, which is why this module
//! reads both the spell table and [`societies`](crate::state::societies).
//!
//! | Method | How | Needs |
//! |---|---|---|
//! | Spirit Guide | spell 130 | the spell known and affordable |
//! | Symbol of Return | Voln | the society, and the rank for it |
//! | Traveler's Song | spell 1020 | a bard |
//! | Sigil of Escape | Sunfist | the society, and the rank |
//! | Familiar Gate | spell 930 | the spell known and affordable |
//!
//! # Telling a move from standing still
//!
//! `moved_from` is the subtle part, and Lich's comment explains why
//! (`fog.rb:230`): `XMLData.room_id` is an **MD5 of the room's text** when the
//! room has no UID, so **two unmapped rooms that read the same share an id**.
//! A fog between them looks like standing still.
//!
//! Lich breaks the tie with a counter of room *streams*
//! (`xmlparser.rb:579`), which also ticks when the room you are standing in
//! refreshes. Cena counts *arrivals* instead — [`GameState::arrivals`],
//! incremented in `Room::arrive`, which already distinguishes a re-declaration
//! of the same room from a move. That is the same tie-break with one fewer
//! false positive.
//!
//! # What is NOT ported
//!
//! `return`, `cast_and_settle`, `pulse_mana` and `wait_for_move` send commands
//! and sleep. They belong to `cena-behavior` at M6, with the rest of the
//! deferred half. `second_cast_from_rift` is pure but is a rule *about* that
//! sequence, so it goes with them.

use crate::spells;
use crate::state::GameState;
use crate::state::character::vocabulary::Society;

/// A way out of the field.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum FogMethod {
    /// Spell 130.
    SpiritGuide,
    /// An Order of Voln symbol.
    SymbolOfReturn,
    /// Spell 1020, a bard's.
    TravelersSong,
    /// A Guardians of Sunfist sigil.
    SigilOfEscape,
    /// Spell 930.
    FamiliarGate,
}

impl FogMethod {
    /// Every method, in `fog.rb:34`'s order — which is also bigshot's
    /// numbering, so [`Self::from_number`] indexes this.
    pub const ALL: [Self; 5] = [
        Self::SpiritGuide,
        Self::SymbolOfReturn,
        Self::TravelersSong,
        Self::SigilOfEscape,
        Self::FamiliarGate,
    ];

    /// The spell number, for the three that are spells.
    #[must_use]
    pub fn spell(self) -> Option<u16> {
        Some(match self {
            Self::SpiritGuide => 130,
            Self::TravelersSong => 1020,
            Self::FamiliarGate => 930,
            Self::SymbolOfReturn | Self::SigilOfEscape => return None,
        })
    }

    /// The society and ability name, for the two that are society abilities.
    #[must_use]
    pub fn ability(self) -> Option<(Society, &'static str)> {
        Some(match self {
            Self::SymbolOfReturn => (Society::OrderOfVoln, "return"),
            Self::SigilOfEscape => (Society::GuardiansOfSunfist, "escape"),
            _ => return None,
        })
    }

    /// Read bigshot's `fog_return` number, 1–5 (`fog.rb:37`).
    ///
    /// **6 is not a method**: bigshot uses it for "the profile's own command
    /// list", which is policy and belongs to the caller. `None` rather than a
    /// sixth variant, so a profile carrying 6 cannot be read as a method
    /// nobody implemented.
    #[must_use]
    pub fn from_number(number: u8) -> Option<Self> {
        Self::ALL.get(usize::from(number.checked_sub(1)?)).copied()
    }

    /// Its name as `fog.rb` spells it.
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::SpiritGuide => "spirit_guide",
            Self::SymbolOfReturn => "symbol_of_return",
            Self::TravelersSong => "travelers_song",
            Self::SigilOfEscape => "sigil_of_escape",
            Self::FamiliarGate => "familiar_gate",
        }
    }

    /// Read a name or a number, as `normalize` does (`fog.rb:44`).
    #[must_use]
    pub fn parse(text: &str) -> Option<Self> {
        let text = text.trim();
        if let Ok(number) = text.parse::<u8>() {
            return Self::from_number(number);
        }
        Self::ALL
            .into_iter()
            .find(|method| method.as_str().eq_ignore_ascii_case(text))
    }
}

/// Where the character was, for comparing against where they are.
///
/// **Two fields, not one.** See the module doc: a room id is not unique when
/// the room has no UID.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Position {
    /// The room id, as the server reports it.
    pub id: Option<String>,
    /// How many rooms had been entered when this was taken.
    pub arrivals: u32,
}

/// The Rift's exit (`fog.rb:21`).
///
/// A fog landing here needs a second cast to go on.
pub const RIFT_ROOM: &str = "2635";

/// Real room UIDs are small (`fog.rb:31`).
///
/// Above this the id is an MD5 stand-in for a room with no UID, which is not
/// unique across rooms whose text reads the same.
pub const MAX_ROOM_UID: u64 = 1_000_000_000;

/// The society whose rank stands in for a circle's ranks (`spell.rb:504`).
///
/// Sunfist, Voln and the Council each have a spell circle whose "ranks" are
/// the character's rank in that society.
#[must_use]
pub fn society_circle(circle: u16) -> Option<Society> {
    Some(match circle {
        97 => Society::GuardiansOfSunfist,
        98 => Society::OrderOfVoln,
        99 => Society::CouncilOfLight,
        _ => return None,
    })
}

/// Whether an id is a real UID or the MD5 stand-in.
#[must_use]
pub fn uidless(id: &str) -> bool {
    id.parse::<u64>().ok().is_none_or(|uid| uid > MAX_ROOM_UID)
}

impl GameState {
    /// Where the character is now, for a later [`Self::moved_from`].
    #[must_use]
    pub fn position(&self) -> Position {
        Position {
            id: self.room.id.clone(),
            arrivals: self.arrivals,
        }
    }

    /// Whether the character has moved since `start`.
    ///
    /// Ports `moved_from?` (`fog.rb:236`) and its reasoning:
    ///
    /// * A **different id** is always a move.
    /// * The **same real UID** is never a move, because a UID is unique.
    /// * The **same MD5 id** may still be a move, between two unmapped rooms
    ///   whose text reads the same — so the arrival count decides.
    #[must_use]
    pub fn moved_from(&self, start: &Position) -> bool {
        let now = self.position();
        if now.id != start.id {
            return true;
        }
        // An id-less room cannot be compared: `Room::arrive` treats two
        // consecutive id-less arrivals as two rooms, and so does this.
        let Some(id) = now.id.as_deref() else {
            return now.arrivals != start.arrivals;
        };
        uidless(id) && now.arrivals != start.arrivals
    }

    /// Whether the character knows this way home at all.
    ///
    /// `known?` (`fog.rb:53`). **`None` when nothing has been read** — a
    /// character whose spell list and society have never arrived does not
    /// "not know" Spirit Guide (§5.2), and answering `false` would have a
    /// travel behavior conclude it is stranded.
    #[must_use]
    pub fn knows_fog(&self, method: FogMethod) -> Option<bool> {
        if let Some(number) = method.spell() {
            return self.knows_spell(number);
        }
        let (society, ability) = method.ability()?;
        let held = self.character.standing.society?;
        if held != Some(society) {
            return Some(false);
        }
        let rank = self.character.standing.society_rank?;
        Some(society.ability(ability).is_some_and(|a| a.rank <= rank))
    }

    /// Whether a spell is known.
    ///
    /// Ports `Spell#known?` (`spell.rb:464`), whose rule is
    /// `(num % 100) <= ranks` — the spell's position within its circle
    /// against ranks in that circle. **An earlier draft here stopped at that
    /// line and got three things wrong**, each read out of the source
    /// afterwards:
    ///
    /// 1. **Ranks are capped by level**: `[Spells.wizard, XMLData.level].min`
    ///    (`:487`). A character with 30 circle ranks at level 10 knows the
    ///    10th spell, not the 30th.
    /// 2. **Circles 97, 98 and 99 use SOCIETY rank**, not circle ranks
    ///    (`:504-509`) — and only while a member of that society. Those are
    ///    Sunfist, Voln and the Council, so this matters to two of the five
    ///    ways home.
    /// 3. **Circle 17 is profession-gated and circle 96 is always false**
    ///    (`:497`, `:510`). Arcane is known only to a Wizard, Cleric, Empath,
    ///    Sorcerer or Savant, and only spell 1700; Combat Maneuvers were
    ///    deprecated out of `Spell` entirely.
    ///
    /// `None` when an input has never been read (§5.2). **Not ported:** `SK`,
    /// which grants a spell outside the rank rules (`:465`) and needs a
    /// feature Cena does not have.
    #[must_use]
    pub fn knows_spell(&self, number: u16) -> Option<bool> {
        let spell = spells::spell(number)?;
        let circle = spell.circle();
        // Circle 96 is deprecated out of the spell rules entirely, and 17 is
        // one spell gated on profession rather than on ranks.
        if circle == 96 {
            return Some(false);
        }
        if circle == 17 {
            const ARCANE: [&str; 5] = ["Wizard", "Cleric", "Empath", "Sorcerer", "Savant"];
            let profession = self.character.identity.profession.as_deref()?;
            return Some(number == 1700 && ARCANE.contains(&profession));
        }

        let ranks = if let Some(society) = society_circle(circle) {
            // A society circle's ranks are the society rank, and only while
            // a member -- a lapsed member knows none of them.
            match self.character.standing.society? {
                Some(held) if held == society => u16::from(self.character.standing.society_rank?),
                _ => return Some(false),
            }
        } else {
            let name = spells::circle_name(circle)?;
            let circle_ranks = self.character.skills.circle(name)?;
            // **Capped by level** (`spell.rb:474`), which the first draft of
            // this method omitted.
            circle_ranks.min(self.level()?)
        };
        Some(number % 100 <= ranks)
    }

    /// The character's level as a number.
    ///
    /// `Experience::level` is the wire's label verbatim -- `"Level 100"` --
    /// by the deliberate decision its own doc records.
    fn level(&self) -> Option<u16> {
        let label = self.character.experience.level.as_deref()?;
        label
            .trim()
            .rsplit_once(' ')
            .map_or(label.trim(), |(_, n)| n)
            .parse()
            .ok()
    }

    /// Every way home the character can use **right now**.
    ///
    /// `available` (`fog.rb:77`). Known, and affordable: a spell whose mana
    /// cost exceeds current mana is not a way home this instant, which is the
    /// question a travel behavior is asking.
    ///
    /// A method whose inputs are unknown is **omitted rather than assumed
    /// available** — the failure of including one is a behavior committing to
    /// a fog it cannot cast.
    #[must_use]
    pub fn fog_available(&self) -> Vec<FogMethod> {
        FogMethod::ALL
            .into_iter()
            .filter(|method| self.fog_is_available(*method) == Some(true))
            .collect()
    }

    /// Whether one way home is usable right now.
    ///
    /// `None` when something it depends on has never been read.
    #[must_use]
    pub fn fog_is_available(&self, method: FogMethod) -> Option<bool> {
        if !self.knows_fog(method)? {
            return Some(false);
        }
        let Some(number) = method.spell() else {
            // A society ability's cost is favor or stamina, which this model
            // does not yet track. Known is as far as it can honestly go.
            return Some(true);
        };
        let spell = spells::spell(number)?;
        let Some(cost) = spell.mana else {
            return Some(true);
        };
        let mana = self.mana()?;
        let (current, _) = mana.amount()?;
        Some(i64::from(current) >= i64::from(cost))
    }
}
