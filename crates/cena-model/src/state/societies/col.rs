//! The Council of Light: 20 signs, powered by spirit and mana.
//!
//! Ports `lib/gemstone/societies/council_of_light.rb` (477 lines).
//!
//! # The spirit cost may be owed rather than paid
//!
//! Three signs -- Swords, Shields and Dissipation, plus Madness -- have
//! `cost_type: :dissipates`, which means their spirit is taken **when the
//! effect expires**, not when the sign goes up. That makes "can I afford this"
//! a question about the future: a member with 3 spirit and three dissipating
//! signs already active can put up a fourth and then find themselves at zero.
//!
//! `affordable?` (`council_of_light.rb:344-372`) handles it by adding
//! `pending_spirit_loss` -- the spirit owed by every active dissipating sign --
//! to the cost before comparing. That reads live effect state, so it lives
//! above the model; what the model owes it is the timing, which
//! [`CostTiming`] carries on every sign.
//!
//! # A spirit check Lich makes strictly and a mana check it does not
//!
//! `council_of_light.rb:359` tests `total_spirit < Char.spirit` while `:364`
//! tests `mana_cost <= Char.mana`. The asymmetry is deliberate and the wiki
//! explains why: dropping to **half or less** of maximum spirit is visible to
//! anyone in the room (`Council of Light.txt:103`, "Obvious Spirit Drain"), and
//! a Council member seen draining spirit in front of a non-member *"will be
//! stripped of his or her powers for a period of time determined by the
//! Poohbah"*. Spending your last spirit point is a real risk in a way that
//! spending your last mana point is not.
//!
//! [`affordable`] keeps both comparisons as Lich has them, and says so, because
//! a reader who "fixes" the `<` to `<=` is removing a safety margin that the
//! game enforces socially.

use super::{Ability, AbilityKind, Cost, CostTiming};
use crate::state::character::vocabulary::Society;

/// The rank a Council Master holds.
pub const MASTER_RANK: u8 = 20;

/// Look up one sign by short or long name.
#[must_use]
pub fn sign(name: &str) -> Option<&'static Ability> {
    Society::CouncilOfLight.ability(name)
}

/// Can this sign be paid for right now?
///
/// Ports `affordable?` (`council_of_light.rb:344-372`), including its
/// asymmetric comparisons -- see the module docs, where the wiki's reason is
/// recorded.
///
/// `pending_spirit` is the spirit already owed by active `Dissipates` signs; a
/// caller with no effect state passes 0, which is what Lich computes when
/// nothing is up.
///
/// ```
/// use cena_model::state::societies::col;
/// let healing = col::sign("healing").expect("rank 15 sign");
/// // 2 spirit, invoked: strictly more than 2 must be held.
/// assert!(!col::affordable(healing, 2, 0, 0));
/// assert!(col::affordable(healing, 3, 0, 0));
/// ```
#[must_use]
pub fn affordable(sign: &Ability, spirit: u16, mana: u16, pending_spirit: u16) -> bool {
    let Cost::SpiritMana {
        spirit: spirit_cost,
        mana: mana_cost,
        paid_when,
    } = sign.cost
    else {
        return false;
    };

    if spirit_cost > 0 {
        let mut owed = u16::from(spirit_cost);
        // A dissipating sign's cost is added to what is already owed, so the
        // member does not commit spirit twice over.
        if paid_when == Some(CostTiming::Dissipates) {
            owed = owed.saturating_add(pending_spirit);
        }
        // **Strictly less than**, per `council_of_light.rb:359`. Not a typo for
        // `<=`; the module docs record why.
        if owed >= spirit {
            return false;
        }
    }

    if mana_cost > 0 && u16::from(mana_cost) > mana {
        return false;
    }

    true
}

/// A sign's cost, with zero-and-zero collapsing to [`Cost::Free`].
///
/// The collapse is here rather than at each of the twenty call sites so that
/// "costs nothing" has one spelling in the table and `affordable` has one case
/// to answer for.
const fn cost(spirit: u8, mana: u8, paid_when: Option<CostTiming>) -> Cost {
    if spirit == 0 && mana == 0 {
        Cost::Free
    } else {
        Cost::SpiritMana {
            spirit,
            mana,
            paid_when,
        }
    }
}

