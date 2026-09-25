//! The hunt's loot lines: searching, skinning, bundling, the bounty reward and
//! a wand duplicated in the field.
//!
//! `loottracker.lic`'s `SearchProcessor` (with `KlockProcessor` and
//! `SpecialFindProcessor` folded in), `SkinProcessor`, `BundleProcessor`,
//! `BountyProcessor` and `WandDupeProcessor`.

use std::sync::OnceLock;

use super::text::{Pat, creature, group_silvers, item, item_at, objects};
use super::{Cursor, Find, LootFact, Pending};
use crate::state::chunks::ChunkLine;

struct Patterns {
    search: Pat,
    silver: Pat,
    /// `had`, `carried`, `left … behind`, `Interesting, … carried`, and the
    /// draconic idol's `rifling through … belongings, you find`: one shape,
    /// a bolded creature and an unbolded item.
    item: Pat,
    klock: Pat,
    dust: Pat,
    jewel: Pat,
    boost: Pat,
    skin: Pat,
    bundle_create: Pat,
    bundle_add: Pat,
    bundle_manual: Pat,
    bounty: Pat,
    gesture: Pat,
    duplicated: Pat,
}

fn patterns() -> &'static Patterns {
    static P: OnceLock<Patterns> = OnceLock::new();
    P.get_or_init(|| Patterns {
        search: Pat::new(r"^You search the .+\.$"),
        silver: Pat::new(r"^\S+ had ([\d,]+) silvers on "),
        item: Pat::new(
            r"^(?:Interesting, )?\S+ (?:had|carried|left) |^While rifling through .+ belongings, you find ",
        ),
        klock: Pat::new(r"^A .+ (?:key|lock) appears on the ground!$"),
        // `mote of <gem-word> dust`: the wire's word for the mote is the
        // game's own name, which Rule 3.4's test flags in code; the shape
        // around it identifies the line on its own.
        dust: Pat::new(
            r"You notice a scintillating mote of \w+ dust on the ground and gather it quickly\.",
        ),
        jewel: Pat::new(r"A glint of light catches your eye, and you notice an? .+ at your feet!"),
        boost: Pat::new(r"You have been awarded (\d+) Long-Term Experience Boost!"),
        skin: Pat::new(r"^You skinned the .+, yielding an? .+\.$"),
        bundle_create: Pat::new(
            r"^As you place your .+ inside your .+, you notice another .+ into a neat bundle\.$",
        ),
        bundle_add: Pat::new(r"^You carefully add your .+ to your bundle of .+ inside your .+\.$"),
        bundle_manual: Pat::new(r"^You carefully arrange your two .+ into a neat bundle\.$"),
        bounty: Pat::new(
            r"^\[You have earned ([\d,]+) bounty points?, ([\d,]+) experience points?, and ([\d,]+) silver\.\]$",
        ),
        gesture: Pat::new(r"^You gesture at an? .+\.$"),
        duplicated: Pat::new(
            r"The glow fades from both objects and they solidify, looking almost identical to the original",
        ),
    })
}

/// Read one line. `true` when it was a hunt line, whether or not it yielded
/// a fact, so the later readers are not offered it.
pub(super) fn read(cursor: &mut Cursor, line: &ChunkLine, text: &str) -> bool {
    let p = patterns();

    if p.search.is_match(text) {
        cursor.close_search();
        if let Some(creature) = creature(line) {
            cursor.search = Some(LootFact::Searched {
                creature,
                silvers: 0,
                items: Vec::new(),
                finds: Vec::new(),
            });
        }
        return true;
    }

    if search_result(cursor, line, text) || bundles(cursor, line, text) {
        return true;
    }

    if p.skin.is_match(text) {
        if let (Some(creature), Some(skin)) = (creature(line), item(line)) {
            cursor.out.push(LootFact::Skinned { creature, skin });
        }
        return true;
    }

    if let Some(caps) = p.bounty.captures(text) {
        if let (Some(points), Some(experience), Some(silvers)) = (
            group_silvers(&caps, 1),
            group_silvers(&caps, 2),
            group_silvers(&caps, 3),
        ) {
            cursor.out.push(LootFact::Bounty {
                points,
                experience,
                silvers,
            });
        }
        return true;
    }

    if p.gesture.is_match(text) {
        // Remembered, not reported: most gestures cast something else.
        if let Some(wand) = item(line) {
            cursor.pending = Some(Pending::Gestured(wand));
        }
        return true;
    }
    if p.duplicated.is_match(text) {
        let donor = match cursor.pending.take() {
            Some(Pending::Gestured(wand)) => Some(wand),
            other => {
                cursor.pending = other;
                None
            }
        };
        cursor.out.push(LootFact::WandDuplicated { donor });
        return true;
    }

    false
}

