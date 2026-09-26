//! How many doses a herb has left, by item id: eherbs' dose monitor
//! (`exec_str`, `eherbs.lic:255-348`), read from the chunk as a consumer
//! (`plan/36` Stage 1).
//!
//! eherbs hooks the stream and keeps `$eherbs_measure[id]` from four kinds of
//! line: what eating or drinking leaves (*You have 3 bites left*, *That was
//! the last drop*), what `measure` says (*The acantha leaf has 4 bites left*,
//! or the vague *has a few doses left*), a purchase (*Here's your purchase*,
//! the herb's store doses), and the bundling and splitting of herbs. The
//! vague answers are ranges; eherbs keeps its old count when that count is in
//! the range and a fixed figure when it is not, and so does this.
//!
//! The counts are facts about items in your own containers, so a reconnect
//! keeps them, as it keeps the stow list.

use std::collections::BTreeMap;

use super::chunks::{Chunk, ChunkLine};
use crate::herbs;

/// Doses left, by item id. Absent is *not measured*, never zero.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Doses {
    left: BTreeMap<String, u32>,
}

/// A vague answer's range and the figure eherbs settles on outside it.
struct Vague {
    tail: &'static str,
    low: u32,
    high: u32,
    figure: u32,
}

impl Doses {
    /// Doses left in this item, when measured.
    #[must_use]
    pub fn left(&self, id: &str) -> Option<u32> {
        self.left.get(id).copied()
    }

    /// Forget an item's count, so it is measured again.
    pub fn forget(&mut self, id: &str) {
        self.left.remove(id);
    }

    /// Read one chunk. `right` and `left` are the hands' item ids as the
    /// chunk leaves them: a bundle forms in the right hand, a split dose
    /// lands in the left (`:320-345`).
    pub(crate) fn read_chunk(&mut self, chunk: &Chunk, right: Option<&str>, left: Option<&str>) {
        // The item being eaten or drunk from: its next "left" line is its
        // count. Reset at the prompt, which is the chunk's end.
        let mut using: Option<String> = None;
        let mut bought: Option<u32> = None;
        for line in chunk.lines() {
            let text = line.text();
            let text = text.trim();
            let first = || first_object(line);
            if let Some(id) = &using
                && let Some(count) = count_after_use(text)
            {
                self.left.insert(id.clone(), count);
                continue;
            }
            if text.starts_with("You take a drink from your ")
                || text.starts_with("You take a bite of your ")
            {
                using = first();
                continue;
            }
            if text.starts_with("You carefully pour a little bit from your ") {
                let mut objects = line.objects().filter_map(exist);
                using = objects.next();
                if let Some(into) = objects.next()
                    && let Some(count) = self.left.get_mut(&into)
                {
                    *count += 1;
                }
                continue;
            }
            if text.starts_with("You carefully remove one dose from your ") {
                if let Some(id) = first()
                    && let Some(count) = self.left.get_mut(&id)
                {
                    *count = count.saturating_sub(1);
                }
                if let Some(split) = left {
                    self.left.insert(split.to_owned(), 1);
                }
                continue;
            }
            if text.contains("and says, \"Here's your purchase.") {
                // The herb handed over: its store doses, eherbs' four when it
                // is not a herb the table knows.
                let name = line.objects().next().map(|o| o.text.clone());
                bought = Some(
                    name.and_then(|n| herbs::herb_for(&n).map(|h| h.store_doses))
                        .unwrap_or(4),
                );
                continue;
            }
            if text.starts_with("Carefully, you combine all your ")
                && text.ends_with("into one bundle.")
            {
                self.bundled(right, left, bought.take());
                continue;
            }
            if let Some(id) = first() {
                self.measured(&id, text);
            }
        }
    }

    /// A bundle formed in the right hand: a purchase adds its doses to a
    /// counted bundle; otherwise a bundle known to be large stays large and
    /// anything else is measured again (`:322-339`).
    fn bundled(&mut self, right: Option<&str>, left: Option<&str>, bought: Option<u32>) {
        let Some(bundle) = right else { return };
        if let (Some(count), Some(doses)) = (self.left.get_mut(bundle), bought) {
            *count += doses;
            return;
        }
        let large = |id: Option<&str>| id.and_then(|id| self.left.get(id)).is_some_and(|n| *n > 10);
        if large(Some(bundle)) || large(left) {
            // Overestimated so no more is bought than bundles; the exact
            // count comes when the herb is used.
            self.left.insert(bundle.to_owned(), 50);
        } else {
            self.left.remove(bundle);
        }
        if let Some(left) = left {
            self.left.remove(left);
        }
    }

    /// A `measure` answer about this item.
    fn measured(&mut self, id: &str, text: &str) {
        if let Some(v) = VAGUE.iter().find(|v| text.ends_with(v.tail)) {
            let keep = self
                .left
                .get(id)
                .is_some_and(|n| (v.low..=v.high).contains(n));
            if !keep {
                self.left.insert(id.to_owned(), v.figure);
            }
        } else if text.ends_with("has 1 dose left.")
            || text.ends_with("has one bite left.")
            || text.ends_with("has 1 bite left.")
        {
            self.left.insert(id.to_owned(), 1);
        } else if let Some(count) = number_before(text, " doses left.")
            .or_else(|| number_before(text, " bites left."))
            .filter(|_| text.starts_with("The "))
        {
            self.left.insert(id.to_owned(), count);
        }
    }
}

/// The vague `measure` answers: eherbs keeps its old count inside the range
/// and settles on the figure outside it (`:278-310`).
const VAGUE: &[Vague] = &[
    Vague {
        tail: "has several doses left.",
        low: 5,
        high: 10,
        figure: 7,
    },
    Vague {
        tail: "has a few doses left.",
        low: 3,
        high: 4,
        figure: 4,
    },
    Vague {
        tail: "seems to have plenty of bites left.",
        low: 11,
        high: 50,
        figure: 50,
    },
    Vague {
        tail: "looks like it has several bites left.",
        low: 5,
        high: 10,
        figure: 10,
    },
    Vague {
        tail: "looks like it has a few bites left.",
        low: 3,
        high: 4,
        figure: 4,
    },
];

/// What eating or drinking says is left (`:265-275`).
fn count_after_use(text: &str) -> Option<u32> {
    if text.starts_with("You have only about one quaff left.")
        || text.starts_with("You only have one dose left.")
        || text.starts_with("You only have one quaff left.")
        || text.starts_with("You have one bite left.")
        || text.starts_with("You only have one bite left.")
    {
        return Some(1);
    }
    if text.starts_with("That was the last drop.") || text.starts_with("That was the last of it.") {
        return Some(0);
    }
    let rest = text
        .strip_prefix("You have only about ")
        .or_else(|| text.strip_prefix("You have about "))
        .or_else(|| text.strip_prefix("You have "))?;
    let (number, tail) = rest.split_once(' ')?;
    let count = number.parse().ok()?;
    ["quaffs left.", "doses left.", "bites left."]
        .contains(&tail)
        .then_some(count)
}

/// The number just before `tail` at the end of the line.
fn number_before(text: &str, tail: &str) -> Option<u32> {
    let head = text.strip_suffix(tail)?;
    head.rsplit(' ').next()?.parse().ok()
}

/// The first object's `exist` id on the line.
fn first_object(line: &ChunkLine) -> Option<String> {
    line.objects().find_map(exist)
}

fn exist(link: &cena_protocol::frame::Link) -> Option<String> {
    match &link.kind {
        cena_protocol::frame::LinkKind::Exist { id, .. } => Some(id.clone()),
        _ => None,
    }
}