/// One sign, with the boilerplate the table would otherwise repeat 20 times.
const fn sign_entry(
    rank: u8,
    short_name: &'static str,
    long_name: &'static str,
    kind: AbilityKind,
    cost: Cost,
    spell_number: u16,
) -> Ability {
    Ability {
        society: Society::CouncilOfLight,
        rank,
        short_name,
        long_name,
        kind,
        cost,
        spell_number,
        usage: None,
    }
}

/// Spent when invoked.
const INVOKED: Option<CostTiming> = Some(CostTiming::Invoked);
/// Spent when the effect expires.
const DISSIPATES: Option<CostTiming> = Some(CostTiming::Dissipates);

/// The 20 signs, in rank order.
///
/// `@@col_signs` (`council_of_light.rb:16-236`). VERIFIED against
/// `reference/wiki_clean/Council of Light.txt:71-90`: rank, spirit, mana and
/// cost timing for all twenty, zero disagreements.
pub const SIGNS: &[Ability] = &[
    sign_entry(
        1,
        "recognition",
        "Sign of Recognition",
        AbilityKind::Utility,
        cost(0, 0, None),
        9901,
    ),
    Ability {
        // Invoked by its own verb, not by naming it.
        usage: Some("signal"),
        ..sign_entry(
            2,
            "signal",
            "Sign of Signal",
            AbilityKind::Utility,
            cost(0, 0, None),
            9902,
        )
    },
    sign_entry(
        3,
        "warding",
        "Sign of Warding",
        AbilityKind::Defense,
        cost(0, 1, INVOKED),
        9903,
    ),
    sign_entry(
        4,
        "striking",
        "Sign of Striking",
        AbilityKind::Offense,
        cost(0, 1, INVOKED),
        9904,
    ),
    sign_entry(
        5,
        "clotting",
        "Sign of Clotting",
        AbilityKind::Utility,
        cost(0, 1, INVOKED),
        9905,
    ),
    sign_entry(
        6,
        "thought",
        "Sign of Thought",
        AbilityKind::Utility,
        cost(0, 1, INVOKED),
        9906,
    ),
    sign_entry(
        7,
        "defending",
        "Sign of Defending",
        AbilityKind::Defense,
        cost(0, 2, INVOKED),
        9907,
    ),
    sign_entry(
        8,
        "smiting",
        "Sign of Smiting",
        AbilityKind::Offense,
        cost(0, 2, INVOKED),
        9908,
    ),
    sign_entry(
        9,
        "staunching",
        "Sign of Staunching",
        AbilityKind::Utility,
        cost(0, 1, INVOKED),
        9909,
    ),
    sign_entry(
        10,
        "deflection",
        "Sign of Deflection",
        AbilityKind::Defense,
        cost(0, 3, INVOKED),
        9910,
    ),
    sign_entry(
        11,
        "hypnosis",
        "Sign of Hypnosis",
        AbilityKind::Utility,
        cost(1, 0, INVOKED),
        9911,
    ),
    sign_entry(
        12,
        "swords",
        "Sign of Swords",
        AbilityKind::Offense,
        cost(1, 0, DISSIPATES),
        9912,
    ),
    sign_entry(
        13,
        "shields",
        "Sign of Shields",
        AbilityKind::Defense,
        cost(1, 0, DISSIPATES),
        9913,
    ),
    sign_entry(
        14,
        "dissipation",
        "Sign of Dissipation",
        AbilityKind::Defense,
        cost(1, 0, DISSIPATES),
        9914,
    ),
    sign_entry(
        15,
        "healing",
        "Sign of Healing",
        AbilityKind::Utility,
        cost(2, 0, INVOKED),
        9915,
    ),
    sign_entry(
        16,
        "madness",
        "Sign of Madness",
        AbilityKind::Utility,
        cost(3, 0, DISSIPATES),
        9916,
    ),
    sign_entry(
        17,
        "possession",
        "Sign of Possession",
        AbilityKind::Utility,
        cost(4, 0, INVOKED),
        9917,
    ),
    sign_entry(
        18,
        "wracking",
        "Sign of Wracking",
        AbilityKind::Utility,
        cost(5, 0, INVOKED),
        9918,
    ),
    sign_entry(
        19,
        "darkness",
        "Sign of Darkness",
        AbilityKind::Utility,
        cost(6, 0, INVOKED),
        9919,
    ),
    sign_entry(
        20,
        "hopelessness",
        "Sign of Hopelessness",
        AbilityKind::Utility,
        cost(0, 0, None),
        9920,
    ),
];
