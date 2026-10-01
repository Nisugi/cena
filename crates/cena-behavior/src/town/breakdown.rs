//! What a selling round came to (`plan/61` step 2): eloot's breakdown
//! (`Sell.breakdown`, `eloot.lic:6216-6300`), built from the ledger's facts
//! as the round hears them, with no `wealth` before and after.
//!
//! Silver by shop; the locksmith pool as what its boxes held less its tips
//! and fees, with how many boxes went in and came back; the bank; and what
//! was appraised and kept (`over_max_rows`, `:6174`). The total is the
//! sales and the boxes less the pool's cost. eloot's adds its *Pool Return*
//! count into the silver (`:6231`), which this does not.

use cena_session::{Buyer, LootFact};

/// One round's breakdown, fed each prompt's facts.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Breakdown {
    gemshop: u64,
    pawnshop: u64,
    furrier: u64,
    /// Credited by the Chronomage's clerk: travel, not silver.
    chronomage: u64,
    /// Tips and fees the pool took.
    pool_paid: u64,
    pool_dropped: u32,
    pool_returned: u32,
    /// Coins gathered from opened boxes.
    box_silver: u64,
    deposited: u64,
    withdrew: u64,
    notes: u64,
    /// Appraised and not sold since: the id, the name, the figure (zero
    /// when the shop only said it was too valuable).
    kept: Vec<(String, String, u64)>,
}

impl Breakdown {
    /// Take one prompt's facts.
    pub fn take(&mut self, facts: &[LootFact]) {
        for fact in facts {
            match fact {
                LootFact::Sold {
                    item, silvers, to, ..
                } => {
                    *match to {
                        Buyer::Gemshop => &mut self.gemshop,
                        Buyer::Pawn => &mut self.pawnshop,
                        Buyer::Furrier => &mut self.furrier,
                        Buyer::Chronomage => &mut self.chronomage,
                    } += silvers;
                    if let Some(item) = item {
                        self.kept.retain(|(id, _, _)| *id != item.id);
                    }
                }
                LootFact::Appraised {
                    item: Some(item),
                    value: Some(value),
                    ..
                } => self.keep(&item.id, &item.text, *value),
                LootFact::TooValuable { item: Some(item) } => self.keep(&item.id, &item.text, 0),
                LootFact::PoolDropped { tip, fee, .. } => {
                    self.pool_paid += tip + fee;
                    self.pool_dropped += 1;
                }
                LootFact::BoxReturned { .. } => self.pool_returned += 1,
                LootFact::BoxOpened { silvers, .. } => self.box_silver += silvers,
                LootFact::Deposited(silvers) => self.deposited += silvers,
                LootFact::Withdrew(silvers) => self.withdrew += silvers,
                LootFact::NoteDeposited(silvers) => self.notes += silvers,
                _ => {}
            }
        }
    }

    /// An item valued and still held; a later figure for it replaces an
    /// earlier one unless it is the bare *too valuable*.
    fn keep(&mut self, id: &str, name: &str, value: u64) {
        match self.kept.iter_mut().find(|(kept, _, _)| kept == id) {
            Some(entry) if value > 0 => entry.2 = value,
            Some(_) => {}
            None => self.kept.push((id.to_owned(), name.to_owned(), value)),
        }
    }

    /// What the round came to: the sales and the boxes, less the pool's
    /// tips and fees.
    #[must_use]
    pub fn total(&self) -> i128 {
        i128::from(self.gemshop)
            + i128::from(self.pawnshop)
            + i128::from(self.furrier)
            + i128::from(self.box_silver)
            - i128::from(self.pool_paid)
    }

    /// The breakdown as lines for the player, `skipped` being how many
    /// items the round gave up on; empty when the round did nothing to say.
    #[must_use]
    pub fn lines(&self, skipped: usize) -> Vec<String> {
        let row = |label: &str, silver: i128| format!("  {label:<16}{:>12}", grouped(silver));
        let mut lines = Vec::new();
        for (label, silver) in [
            ("Gem shop", self.gemshop),
            ("Pawnshop", self.pawnshop),
            ("Furrier", self.furrier),
        ] {
            if silver > 0 {
                lines.push(row(label, i128::from(silver)));
            }
        }
        let pool = self.pool_dropped > 0 || self.pool_returned > 0 || self.pool_paid > 0;
        if pool {
            let net = i128::from(self.box_silver) - i128::from(self.pool_paid);
            lines.push(row("Locksmith pool", net));
            lines.push(format!(
                "    {} given, {} back; {} in tips and fees, {} from the boxes",
                self.pool_dropped,
                self.pool_returned,
                grouped(i128::from(self.pool_paid)),
                grouped(i128::from(self.box_silver)),
            ));
        } else if self.box_silver > 0 {
            lines.push(row("Boxes", i128::from(self.box_silver)));
        }
        if !lines.is_empty() {
            lines.push(row("Total", self.total()));
        }
        if self.chronomage > 0 {
            lines.push(format!(
                "  The Chronomage credited {} for gold rings.",
                grouped(i128::from(self.chronomage))
            ));
        }
        let mut bank = Vec::new();
        for (word, silver) in [
            ("deposited", self.deposited),
            ("notes", self.notes),
            ("withdrew", self.withdrew),
        ] {
            if silver > 0 {
                bank.push(format!("{word} {}", grouped(i128::from(silver))));
            }
        }
        if !bank.is_empty() {
            lines.push(format!("  Bank: {}.", bank.join(", ")));
        }
        if !self.kept.is_empty() {
            let kept: Vec<String> = self
                .kept
                .iter()
                .map(|(_, name, value)| match value {
                    0 => format!("{name} (too valuable for the shop)"),
                    value => format!("{name} ({})", grouped(i128::from(*value))),
                })
                .collect();
            lines.push(format!("  Appraised and kept: {}.", kept.join(", ")));
        }
        if skipped > 0 {
            lines.push(format!("  {skipped} items could not be sold."));
        }
        if !lines.is_empty() {
            lines.insert(0, "Loot: the round came to".to_owned());
        }
        lines
    }
}

