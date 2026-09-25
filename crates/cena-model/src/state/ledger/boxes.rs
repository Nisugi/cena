//! Boxes: coins gathered from one, the locksmith pool's quote, drop and
//! return.
//!
//! `loottracker.lic`'s `BoxProcessor` and `LocksmithProcessor`. The box's
//! **contents** are not here: they arrive as `<inv id=>` frames, which the
//! model's inventory holds, and the ledger reads the box there when it records
//! the opening (`plan/34` §2).

use std::sync::OnceLock;

use super::text::{Pat, group_silvers, item, named, objects};
use super::{Cursor, LootFact};
use crate::state::chunks::ChunkLine;

struct Patterns {
    gathered: Pat,
    charm: Pat,
    charm_pile: Pat,
    returned: Pat,
    quote: Pat,
    dropped: Pat,
}

fn patterns() -> &'static Patterns {
    static P: OnceLock<Patterns> = OnceLock::new();
    P.get_or_init(|| Patterns {
        gathered: Pat::new(r"^You gather the remaining ([\d,]+) coins from inside (?:your|an?) (.+)\.$"),
        charm: Pat::new(r"^You summon a swarm of .+ from your .+ reclaiming them"),
        charm_pile: Pat::new(r"locate a pile of ([\d,]+) coins, reclaiming them"),
        returned: Pat::new(r#"says, "Alright, here's your (.+) back\.""#),
        quote: Pat::new(
            r"^You want a locksmith to open an? (.+) for a tip of ([\d,]+) silvers?.+fee of ([\d,]+) silvers? due up front",
        ),
        dropped: Pat::new(
            r#"takes your (\w+) and says, "Your tip of ([\d,]+) silvers? has been recorded, and the ([\d,]+) silvers? fee has been collected\."#,
        ),
    })
}

/// Read one line. `true` when it was a box line.
pub(super) fn read(cursor: &mut Cursor, line: &ChunkLine, text: &str) -> bool {
    let p = patterns();

    if let Some(caps) = p.gathered.captures(text) {
        // `<coins> from inside your <box>`: the box is the object the text
        // names after `inside`, or failing an exact match the last one.
        let box_ = caps
            .get(2)
            .and_then(|m| named(line, m.as_str()))
            .or_else(|| objects(line).pop().map(|(o, _)| o));
        if let (Some(item), Some(silvers)) = (box_, group_silvers(&caps, 1)) {
            cursor.out.push(LootFact::BoxOpened { item, silvers });
        }
        return true;
    }

    if p.charm.is_match(text) {
        // `from your <charm> … inside an <box> … pile of N <coins>`: the box
        // is the second object that is not the coins.
        let box_ = objects(line)
            .into_iter()
            .map(|(o, _)| o)
            .filter(|o| o.noun != "coins")
            .nth(1);
        let silvers = p
            .charm_pile
            .captures(text)
            .and_then(|c| group_silvers(&c, 1));
        if let (Some(item), Some(silvers)) = (box_, silvers) {
            cursor.out.push(LootFact::BoxOpened { item, silvers });
        }
        return true;
    }

    if let Some(caps) = p.returned.captures(text) {
        let box_ = caps
            .get(1)
            .and_then(|m| named(line, m.as_str()))
            .or_else(|| item(line));
        if let Some(item) = box_ {
            cursor.out.push(LootFact::BoxReturned { item });
        }
        return true;
    }

    if let Some(caps) = p.quote.captures(text) {
        if let (Some(item), Some(tip), Some(fee)) =
            (item(line), group_silvers(&caps, 2), group_silvers(&caps, 3))
        {
            cursor.out.push(LootFact::PoolQuoted { item, tip, fee });
        }
        return true;
    }

    if let Some(caps) = p.dropped.captures(text) {
        if let (Some(noun), Some(tip), Some(fee)) = (
            caps.get(1),
            group_silvers(&caps, 2),
            group_silvers(&caps, 3),
        ) {
            cursor.out.push(LootFact::PoolDropped {
                noun: noun.as_str().to_owned(),
                tip,
                fee,
            });
        }
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
            p.gathered.compiled(),
            p.charm.compiled(),
            p.charm_pile.compiled(),
            p.returned.compiled(),
            p.quote.compiled(),
            p.dropped.compiled(),
        ];
        assert!(
            all.iter().all(|c| *c),
            "a literal failed to compile: {all:?}"
        );
    }
}
