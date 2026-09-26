//! Reading a line's objects and figures for the loot classifier.
//!
//! The one place the classifier touches a link: everything else matches plain
//! text and asks here for *the creature*, *the item* or *the object called X*.

use cena_protocol::frame::LinkKind;
use regex::Regex;

use crate::state::chunks::ChunkLine;
use crate::state::containers::ItemRef;

/// Every game object on the line, in order, with whether it arrived bolded.
///
/// `Run::object`, not `Run::link`: the thing the text names, even inside a
/// clickable command (`containers.rs`, `ItemRef::first_on`).
pub(crate) fn objects(line: &ChunkLine) -> Vec<(ItemRef, bool)> {
    line.runs
        .runs
        .iter()
        .filter_map(|run| {
            let object = run.object()?;
            let LinkKind::Exist { id, noun } = &object.kind else {
                return None;
            };
            Some((
                ItemRef {
                    id: id.clone(),
                    noun: noun.clone(),
                    text: object.text.clone(),
                },
                run.style.bold_depth > 0,
            ))
        })
        .collect()
}

/// The creature on the line: the first **bolded** object. Bold is the wire's
/// mark for a creature (`state/room.rs:207`); the pronoun forms (`He`, `her`)
/// carry the same id and the same bold.
pub(crate) fn creature(line: &ChunkLine) -> Option<ItemRef> {
    objects(line)
        .into_iter()
        .find_map(|(item, bold)| bold.then_some(item))
}

/// The item on the line: the first object that is **not** bolded.
pub(crate) fn item(line: &ChunkLine) -> Option<ItemRef> {
    objects(line)
        .into_iter()
        .find_map(|(item, bold)| (!bold).then_some(item))
}

/// The `n`-th unbolded object, for the lines that name several items.
pub(super) fn item_at(line: &ChunkLine, n: usize) -> Option<ItemRef> {
    objects(line)
        .into_iter()
        .filter_map(|(item, bold)| (!bold).then_some(item))
        .nth(n)
}

/// The object whose display text is exactly `name`: what a text capture
/// resolves to when a line names several objects and the words between them
/// say which is which.
pub(super) fn named(line: &ChunkLine, name: &str) -> Option<ItemRef> {
    objects(line)
        .into_iter()
        .find_map(|(item, _)| (item.text == name).then_some(item))
}

/// A comma-grouped silver figure, or `None` for anything that is not one.
pub(super) fn silvers(text: &str) -> Option<u64> {
    crate::state::numbers::grouped(text)
}

/// `regex::Captures` group `n` as a silver figure.
pub(super) fn group_silvers(caps: &regex::Captures<'_>, n: usize) -> Option<u64> {
    caps.get(n).and_then(|m| silvers(m.as_str()))
}

/// A pattern that either compiled or never matches.
///
/// The literals are checked by a test per module ([`compiled`]), so a typo is
/// a red test rather than a panic at first use or -- `bounty.rs`'s choice -- a
/// silent miss.
pub(crate) struct Pat(Option<Regex>);

impl Pat {
    pub(crate) fn new(pattern: &str) -> Self {
        Self(Regex::new(pattern).ok())
    }

    pub(crate) fn is_match(&self, text: &str) -> bool {
        self.0.as_ref().is_some_and(|re| re.is_match(text))
    }

    pub(crate) fn captures<'t>(&self, text: &'t str) -> Option<regex::Captures<'t>> {
        self.0.as_ref()?.captures(text)
    }

    #[cfg(test)]
    pub(crate) const fn compiled(&self) -> bool {
        self.0.is_some()
    }
}
