//! The Order of Voln: 26 symbols, powered by favor.
//!
//! Ports `lib/gemstone/societies/order_of_voln.rb` (540 lines).
//!
//! # Favor cost is level-scaled, and the table is the game's
//!
//! Every symbol costs a fraction of one number: the favor cost of Symbol of
//! Return at the member's character level. [`BASE_FAVOR_COST_BY_LEVEL`] is that
//! number for levels 3..=100, and each symbol carries the fraction. Lich does
//! the same (`order_of_voln.rb:279-285`) and cites its source at line 10:
//! <https://gswiki.play.net/Favor#Symbol_Use_Favor_Cost>.
//!
//! **That citation was checked rather than trusted.** The page is in
//! `reference/wiki_clean/Favor.txt`, section `==Symbol Use Favor Cost==`, and
//! all 98 of its values match Lich exactly. The wiki also states that levels 3
//! to 42 and level 100 are measured in game and 43-99 are interpolated,
//! *"accurate to within 1 or 2 favor"* -- so [`favor_cost`] is an estimate in
//! that band, and a caller deciding whether a symbol is affordable should
//! leave itself that margin.
//!
//! # The one place Lich and the wiki disagree
//!
//! **Symbol of Retribution has two costs and Lich models one.**
//!
//! The wiki's factor table lists `Retribution (attack version)` at 0.04 and
//! `Retribution (self-cast version)` at 0.30. `order_of_voln.rb:182` stores
//! 0.04 and writes the other in a comment -- `# attack version, 0.30 for
//! selfcast` -- but, unlike Symbol of Blessing three hundred lines above, never
//! puts it in an `alt_cost_modifier` field. So `OrderOfVoln.affordable?`
//! reports a self-cast Retribution as costing an eighth of what it does, and at
//! level 100 that is 87 favor against a true 653.
//!
//! This is ported as a **fix, not a reproduction**: the alternate is in the
//! table, and `retribution_has_both_costs` in the tests asserts it. The
//! structure Lich already has for Blessing is the structure this needed;
//! nothing was invented to hold it.
//!
//! # Free is not the same as unknown
//!
//! Five symbols cost nothing and one costs nothing *because it does nothing*.
//! Recognition, Kai's Strike, Kai's Smite and Seeking are [`Cost::Free`] with a
//! `cost_modifier` of 0.00 in Lich. **Symbol of Thought is different**: its
//! modifier is `nil`, and `calculate_cost` returns `nil` for it
//! (`order_of_voln.rb:280`), because the symbol was retired -- its own summary
//! says it *"is no longer required and will be replaced in the future"*. Both
//! are `Free` here, since a retired symbol that cannot be used costs nothing to
//! not use, and the distinction a caller actually wants is carried by the
//! summary text rather than by a third cost state.

use super::{Ability, AbilityKind, AlternateCost, Cost};
use crate::state::character::vocabulary::Society;

/// Favor cost of one Symbol of Return activation, indexed by character level.
///
/// Index 0, 1 and 2 are `None`: the Order does not admit characters below
/// level 3, so there is no cost for those levels rather than a cost of zero.
/// Lich writes the same three as `nil` (`order_of_voln.rb:264`).
///
/// VERIFIED against `reference/wiki_clean/Favor.txt`, section
/// `==Symbol Use Favor Cost==`: 98 values, zero disagreements.
pub const BASE_FAVOR_COST_BY_LEVEL: [Option<u16>; 101] = {
    let mut table = [None; 101];
    let costs: [u16; 98] = [
        13, 22, 32, 43, 56, 70, 85, 100, 117, 134, 151, 169, 188, 207, 226, 246, 266, 286, 307,
        328, 349, 370, 391, 412, 434, 456, 478, 500, 522, 544, 577, 590, 613, 636, 659, 682, 705,
        728, 751, 774, 797, 820, 843, 866, 889, 912, 935, 958, 981, 1004, 1027, 1050, 1073, 1097,
        1121, 1145, 1169, 1193, 1217, 1241, 1265, 1289, 1313, 1337, 1361, 1385, 1409, 1433, 1457,
        1481, 1505, 1529, 1553, 1577, 1601, 1625, 1649, 1674, 1699, 1724, 1749, 1774, 1799, 1824,
        1849, 1874, 1899, 1924, 1949, 1974, 1999, 2024, 2049, 2074, 2099, 2124, 2149, 2174,
    ];
    let mut level = 3;
    while level <= 100 {
        table[level] = Some(costs[level - 3]);
        level += 1;
    }
    table
};