/// A figure with its thousands grouped: `12,340`, `-1,500`.
fn grouped(silver: i128) -> String {
    let digits = silver.unsigned_abs().to_string();
    let mut out = String::with_capacity(digits.len() + digits.len() / 3 + 1);
    for (at, digit) in digits.chars().enumerate() {
        if at > 0 && (digits.len() - at).is_multiple_of(3) {
            out.push(',');
        }
        out.push(digit);
    }
    if silver < 0 {
        out.insert(0, '-');
    }
    out
}

#[cfg(test)]
mod tests {
    use cena_session::Appraiser;
    use cena_session::containers::ItemRef;

    use super::*;

    fn item(id: &str, text: &str) -> ItemRef {
        ItemRef {
            id: id.to_owned(),
            noun: text.rsplit(' ').next().unwrap_or(text).to_owned(),
            text: text.to_owned(),
        }
    }

    fn sold(id: Option<&str>, silvers: u64, to: Buyer) -> LootFact {
        LootFact::Sold {
            item: id.map(|id| item(id, "thing")),
            silvers,
            to,
            note: None,
        }
    }

    #[test]
    fn figures_are_grouped() {
        assert_eq!(grouped(0), "0");
        assert_eq!(grouped(999), "999");
        assert_eq!(grouped(12_340), "12,340");
        assert_eq!(grouped(1_234_567), "1,234,567");
        assert_eq!(grouped(-1_500), "-1,500");
    }

    /// The shops by name, the pool as what its boxes held less what it
    /// took, the total, the bank, and what was appraised and kept.
    #[test]
    fn a_round_is_added_up_by_shop_with_the_pool_net() {
        let mut round = Breakdown::default();
        round.take(&[
            sold(None, 12_000, Buyer::Gemshop),
            sold(Some("5"), 340, Buyer::Gemshop),
            sold(Some("6"), 4_100, Buyer::Pawn),
        ]);
        round.take(&[
            LootFact::PoolDropped {
                noun: "coffer".to_owned(),
                tip: 1_000,
                fee: 500,
            },
            LootFact::BoxReturned {
                item: item("9", "iron strongbox"),
            },
            LootFact::BoxOpened {
                item: item("9", "iron strongbox"),
                silvers: 2_345,
            },
            LootFact::Deposited(18_000),
            LootFact::Withdrew(500),
        ]);
        assert_eq!(round.total(), 12_340 + 4_100 + 2_345 - 1_500);
        let lines = round.lines(2);
        let has = |text: &str| lines.iter().any(|line| line.contains(text));
        assert_eq!(lines[0], "Loot: the round came to");
        assert!(has("Gem shop") && has("12,340"), "{lines:?}");
        assert!(has("Pawnshop") && has("4,100"), "{lines:?}");
        assert!(!has("Furrier"), "a shop that bought nothing is not named");
        assert!(has("Locksmith pool") && has("845"), "{lines:?}");
        assert!(has(
            "1 given, 1 back; 1,500 in tips and fees, 2,345 from the boxes"
        ));
        assert!(has("Total") && has("17,285"), "{lines:?}");
        assert!(has("Bank: deposited 18,000, withdrew 500."), "{lines:?}");
        assert!(has("2 items could not be sold."), "{lines:?}");
    }

    /// Appraised and sold is a sale; appraised and not sold is kept, with
    /// its figure; the gem shop's *too valuable* alone is kept without one.
    #[test]
    fn what_was_appraised_and_not_sold_is_named() {
        let appraised = |id: &str, name: &str, value| LootFact::Appraised {
            item: Some(item(id, name)),
            value: Some(value),
            by: Appraiser::Shop,
        };
        let mut round = Breakdown::default();
        round.take(&[
            appraised("5", "a blue sapphire", 45_000),
            appraised("6", "a small ruby", 900),
            LootFact::TooValuable {
                item: Some(item("7", "a star emerald")),
            },
        ]);
        round.take(&[sold(Some("6"), 900, Buyer::Gemshop)]);
        let lines = round.lines(0);
        assert!(
            lines.iter().any(|line| line
                == "  Appraised and kept: a blue sapphire (45,000), a star emerald (too valuable for the shop)."),
            "{lines:?}"
        );
    }

    #[test]
    fn a_round_that_did_nothing_says_nothing() {
        assert!(Breakdown::default().lines(0).is_empty());
    }
}
