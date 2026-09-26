//! What a PSM costs to use, and whether it can be used now: Lich's
//! `affordable?` and `available?` (`lib/gemstone/psms.rb:101-145`).
//!
//! # The table
//!
//! Cut whole from Lich's six `:cost` tables (`lib/gemstone/psms/*.rb`) by
//! `tools/extract_psm_costs.rb` into `data/psm_costs.tsv`: 187 rows, 11 armor,
//! 80 combat maneuver, 33 feat, 33 shield, 24 weapon and 6 warcry, each with
//! its source line. Every cost is one gauge and a number, stamina for all but
//! Excoriate (10 mana, `feat.rb:131`), and two are two numbers: Burst of
//! Swiftness and Surge of Strength cost 60 stamina while their own cooldown
//! is up and 30 otherwise (`cman.rb:62`, `:576`).
//!
//! **Lich evaluates those two once, when `cman.rb` loads.** The conditional
//! sits inside the `@@combat_mans` hash literal (`cman.rb:22`), so its answer
//! is whatever the Cooldowns dialog held at load, and never changes after.
//! This port asks at the moment of the question, which is what the literal
//! was written to mean, and what bigshot's own burst gate does
//! (`cmd_burst`, `bigshot.lic:6454-6455`: 30, or 60 while cooling). That gate
//! passes at exactly 30 (`Char.stamina < 30` refuses); Lich's `affordable?`
//! does not (`psms.rb:118`, `cost < stamina`). This answers Lich's way.
//!
//! Two feats share the mnemonic `wps` (`weighting` and `padding`,
//! `feat.rb:258-271`). Both rows are kept; they cost the same, which the tool
//! checks, so a lookup by mnemonic does not depend on which it finds.
//!
//! Read on each lookup rather than parsed once into a `static`: a hunt asks
//! once a step, and 187 short lines is not worth a process global.
//!
//! # The rule, as Lich writes it
//!
//! [`GameState::psm_availability`] answers each of Lich's four tests
//! separately, so a caller can tell *why* as well as whether:
//!
//! | test | Lich | here |
//! |---|---|---|
//! | known | ranks `>=` 1 (`cman.rb:710-712`) | [`PsmSet::get`](super::PsmSet::get); unknown until the category's list is read |
//! | affordable | cost **`<`** current points, strictly (`psms.rb:118`) | the gauge's current points, when the bar states them |
//! | not cooling | not in `Cooldowns` by name (`psms.rb:143-144`) | [`Effects::active_named`](crate::Effects::active_named) with the long name |
//! | not overexerted | `Overexerted` not in `Debuffs` (`psms.rb:143`) | the same, for `Overexerted` |
//!
//! And the waivers: a weapon or shield `area_of_effect` technique is
//! affordable whatever it costs while `Glorious Momentum` is up
//! (`weapon.rb:236`, `shield.rb:307`), and a weapon technique ignores its
//! cooldown while `Glorious Momentum` is up for an `area_of_effect` one, or
//! `Ardor of the Scourge` for an `assault` (`weapon.rb:257-262`).
//!
//! # What is not answered
//!
//! - **Warcries' availability.** [`warcry_cost`] gives the cost, but Lich's
//!   `Warcry.available?` also asks `Status.cutthroat?` and `silenced?`
//!   (`warcry.rb:139-145`), and the known warcries
//!   ([`Standing::warcries`](super::super::standing::Standing::warcries)) are a
//!   set with no "never stated" of their own. The hunt's warcry gate already
//!   follows bigshot's (`cmd_warrior_shouts`), whose thresholds are not
//!   Lich's: `shout` 25 against 20, `holler` 31 against 20, `bellow` 11
//!   single and 21 for all against 20 (`bigshot.lic:5373-5381`,
//!   `warcry.rb:27-64`, where Lich notes "only 10 for single").
//! - **Forced roundtime.** `forcert_count` raises the cost by
//!   `25 + 10 x count` percent (`psms.rb:115-116`); bigshot never passes one.
//! - **`CMan.available?(ignore_cooldown:)`** for Burst and Surge
//!   (`cman.rb:751-759`), which a caller asks for explicitly; bigshot does not.