/// The lowest level at which the Order grants symbols.
pub const MIN_LEVEL: usize = 3;

/// The rank a Voln Master holds.
pub const MASTER_RANK: u8 = 26;

/// What one use of a symbol costs, at this character level.
///
/// Ports `calculate_cost` (`order_of_voln.rb:279-285`), including its rounding:
/// Ruby's `.ceil` on the product, so a cost is never rounded down in the
/// member's favour.
///
/// Returns `None` when the symbol is free, when the level is outside 3..=100,
/// or when the level has no measured cost.
///
/// ```
/// use cena_model::state::societies::voln;
/// // Transcendence is 0.60 of the base; at level 40 that is 728 * 0.60.
/// let transcendence = voln::symbol("transcendence").expect("rank 12 symbol");
/// assert_eq!(voln::favor_cost(&transcendence.cost, 40), Some(437));
/// ```
#[must_use]
pub fn favor_cost(cost: &Cost, level: usize) -> Option<u32> {
    let Cost::Favor { modifier, .. } = cost else {
        return None;
    };
    favor_cost_with(*modifier, level)
}

/// The alternate cost, for the two symbols that have one.
///
/// Returns `None` when the symbol has no alternate use, which is every symbol
/// but Blessing and Retribution.
#[must_use]
pub fn alternate_favor_cost(cost: &Cost, level: usize) -> Option<u32> {
    let Cost::Favor {
        alternate: Some(alt),
        ..
    } = cost
    else {
        return None;
    };
    favor_cost_with(alt.modifier, level)
}

/// Resolve one modifier against the level table.
fn favor_cost_with(modifier: f64, level: usize) -> Option<u32> {
    if modifier <= 0.0 {
        return None;
    }
    let base = f64::from(*BASE_FAVOR_COST_BY_LEVEL.get(level)?.as_ref()?);
    // Ruby's `(base * modifier).to_f.ceil`. The product of a value under 2200
    // and a modifier at most 1.0 cannot leave u32.
    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    Some((base * modifier).ceil() as u32)
}

/// Look up one symbol by short or long name.
///
/// ```
/// use cena_model::state::societies::voln;
/// assert_eq!(voln::symbol("holiness").map(|s| s.rank), Some(9));
/// assert_eq!(voln::symbol("Symbol of Holiness").map(|s| s.rank), Some(9));
/// assert!(voln::symbol("nothing of the kind").is_none());
/// ```
#[must_use]
pub fn symbol(name: &str) -> Option<&'static Ability> {
    Society::OrderOfVoln.ability(name)
}

/// One symbol, with the boilerplate the table would otherwise repeat 26 times.
const fn symbol_entry(
    rank: u8,
    short_name: &'static str,
    long_name: &'static str,
    kind: AbilityKind,
    cost: Cost,
    spell_number: u16,
) -> Ability {
    Ability {
        society: Society::OrderOfVoln,
        rank,
        short_name,
        long_name,
        kind,
        cost,
        spell_number,
        usage: None,
    }
}

/// A symbol whose cost is a plain fraction of the base.
const fn favor(modifier: f64) -> Cost {
    Cost::Favor {
        modifier,
        alternate: None,
    }
}

