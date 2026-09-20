//! The Guardians of Sunfist: 20 sigils, powered by stamina and mana.
//!
//! Ports `lib/gemstone/societies/guardians_of_sunfist.rb` (407 lines).
//!
//! # The simplest of the three
//!
//! A sigil costs stamina, mana, or both, and both are paid on use -- there is
//! no level scaling as Voln has, and no deferred cost as the Council has. So
//! [`affordable`] is two comparisons, both `>=`
//! (`guardians_of_sunfist.rb:328-334`), with none of the asymmetry the Council
//! module has to explain.
//!
//! # Two costs the game varies and neither source pins down
//!
//! Lich flags both with a `TODO`:
//!
//! - **Sigil of Distraction** (`:99`) -- the table holds the single-target
//!   cost, and Lich notes *"Figure out how to calc room version cost instead"*.
//! - **Sigil of Escape** (`:186`) -- 75 stamina / 15 mana for the ordinary use,
//!   and *"figure out higher cost?"* for the emergency version usable while
//!   stunned or bound.
//!
//! The wiki does not quantify either, and it calls Sigil of Intimidation's mana
//! cost "Variable" where Lich commits to 5. All three are ported at the value
//! Lich holds, with the uncertainty recorded here rather than silently baked in
//! -- a caller comparing a sigil's cost against current stamina should treat
//! these three as lower bounds.

use super::{Ability, AbilityKind, Cost};
use crate::state::character::vocabulary::Society;

/// The rank a Sunfist Master holds.
pub const MASTER_RANK: u8 = 20;

/// Look up one sigil by short or long name.
#[must_use]
pub fn sigil(name: &str) -> Option<&'static Ability> {
    Society::GuardiansOfSunfist.ability(name)
}

/// Can this sigil be paid for right now?
///
/// Ports `affordable?` (`guardians_of_sunfist.rb:319-338`). Both comparisons
/// are `>=`: a sigil that costs exactly what the member has is affordable.
///
/// ```
/// use cena_model::state::societies::sunfist;
/// let power = sunfist::sigil("power").expect("rank 18 sigil");
/// // 50 stamina, no mana: exactly 50 is enough.
/// assert!(sunfist::affordable(power, 50, 0));
/// assert!(!sunfist::affordable(power, 49, 0));
/// ```
#[must_use]
pub fn affordable(sigil: &Ability, stamina: u16, mana: u16) -> bool {
    let (stamina_cost, mana_cost) = match sigil.cost {
        Cost::Free => return true,
        Cost::StaminaMana { stamina, mana } => (stamina, mana),
        _ => return false,
    };
    if stamina_cost > 0 && stamina < u16::from(stamina_cost) {
        return false;
    }
    if mana_cost > 0 && mana < u16::from(mana_cost) {
        return false;
    }
    true
}

/// One sigil, with the boilerplate the table would otherwise repeat 20 times.
const fn sigil_entry(
    rank: u8,
    short_name: &'static str,
    long_name: &'static str,
    kind: AbilityKind,
    stamina: u8,
    mana: u8,
    spell_number: u16,
) -> Ability {
    Ability {
        society: Society::GuardiansOfSunfist,
        rank,
        short_name,
        long_name,
        kind,
        cost: if stamina == 0 && mana == 0 {
            Cost::Free
        } else {
            Cost::StaminaMana { stamina, mana }
        },
        spell_number,
        usage: None,
    }
}

/// The 20 sigils, in rank order.
///
/// `@@sunfist_sigils` (`guardians_of_sunfist.rb:18-191`). VERIFIED against
/// `reference/wiki_clean/Guardians of Sunfist.txt:131-149`: rank, stamina and
/// mana, zero disagreements on every value the wiki states.
///
/// **`:type` is absent from every Sunfist entry but the first.** Lich declares
/// `type: :utility` on Recognition and omits the field on the other nineteen,
/// so `sigil[:type]` is `nil` for them -- which means a caller filtering
/// Sunfist sigils by kind gets one result. The kinds below are assigned from
/// each sigil's stated effect (a DS bonus is `Defense`, an AS bonus `Offense`,
/// an enemy debuff `Attack`), because a table where nineteen of twenty entries
/// answer `nil` is not worth porting faithfully.
pub const SIGILS: &[Ability] = &[
    sigil_entry(
        1,
        "recognition",
        "Sigil of Recognition",
        AbilityKind::Utility,
        0,
        0,
        9701,
    ),
    sigil_entry(
        2,
        "location",
        "Sigil of Location",
        AbilityKind::Utility,
        0,
        0,
        9702,
    ),
    sigil_entry(
        3,
        "contact",
        "Sigil of Contact",
        AbilityKind::Utility,
        0,
        1,
        9703,
    ),
    sigil_entry(
        4,
        "resolve",
        "Sigil of Resolve",
        AbilityKind::Utility,
        5,
        0,
        9704,
    ),
    sigil_entry(
        5,
        "minor bane",
        "Sigil of Minor Bane",
        AbilityKind::Offense,
        3,
        3,
        9705,
    ),
    sigil_entry(
        6,
        "bandages",
        "Sigil of Bandages",
        AbilityKind::Utility,
        10,
        0,
        9706,
    ),
    sigil_entry(
        7,
        "defense",
        "Sigil of Defense",
        AbilityKind::Defense,
        5,
        5,
        9707,
    ),
    sigil_entry(
        8,
        "offense",
        "Sigil of Offense",
        AbilityKind::Offense,
        5,
        5,
        9708,
    ),
    sigil_entry(
        9,
        "distraction",
        "Sigil of Distraction",
        AbilityKind::Attack,
        // Single-target cost; the room version costs more. See module docs.
        10,
        5,
        9709,
    ),
    sigil_entry(
        10,
        "minor protection",
        "Sigil of Minor Protection",
        AbilityKind::Defense,
        10,
        5,
        9710,
    ),
    sigil_entry(
        11,
        "focus",
        "Sigil of Focus",
        AbilityKind::Defense,
        5,
        5,
        9711,
    ),
    sigil_entry(
        12,
        "intimidation",
        "Sigil of Intimidation",
        AbilityKind::Attack,
        // The wiki calls this mana cost "Variable"; Lich commits to 5.
        10,
        5,
        9712,
    ),
    sigil_entry(
        13,
        "mending",
        "Sigil of Mending",
        AbilityKind::Utility,
        15,
        10,
        9713,
    ),
    sigil_entry(
        14,
        "concentration",
        "Sigil of Concentration",
        AbilityKind::Utility,
        30,
        0,
        9714,
    ),
    sigil_entry(
        15,
        "major bane",
        "Sigil of Major Bane",
        AbilityKind::Offense,
        10,
        10,
        9715,
    ),
    sigil_entry(
        16,
        "determination",
        "Sigil of Determination",
        AbilityKind::Utility,
        30,
        0,
        9716,
    ),
    sigil_entry(
        17,
        "health",
        "Sigil of Health",
        AbilityKind::Utility,
        20,
        10,
        9717,
    ),
    sigil_entry(
        18,
        "power",
        "Sigil of Power",
        AbilityKind::Utility,
        50,
        0,
        9718,
    ),
    sigil_entry(
        19,
        "major protection",
        "Sigil of Major Protection",
        AbilityKind::Defense,
        15,
        10,
        9719,
    ),
    sigil_entry(
        20,
        "escape",
        "Sigil of Escape",
        AbilityKind::Utility,
        // Ordinary use; the emergency version costs more. See module docs.
        75,
        15,
        9720,
    ),
];