/// The lines under `You search the <creature>.`. They mean something only
/// under the search that produced them; on their own they are not loot facts.
fn search_result(cursor: &mut Cursor, line: &ChunkLine, text: &str) -> bool {
    let p = patterns();
    if let Some(LootFact::Searched {
        silvers,
        items,
        finds,
        ..
    }) = &mut cursor.search
    {
        if let Some(caps) = p.silver.captures(text) {
            *silvers = group_silvers(&caps, 1).unwrap_or(0);
            return true;
        }
        if p.item.is_match(text) && creature(line).is_some() {
            if let Some(found) = item(line) {
                items.push(found);
            }
            return true;
        }
        if p.klock.is_match(text) {
            if let Some(piece) = item(line).filter(|i| i.noun == "key" || i.noun == "lock") {
                finds.push(Find::Klock(piece));
            }
            return true;
        }
        if p.dust.is_match(text) {
            finds.push(Find::GemDust);
            return true;
        }
        if p.jewel.is_match(text) {
            // The whole line arrives bolded, the jewel's link with it, so
            // "the unbolded object" would find nothing.
            if let Some((jewel, _)) = objects(line).into_iter().next() {
                finds.push(Find::Jewel(jewel));
            }
            return true;
        }
        if let Some(caps) = p.boost.captures(text) {
            if let Some(count) = caps.get(1).and_then(|m| m.as_str().parse().ok()) {
                finds.push(Find::Boost(count));
            }
            return true;
        }
    }
    false
}

/// The three bundle lines.
fn bundles(cursor: &mut Cursor, line: &ChunkLine, text: &str) -> bool {
    let p = patterns();
    if p.bundle_create.is_match(text) {
        // `your <skin> inside your <container>, … another <skin> inside the
        // <container> … the two <skins> into a neat bundle`: the bundle is
        // the last object named.
        let all = objects(line);
        if let Some((bundle, _)) = all.last().cloned() {
            cursor.out.push(LootFact::Bundled {
                skin: item_at(line, 0),
                bundle,
                container: item_at(line, 1),
                created: true,
            });
        }
        return true;
    }
    if p.bundle_add.is_match(text) {
        if let Some(bundle) = item_at(line, 1) {
            cursor.out.push(LootFact::Bundled {
                skin: item_at(line, 0),
                bundle,
                container: item_at(line, 2),
                created: false,
            });
        }
        return true;
    }
    if p.bundle_manual.is_match(text) {
        if let Some(bundle) = item(line) {
            cursor.out.push(LootFact::Bundled {
                skin: None,
                bundle,
                container: None,
                created: true,
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
            p.search.compiled(),
            p.silver.compiled(),
            p.item.compiled(),
            p.klock.compiled(),
            p.dust.compiled(),
            p.jewel.compiled(),
            p.boost.compiled(),
            p.skin.compiled(),
            p.bundle_create.compiled(),
            p.bundle_add.compiled(),
            p.bundle_manual.compiled(),
            p.bounty.compiled(),
            p.gesture.compiled(),
            p.duplicated.compiled(),
        ];
        assert!(
            all.iter().all(|c| *c),
            "a literal failed to compile: {all:?}"
        );
    }
}