/// The 26 symbols, in rank order.
///
/// `@@voln_symbols` (`order_of_voln.rb:22-259`). The `:summary` and `:duration`
/// lambdas are **not** ported: they are display strings built from live rank
/// and level, which is a presentation concern above the model, and every one of
/// them is a formula a caller can apply itself from `rank`.
pub const SYMBOLS: &[Ability] = &[
    symbol_entry(
        1,
        "recognition",
        "Symbol of Recognition",
        AbilityKind::Utility,
        Cost::Free,
        9801,
    ),
    Ability {
        // **Blessing has two costs and Lich models both** -- the structure that
        // Retribution below needed and did not get.
        cost: Cost::Favor {
            modifier: 0.20,
            alternate: Some(AlternateCost {
                modifier: 0.04,
                reason: "non-magical",
            }),
        },
        ..symbol_entry(
            2,
            "blessing",
            "Symbol of Blessing",
            AbilityKind::Utility,
            Cost::Free,
            9802,
        )
    },
    symbol_entry(
        3,
        "thought",
        "Symbol of Thought",
        AbilityKind::Utility,
        // Retired. Lich stores `cost_modifier: nil`; see the module docs.
        Cost::Free,
        9803,
    ),
    symbol_entry(
        4,
        "diminishment",
        "Symbol of Diminishment",
        AbilityKind::Attack,
        favor(0.30),
        9804,
    ),
    symbol_entry(
        5,
        "courage",
        "Symbol of Courage",
        AbilityKind::Offense,
        favor(0.10),
        9805,
    ),
    symbol_entry(
        6,
        "protection",
        "Symbol of Protection",
        AbilityKind::Defense,
        favor(0.10),
        9806,
    ),
    symbol_entry(
        7,
        "submission",
        "Symbol of Submission",
        AbilityKind::Attack,
        favor(0.30),
        9807,
    ),
    symbol_entry(
        8,
        "strike",
        "Kai's Strike",
        AbilityKind::Utility,
        Cost::Free,
        9808,
    ),
    symbol_entry(
        9,
        "holiness",
        "Symbol of Holiness",
        AbilityKind::Attack,
        favor(0.30),
        9809,
    ),
    symbol_entry(
        10,
        "recall",
        "Symbol of Recall",
        AbilityKind::Utility,
        favor(0.40),
        9810,
    ),
    symbol_entry(
        11,
        "sleep",
        "Symbol of Sleep",
        AbilityKind::Attack,
        // Base cost. The mass version costs more per target, which neither
        // Lich nor the wiki quantifies.
        favor(0.20),
        9811,
    ),
    symbol_entry(
        12,
        "transcendence",
        "Symbol of Transcendence",
        AbilityKind::Defense,
        favor(0.60),
        9812,
    ),
    symbol_entry(
        13,
        "mana",
        "Symbol of Mana",
        AbilityKind::Utility,
        favor(0.30),
        9813,
    ),
    symbol_entry(
        14,
        "sight",
        "Symbol of Sight",
        AbilityKind::Utility,
        favor(0.30),
        9814,
    ),
    Ability {
        // **THE FIX.** Lich stores only the attack cost and writes the
        // self-cast one in a comment (`order_of_voln.rb:182`); the wiki lists
        // both. See the module docs.
        cost: Cost::Favor {
            modifier: 0.04,
            alternate: Some(AlternateCost {
                modifier: 0.30,
                reason: "self-cast",
            }),
        },
        ..symbol_entry(
            15,
            "retribution",
            "Symbol of Retribution",
            AbilityKind::Attack,
            Cost::Free,
            9815,
        )
    },
    symbol_entry(
        16,
        "supremacy",
        "Symbol of Supremacy",
        AbilityKind::Offense,
        favor(0.50),
        9816,
    ),
    symbol_entry(
        17,
        "restoration",
        "Symbol of Restoration",
        AbilityKind::Utility,
        favor(0.40),
        9817,
    ),
    symbol_entry(
        18,
        "need",
        "Symbol of Need",
        AbilityKind::Utility,
        favor(0.40),
        9818,
    ),
    symbol_entry(
        19,
        "renewal",
        "Symbol of Renewal",
        AbilityKind::Utility,
        favor(0.50),
        9819,
    ),
    symbol_entry(
        20,
        "disruption",
        "Symbol of Disruption",
        AbilityKind::Attack,
        favor(0.30),
        9820,
    ),
    Ability {
        // Invoked by its own verb, not by naming it.
        usage: Some("smite"),
        ..symbol_entry(
            21,
            "smite",
            "Kai's Smite",
            AbilityKind::Attack,
            Cost::Free,
            9821,
        )
    },
    symbol_entry(
        22,
        "turning",
        "Symbol of Turning",
        AbilityKind::Attack,
        favor(0.30),
        9822,
    ),
    symbol_entry(
        23,
        "preservation",
        "Symbol of Preservation",
        AbilityKind::Utility,
        favor(0.60),
        9823,
    ),
    symbol_entry(
        24,
        "dreams",
        "Symbol of Dreams",
        AbilityKind::Utility,
        favor(0.60),
        9824,
    ),
    symbol_entry(
        25,
        "return",
        "Symbol of Return",
        AbilityKind::Utility,
        // 1.00 by definition: the base table IS Symbol of Return's cost.
        favor(1.00),
        9825,
    ),
    symbol_entry(
        26,
        "seeking",
        "Symbol of Seeking",
        AbilityKind::Utility,
        Cost::Free,
        9826,
    ),
];