use crate::state::GameState;
use crate::state::character::vocabulary::{PsmCategory, Warcry};

const PSM_COSTS_TSV: &str = include_str!("../../../../data/psm_costs.tsv");

/// Which gauge a PSM spends: the key of Lich's `:cost` hash.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Gauge {
    /// Stamina: every row but one.
    Stamina,
    /// Mana: Excoriate.
    Mana,
}

impl Gauge {
    /// The `<progressBar>` id that states it.
    #[must_use]
    pub const fn bar(self) -> &'static str {
        match self {
            Self::Stamina => "stamina",
            Self::Mana => "mana",
        }
    }

    fn parse(text: &str) -> Option<Self> {
        match text {
            "stamina" => Some(Self::Stamina),
            "mana" => Some(Self::Mana),
            _ => None,
        }
    }
}

/// One row of Lich's cost tables.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PsmCost {
    /// Lich's long name, e.g. `coup_de_grace`: what the Cooldowns dialog
    /// names it, once folded ([`Effects::active_named`](crate::Effects::active_named)).
    pub long_name: &'static str,
    /// Lich's `:type`, e.g. `attack`, `area_of_effect`, `assault`, `passive`.
    pub kind: &'static str,
    /// The gauge it spends.
    pub gauge: Gauge,
    /// What it costs.
    pub amount: u16,
    /// What it costs while its own cooldown is up, when that differs: Burst
    /// of Swiftness and Surge of Strength, 60.
    pub while_cooling: Option<u16>,
    /// The line of Lich's table it was cut from, e.g.
    /// `lich-5/lib/gemstone/psms/cman.rb:106`.
    pub source: &'static str,
}

/// Every row: the table's category word (`cman`, ..., `warcry`), the
/// mnemonic, and the cost.
fn rows() -> impl Iterator<Item = (&'static str, &'static str, PsmCost)> {
    PSM_COSTS_TSV
        .lines()
        .filter(|line| !line.starts_with('#') && !line.starts_with("category\t"))
        .filter_map(read_row)
}

fn read_row(line: &'static str) -> Option<(&'static str, &'static str, PsmCost)> {
    let mut field = line.split('\t');
    let category = field.next()?;
    let mnemonic = field.next()?;
    let long_name = field.next()?;
    let kind = field.next()?;
    let gauge = Gauge::parse(field.next()?)?;
    let amount = field.next()?.parse().ok()?;
    let while_cooling = match field.next()? {
        "" => None,
        hot => Some(hot.parse().ok()?),
    };
    let source = field.next()?;
    Some((
        category,
        mnemonic,
        PsmCost {
            long_name,
            kind,
            gauge,
            amount,
            while_cooling,
            source,
        },
    ))
}

/// What one PSM costs, by category and mnemonic (`cman`, `coupdegrace`).
///
/// `None`: Lich's table does not list it. The PSM list itself is an open set
/// (`psm.rs`), so a mnemonic the game prints can be missing here.
#[must_use]
pub fn psm_cost(category: PsmCategory, mnemonic: &str) -> Option<PsmCost> {
    row(category.as_str(), mnemonic)
}

/// What one warcry costs (`warcry.rb:22-68`).
///
/// `Option` only because the table is data: all six are in it, which
/// `tests/psm_cost.rs` asserts.
#[must_use]
pub fn warcry_cost(cry: Warcry) -> Option<PsmCost> {
    row("warcry", cry.short_name())
}

/// Every row, in the table's order.
pub fn every_cost() -> impl Iterator<Item = (&'static str, &'static str, PsmCost)> {
    rows()
}

fn row(category: &str, mnemonic: &str) -> Option<PsmCost> {
    rows()
        .find(|(c, m, _)| *c == category && *m == mnemonic)
        .map(|(_, _, cost)| cost)
}

