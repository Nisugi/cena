//! Town: appraisals, sales, the bank, and a gem lost to a song.
//!
//! `loottracker.lic`'s `AppraisalProcessor`, `LoresongProcessor`,
//! `ShopAppraisalProcessor`, `SellProcessor`, `GemshopProcessor`,
//! `FurrierProcessor`, `ChronomageProcessor` and `BankProcessor`.
//!
//! Several of these are two lines: an offer or a look, then a figure. Within
//! one chunk the pair is made here through the cursor's `pending`; across
//! chunks the ledger pairs the halves, each reported with its missing side
//! `None`.

use std::sync::OnceLock;

use super::text::{Pat, group_silvers, item, item_at, named};
use super::{Appraiser, Buyer, Cursor, LootFact, Pending};
use crate::state::chunks::ChunkLine;

struct Patterns {
    gem_appraise: Pat,
    skin_appraise: Pat,
    bundle_value: Pat,
    loresong_start: Pat,
    loresong_value: Pat,
    shatter: Pat,
    offer: Pat,
    ask: Pat,
    pawn_paid: Pat,
    pawn_note: Pat,
    worthless: Pat,
    junk: Pat,
    shop_looking: Pat,
    shop_offer: Pat,
    jeweler_offer: Pat,
    shop_worth: Pat,
    gemshop_paid: Pat,
    gemshop_note: Pat,
    gemshop_bulk: Pat,
    gemshop_bulk_chit: Pat,
    gemshop_reject: Pat,
    chronomage: Pat,
    furrier_paid: Pat,
    furrier_bulk: Pat,
    deposit: Pat,
    withdraw: Pat,
    note_deposit: Pat,
}

fn re(pattern: &str) -> Pat {
    Pat::new(pattern)
}

