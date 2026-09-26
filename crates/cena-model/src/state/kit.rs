//! A Survivalist's Kit, as `analyze` and `look in` describe it (`plan/36`
//! Stage 3): eherbs' `determine_survival_kit`, `survival_contents` and
//! `distill` (`eherbs.lic:2981-3076`, `:2469-2518`), read from the chunk.
//!
//! A kit is not an ordinary container. It holds herbs as doses it counts,
//! listed in one line per form -- *The kit contains DOSEs acantha leaf (12),
//! basal moss (3).* and *contains TINCTUREs ...* -- each herb a link with its
//! own id, which is what `get` takes. `analyze` says whether a container is a
//! kit at all, its capacity tier out of five, whether it has the Liquid
//! Extractor, and what the extractor is working on.

use std::collections::BTreeMap;

use super::chunks::{Chunk, ChunkLine};
use super::containers::ItemRef;

/// One herb a kit holds, with its count.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct KitHerb {
    /// The herb, with the id `get` takes.
    pub item: ItemRef,
    /// Doses of it.
    pub count: u32,
    /// Listed as a TINCTURE: drunk, not eaten.
    pub liquid: bool,
}

/// What `analyze` said of a container.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct KitAnalysis {
    /// It is a Survivalist's Kit.
    pub is_kit: bool,
    /// `Capacity: N/5`.
    pub tier: Option<u32>,
    /// It has the Liquid Extractor unlock.
    pub extractor: bool,
    /// What the extractor is working on, when it is.
    pub distilling: Option<String>,
}

/// Kits by container id: what each last analyzed as and last listed.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Kits {
    analyses: BTreeMap<String, KitAnalysis>,
    listings: BTreeMap<String, Vec<KitHerb>>,
}

impl Kits {
    /// What `analyze` last said of this container.
    #[must_use]
    pub fn analysis(&self, id: &str) -> Option<&KitAnalysis> {
        self.analyses.get(id)
    }

    /// The kit's herbs as last listed; `None` when never listed.
    #[must_use]
    pub fn listing(&self, id: &str) -> Option<&[KitHerb]> {
        self.listings.get(id).map(Vec::as_slice)
    }

    /// Read one chunk.
    pub(crate) fn read_chunk(&mut self, chunk: &Chunk) {
        let mut analyzing: Option<String> = None;
        // A listing is restated whole: both forms arrive in the same answer,
        // so a kit's list is replaced, not merged, the first time it is seen
        // in a chunk.
        let mut listed: Vec<String> = Vec::new();
        for line in chunk.lines() {
            let text = line.text();
            let text = text.trim();
            if text.starts_with("You analyze") {
                analyzing = first_id(line);
                if let Some(id) = &analyzing {
                    self.analyses.insert(id.clone(), KitAnalysis::default());
                }
                continue;
            }
            if let Some((liquid, at)) = form(text) {
                let Some(kit) = first_id(line) else { continue };
                if !listed.contains(&kit) {
                    listed.push(kit.clone());
                    self.listings.insert(kit.clone(), Vec::new());
                }
                let herbs = herbs_on(line, &text[at..], liquid);
                self.listings.entry(kit).or_default().extend(herbs);
                continue;
            }
            let Some(analysis) = analyzing.as_ref().and_then(|id| self.analyses.get_mut(id)) else {
                continue;
            };
            if text.contains("is a Survivalist's Kit, which is a specialized container") {
                analysis.is_kit = true;
            }
            if let Some(rest) = text.split("Capacity: ").nth(1)
                && let Some((tier, _)) = rest.split_once("/5")
            {
                analysis.tier = tier.trim().parse().ok();
            }
            if text.contains("has the Liquid Extractor unlock") {
                analysis.extractor = true;
            }
            if let Some(rest) = text.split("The extractor is currently targeting ").nth(1) {
                analysis.distilling = rest
                    .split(", with around")
                    .next()
                    .map(|what| what.trim().to_owned());
            }
        }
    }
}

/// Where the herbs begin, and whether they are liquid: after `contains
/// DOSEs` or `contains TINCTUREs`.
fn form(text: &str) -> Option<(bool, usize)> {
    for (word, liquid) in [("contains DOSEs", false), ("contains TINCTUREs", true)] {
        if let Some(at) = text.find(word) {
            return Some((liquid, at + word.len()));
        }
    }
    None
}

/// The herbs a listing line names: every object after the kit, each paired
/// with the next `(N)` in the text.
fn herbs_on(line: &ChunkLine, tail: &str, liquid: bool) -> Vec<KitHerb> {
    let mut counts = tail.split('(').skip(1).filter_map(|chunk| {
        chunk
            .split_once(')')
            .and_then(|(n, _)| n.trim().parse::<u32>().ok())
    });
    line.objects()
        .skip(1)
        .filter_map(item_ref)
        .filter_map(|item| {
            counts.next().map(|count| KitHerb {
                item,
                count,
                liquid,
            })
        })
        .collect()
}

fn first_id(line: &ChunkLine) -> Option<String> {
    line.objects().find_map(item_ref).map(|item| item.id)
}

fn item_ref(link: &cena_protocol::frame::Link) -> Option<ItemRef> {
    match &link.kind {
        cena_protocol::frame::LinkKind::Exist { id, noun } => Some(ItemRef {
            id: id.clone(),
            noun: noun.clone(),
            text: link.text.clone(),
        }),
        _ => None,
    }
}