/// Lich's four tests for one PSM, each answered on its own
/// ([`GameState::psm_availability`]).
///
/// Every `None` is **not known**, never "no": the list unread, the bar
/// stating no points, the dialog never stated, or the clock not yet set.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PsmAvailability {
    /// Lich's table row, or `None` when the table does not list it.
    pub cost: Option<PsmCost>,
    /// Trained, ranks `>= 1`. `None`: this category's list has not been read.
    pub known: Option<bool>,
    /// The gauge's current points exceed the cost. `None`: no row, or the bar
    /// has not stated its points.
    pub affordable: Option<bool>,
    /// Listed and live in the Cooldowns dialog.
    pub cooling: Option<bool>,
    /// A weapon waiver lets it be used while cooling (`weapon.rb:257-262`).
    pub cooldown_waived: bool,
    /// `Overexerted` is live in the Debuffs dialog.
    pub overexerted: Option<bool>,
}

impl PsmAvailability {
    /// Lich's `available?`: known, affordable, not cooling unless waived, and
    /// not overexerted.
    ///
    /// `Some(false)` as soon as any test is known to fail, whatever the
    /// others; `Some(true)` only when all four are known to pass.
    #[must_use]
    pub fn available(&self) -> Option<bool> {
        let cooling = if self.cooldown_waived {
            Some(false)
        } else {
            self.cooling
        };
        let passes = [
            self.known,
            self.affordable,
            cooling.map(|c| !c),
            self.overexerted.map(|o| !o),
        ];
        if passes.contains(&Some(false)) {
            Some(false)
        } else if passes.contains(&None) {
            None
        } else {
            Some(true)
        }
    }
}

impl GameState {
    /// Lich's `available?` for one PSM, test by test.
    ///
    /// The effects are read at [`GameState::game_time_now`]; before the first
    /// prompt teaches the clock, `cooling` and `overexerted` are `None`.
    #[must_use]
    pub fn psm_availability(&self, category: PsmCategory, mnemonic: &str) -> PsmAvailability {
        let cost = psm_cost(category, mnemonic);
        let psms = &self.character.psms;
        let known = match psms.get(category, mnemonic) {
            Some(ranks) => Some(ranks.ranks >= 1),
            None => psms.has_table(category).then_some(false),
        };
        let now = self.game_time_now();
        let named = |dialog: &str, name: &str| {
            now.and_then(|now| self.effects.active_named(dialog, name, now))
        };
        let cooling = cost.and_then(|c| named("Cooldowns", c.long_name));
        let overexerted = named("Debuffs", "Overexerted");
        let momentum = named("Buffs", "Glorious Momentum") == Some(true);
        let ardor = named("Buffs", "Ardor of the Scourge") == Some(true);
        let area = cost.is_some_and(|c| c.kind == "area_of_effect");
        let assault = cost.is_some_and(|c| c.kind == "assault");
        let waived_cost =
            area && momentum && matches!(category, PsmCategory::Weapon | PsmCategory::Shield);
        let cooldown_waived =
            category == PsmCategory::Weapon && ((area && momentum) || (assault && ardor));
        let affordable = if waived_cost {
            Some(true)
        } else {
            cost.and_then(|c| self.affords(c, cooling))
        };
        PsmAvailability {
            cost,
            known,
            affordable,
            cooling,
            cooldown_waived,
            overexerted,
        }
    }

    /// Whether the gauge's current points exceed the cost: `psms.rb:118`,
    /// `cost_amount < XMLData.<gauge>`.
    ///
    /// A cost that doubles while cooling, asked with the cooldown unknown,
    /// answers only when both costs give the same answer.
    fn affords(&self, cost: PsmCost, cooling: Option<bool>) -> Option<bool> {
        let current = self.vital(cost.gauge.bar())?.current?;
        let covers = |amount: u16| i32::from(amount) < current;
        match (cost.while_cooling, cooling) {
            (None, _) | (Some(_), Some(false)) => Some(covers(cost.amount)),
            (Some(hot), Some(true)) => Some(covers(hot)),
            (Some(hot), None) => {
                let cold = covers(cost.amount);
                (cold == covers(hot)).then_some(cold)
            }
        }
    }
}