fn patterns() -> &'static Patterns {
    static P: OnceLock<Patterns> = OnceLock::new();
    P.get_or_init(|| Patterns {
        gem_appraise: re(
            r"^You peer intently at the .+ as you turn it in your fingers.+worth approximately ([\d,]+) silvers[.!]",
        ),
        skin_appraise: re(
            r"^You turn the .+ over in your hands, meticulously inspecting for flaws\..+ is of \w+ quality and worth approximately ([\d,]+) silvers[.!]",
        ),
        bundle_value: re(r"^You estimate that the total value of your .+ is approximately ([\d,]+) silvers\."),
        loresong_start: re(r"As you sing, you feel a faint resonating vibration from the .+ in your hand"),
        loresong_value: re(r"it's worth about ([\d,]+) silvers"),
        shatter: re(r"Your focused voice causes the .+ to shatter"),
        offer: re(r"^You offer to sell your .+ to "),
        ask: re(r"^You ask \S+ (?:if \S+ would like to buy|to appraise) an? .+\.$"),
        pawn_paid: re(r"takes your .+, glances at it briefly, then hands you ([\d,]+) silver coins\."),
        pawn_note: re(r"scribbles out an? .+ for ([\d,]+) silvers? and hands it to you\."),
        worthless: re(r#"says, "That's basically worthless here,"#),
        junk: re(r"Where do you find this junk\?"),
        shop_looking: re(r"turns the .+ over in (?:his|her) hands"),
        shop_offer: re(r"I(?:'ll| will) (?:give|offer) you ([\d,]+) silver"),
        jeweler_offer: re(
            r#"takes the .+ and inspects it carefully before saying, "I'll give you ([\d,]+) silvers? for it"#,
        ),
        shop_worth: re(r"That .+ looks decent, probably worth about ([\d,]+) silvers"),
        gemshop_paid: re(r"takes the .+, gives it a careful examination and hands you ([\d,]+) silver for it\."),
        gemshop_note: re(
            r"takes the .+, gives it a careful examination and.+hands you an? (.+) for ([\d,]+) silvers?\.",
        ),
        gemshop_bulk: re(
            r"takes the .+, inspects the contents carefully, and removes the gems.+hands it back to you, along with ([\d,]+) silver\.",
        ),
        gemshop_bulk_chit: re(r"removes the gems and hands you an? .+ for ([\d,]+) silvers?\."),
        gemshop_reject: re(r#"says, "Sorry,.+I'm not buying anything this valuable today"#),
        chronomage: re(
            r"gleefully snatches an? .+ from your outreached hand.+I'll charge you ([\d,]+) silvers less",
        ),
        furrier_paid: re(
            r"^\w+ takes the .+, (?:scrutinizes it carefully|appraises it minutely), then (?:hands|pays) you ([\d,]+) silvers?\.",
        ),
        furrier_bulk: re(
            r"takes the .+, inspects the contents carefully and removes the items?.+hands it back to you, along with ([\d,]+) silver\.",
        ),
        deposit: re(r"^You deposit ([\d,]+) silvers? into your account\."),
        withdraw: re(r"teller carefully records the transaction,? (?:and )?hands you ([\d,]+) silvers?"),
        note_deposit: re(r"That's a total of ([\d,]+) silvers, bringing your balance to"),
    })
}

/// Read one line. `true` when it was a town line.
pub(super) fn read(cursor: &mut Cursor, line: &ChunkLine, text: &str) -> bool {
    appraisals(cursor, line, text) || sales(cursor, line, text) || bank(cursor, text)
}

fn appraisals(cursor: &mut Cursor, line: &ChunkLine, text: &str) -> bool {
    let p = patterns();
    let appraised = |item, value, by| LootFact::Appraised { item, value, by };

    if let Some(caps) = p.gem_appraise.captures(text) {
        cursor.out.push(appraised(
            item(line),
            group_silvers(&caps, 1),
            Appraiser::Gem,
        ));
        return true;
    }
    if let Some(caps) = p
        .skin_appraise
        .captures(text)
        .or_else(|| p.bundle_value.captures(text))
    {
        cursor.out.push(appraised(
            item(line),
            group_silvers(&caps, 1),
            Appraiser::Skin,
        ));
        return true;
    }
    if p.loresong_start.is_match(text) {
        if let Some(sung) = item(line) {
            cursor.pending = Some(Pending::Singing(sung));
        }
        return true;
    }
    if let Some(caps) = p.loresong_value.captures(text) {
        let sung = match cursor.pending.take() {
            Some(Pending::Singing(sung)) => Some(sung),
            other => {
                cursor.pending = other;
                None
            }
        };
        cursor.out.push(appraised(
            sung,
            group_silvers(&caps, 1),
            Appraiser::Loresong,
        ));
        return true;
    }
    if p.shatter.is_match(text) {
        if let Some(gem) = item(line) {
            cursor.out.push(LootFact::Shattered { item: gem });
        }
        return true;
    }
    // The shopkeeper's look and the figure that follows. The jeweler and the
    // `probably worth about` form put both on one line, and are checked
    // before the bare figure so the item is not lost.
    if let Some(caps) = p
        .jeweler_offer
        .captures(text)
        .or_else(|| p.shop_worth.captures(text))
    {
        cursor.out.push(appraised(
            item(line),
            group_silvers(&caps, 1),
            Appraiser::Shop,
        ));
        return true;
    }
    if p.shop_looking.is_match(text) {
        if let Some(looked) = item(line) {
            cursor.pending = Some(Pending::ShopLooking(looked));
        }
        return true;
    }
    if let Some(caps) = p.shop_offer.captures(text) {
        let looked = match cursor.pending.take() {
            Some(Pending::ShopLooking(looked)) => Some(looked),
            other => {
                cursor.pending = other;
                None
            }
        };
        cursor
            .out
            .push(appraised(looked, group_silvers(&caps, 1), Appraiser::Shop));
        return true;
    }
    false
}

fn sales(cursor: &mut Cursor, line: &ChunkLine, text: &str) -> bool {
    let p = patterns();
    let sold = |item, silvers, to, note| LootFact::Sold {
        item,
        silvers,
        to,
        note,
    };

    if p.offer.is_match(text) || p.ask.is_match(text) {
        if let Some(offered) = item(line) {
            cursor.pending = Some(Pending::Offered(offered));
        }
        return true;
    }
    if let Some(caps) = p.pawn_paid.captures(text) {
        if let Some(silvers) = group_silvers(&caps, 1) {
            cursor
                .out
                .push(sold(item(line), silvers, Buyer::Pawn, None));
            cursor.take_offered();
        }
        return true;
    }
    if let Some(caps) = p.pawn_note.captures(text) {
        if let Some(silvers) = group_silvers(&caps, 1) {
            let offered = cursor.take_offered();
            cursor
                .out
                .push(sold(offered, silvers, Buyer::Pawn, item(line)));
        }
        return true;
    }
    if p.worthless.is_match(text) || p.junk.is_match(text) {
        let refused = item(line).or_else(|| cursor.take_offered());
        cursor.out.push(LootFact::Worthless { item: refused });
        return true;
    }
    if let Some(caps) = p
        .gemshop_paid
        .captures(text)
        .or_else(|| p.gemshop_bulk.captures(text))
    {
        if let Some(silvers) = group_silvers(&caps, 1) {
            cursor
                .out
                .push(sold(item(line), silvers, Buyer::Gemshop, None));
        }
        return true;
    }
    if let Some(caps) = p.gemshop_note.captures(text) {
        if let Some(silvers) = group_silvers(&caps, 2) {
            let note = caps
                .get(1)
                .and_then(|m| named(line, m.as_str()))
                .or_else(|| item_at(line, 1));
            cursor
                .out
                .push(sold(item(line), silvers, Buyer::Gemshop, note));
        }
        return true;
    }
    if let Some(caps) = p.gemshop_bulk_chit.captures(text) {
        if let Some(silvers) = group_silvers(&caps, 1) {
            cursor
                .out
                .push(sold(None, silvers, Buyer::Gemshop, item(line)));
        }
        return true;
    }
    if p.gemshop_reject.is_match(text) {
        let asked = cursor.take_offered();
        cursor.out.push(LootFact::TooValuable { item: asked });
        return true;
    }
    if let Some(caps) = p.chronomage.captures(text) {
        if let Some(silvers) = group_silvers(&caps, 1) {
            cursor
                .out
                .push(sold(item(line), silvers, Buyer::Chronomage, None));
        }
        return true;
    }
    if let Some(caps) = p
        .furrier_paid
        .captures(text)
        .or_else(|| p.furrier_bulk.captures(text))
    {
        if let Some(silvers) = group_silvers(&caps, 1) {
            cursor
                .out
                .push(sold(item(line), silvers, Buyer::Furrier, None));
        }
        return true;
    }
    false
}

fn bank(cursor: &mut Cursor, text: &str) -> bool {
    let p = patterns();
    let figure = |caps: regex::Captures<'_>| group_silvers(&caps, 1);
    if let Some(silvers) = p.deposit.captures(text).and_then(figure) {
        cursor.out.push(LootFact::Deposited(silvers));
        return true;
    }
    if let Some(silvers) = p.withdraw.captures(text).and_then(figure) {
        cursor.out.push(LootFact::Withdrew(silvers));
        return true;
    }
    if let Some(silvers) = p.note_deposit.captures(text).and_then(figure) {
        cursor.out.push(LootFact::NoteDeposited(silvers));
        return true;
    }
    false
}

#[cfg(test)]
mod tests {
    #[test]
    fn every_pattern_compiled() {
        let p = super::patterns();
        let all = [
            p.gem_appraise.compiled(),
            p.skin_appraise.compiled(),
            p.bundle_value.compiled(),
            p.loresong_start.compiled(),
            p.loresong_value.compiled(),
            p.shatter.compiled(),
            p.offer.compiled(),
            p.ask.compiled(),
            p.pawn_paid.compiled(),
            p.pawn_note.compiled(),
            p.worthless.compiled(),
            p.junk.compiled(),
            p.shop_looking.compiled(),
            p.shop_offer.compiled(),
            p.jeweler_offer.compiled(),
            p.shop_worth.compiled(),
            p.gemshop_paid.compiled(),
            p.gemshop_note.compiled(),
            p.gemshop_bulk.compiled(),
            p.gemshop_bulk_chit.compiled(),
            p.gemshop_reject.compiled(),
            p.chronomage.compiled(),
            p.furrier_paid.compiled(),
            p.furrier_bulk.compiled(),
            p.deposit.compiled(),
            p.withdraw.compiled(),
            p.note_deposit.compiled(),
        ];
        assert!(
            all.iter().all(|c| *c),
            "a literal failed to compile: {all:?}"
        );
    }
}
