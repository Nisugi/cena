//! Containers and what is in them: **M2 step 4**, `plan/18` §2d.
//!
//! Split out of `state.rs` under Rule 4.1 (`plan/05:352-353`).
//!
//! # The wire shape
//!
//! ```text
//! <container id='stow' title="My Cloak" target='#64863904' location='right'/>
//! <clearContainer id="stow"/>
//! <inv id='stow'>In the <a exist="64863904" noun="cloak">cloak</a>:</inv>
//! <inv id='stow'> an <a exist="64863936" noun="dauber">undulant vaelfyren tentacle dauber</a></inv>
//! <inv id='stow'> a <a exist="64863935" noun="card">pass card</a></inv>
//! ```
//!
//! `<container>` declares, `<clearContainer>` empties, and each `<inv>` carries
//! **one line**. MEASURED over 12 files across 4 characters: **66,996** `<inv>`
//! tags, 1,147 `<clearContainer>`, 167 `<container>`, 38 `<deleteContainer>`.
//!
//! # Three line shapes, and only one is an item
//!
//! MEASURED on the same sample: 45,839 lines carry an `<a exist=>` link, 817 are
//! the container's own header (`In the cloak:`), and the rest are the literal
//! string ` nothing` for an empty container.
//!
//! So an item is **a line with a link**, and the header and ` nothing` fall out
//! for free rather than needing to be recognised: the header's only link is the
//! container itself, which is why it is matched by id rather than by prose.
//! `VellumFE` takes the same "first `<a>` link per line is one item" rule for
//! worn items (`core/messages/element.rs:494-503`).
//!
//! # Ids are `exist` ids, not names
//!
//! MEASURED: the twelve busiest container ids are numbers (`65019875` 14,604
//! times, `64863769` 11,726) with `stow` the one named exception. So the id is
//! kept **verbatim as a string**, the same decision
//! [`RoomItem::id`](super::RoomItem) records and for the same reason: they are
//! negative for some objects and the wire promises no numeric range.

use super::RoomItem;
use cena_protocol::frame::LinkKind;
use cena_protocol::runs::Runs;
use std::collections::BTreeMap;

/// One container and its contents.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Container {
    /// `<container title=>`, e.g. `My Cloak`. `None` until declared -- the wire
    /// sends `<inv>` lines for containers it never declared in the session.
    pub title: Option<String>,
    /// The items in it, in the order the game listed them.
    pub items: Vec<RoomItem>,
    /// `<container target='#64863904'>` with the `#` stripped: the `exist` id of
    /// the container OBJECT, where [`Inventory::container`]'s key is the
    /// container WINDOW.
    ///
    /// Usually the same string, but not always. MEASURED over 12 files: of 817
    /// header lines, **798** carry an `exist` equal to the window id and **19**
    /// do not -- every one of those being `stow`, the one container the wire
    /// names rather than numbers, whose header links `exist="64863904"` while its
    /// window id is `stow`. Without the target, that container lists itself as
    /// its own contents.
    pub target: Option<String>,
}

/// Every container the session knows about.
///
/// `BTreeMap` for criterion 7, as everywhere else in this crate: a replay
/// iterating a `HashMap` would be non-deterministic.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Inventory {
    containers: BTreeMap<String, Container>,
}

impl Inventory {
    /// One container, if the session has seen it.
    #[must_use]
    pub fn container(&self, id: &str) -> Option<&Container> {
        self.containers.get(id)
    }

    /// Every container, in id order.
    pub fn containers(&self) -> impl Iterator<Item = (&str, &Container)> {
        self.containers.iter().map(|(id, c)| (id.as_str(), c))
    }

    /// Whether anything is known at all.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.containers.is_empty()
    }

    /// `<container id= title=>`: declare a container, keeping any contents.
    ///
    /// **Does not clear.** The wire sends `<container>` immediately followed by
    /// `<clearContainer>` when it means to replace the contents, so clearing here
    /// would make the explicit clear redundant and would drop items on a
    /// re-declaration that carried none.
    pub(super) fn declare(&mut self, id: &str, title: Option<&str>, target: Option<&str>) {
        let entry = self.containers.entry(id.to_owned()).or_default();
        if let Some(title) = title {
            entry.title = Some(title.to_owned());
        }
        if let Some(target) = target {
            // `target='#64863904'`. The `#` is a wire convention, not part of
            // the id -- `exist=` never carries one.
            entry.target = Some(target.trim_start_matches('#').to_owned());
        }
    }

    /// `<clearContainer id=>`: empty one container, keeping its declaration.
    ///
    /// The title survives, because the wire clears in order to refill and a
    /// window that lost its name mid-refresh would flicker.
    pub(super) fn clear(&mut self, id: &str) {
        if let Some(c) = self.containers.get_mut(id) {
            c.items.clear();
        }
    }

    /// `<deleteContainer id=>`: the container is gone.
    pub(super) fn delete(&mut self, id: &str) {
        self.containers.remove(id);
    }

    /// `<inv id=>`: one line of a container's listing.
    ///
    /// An item is **a line carrying an `<a exist=>` link that is not the
    /// container itself**. That one rule covers all three shapes the wire sends:
    /// the header names the container, so it is excluded by id; ` nothing` has no
    /// link at all; an item line has exactly one.
    pub(super) fn add_line(&mut self, id: &str, body: &Runs) {
        let entry = self.containers.entry(id.to_owned()).or_default();
        for (index, run) in body.runs.iter().enumerate() {
            let Some(link) = run.link.as_ref() else {
                continue;
            };
            let LinkKind::Exist { id: exist, noun } = &link.kind else {
                continue;
            };
            // The header line -- `In the <a exist="64863904">cloak</a>:` -- names
            // the container, and a container is not inside itself. Matched
            // against BOTH ids: the window id covers 798 of 817 headers, and the
            // target covers the 19 `stow` ones where they differ.
            if exist == id || entry.target.as_deref() == Some(exist.as_str()) {
                continue;
            }
            // The prose either side of the link, which together with the link
            // text is what `gameobj.rb:227`'s `full_name` joins. Read
            // positionally off the runs, in wire order, exactly as `room.rs`
            // reads a player's status from the following run -- no re-parsing
            // of markup (Rule 2.1).
            //
            // **Adjacent runs only.** A line carrying two links would otherwise
            // attribute the whole tail to the first: `xmlparser.rb:1040-1042`
            // has the same restriction by construction, since it overwrites
            // `@obj_before_name` on each `<a>`. MEASURED at 45,839 linked
            // `<inv>` lines, the wire sends one item per line.
            let before = adjacent_prose(body, index.checked_sub(1));
            let after = adjacent_prose(body, Some(index + 1));
            entry.items.push(RoomItem {
                id: exist.clone(),
                noun: noun.clone(),
                text: link.text.clone(),
                before,
                after,
                // Container contents are things, not people; `status` is a
                // property of a player in a room roster.
                status: None,
            });
        }
    }
}

/// The trimmed text of a neighbouring run, when it is prose rather than a link.
///
/// `None` for an index off either end, for a run that is itself a link -- two
/// items abutting are not each other's qualifiers -- and for text that is
/// nothing but whitespace, which `xmlparser.rb:1040`'s `.strip` also discards.
fn adjacent_prose(body: &Runs, index: Option<usize>) -> Option<String> {
    let run = body.runs.get(index?)?;
    if run.link.is_some() {
        return None;
    }
    let trimmed = run.text.trim();
    (!trimmed.is_empty()).then(|| trimmed.to_owned())
}

impl crate::GameState {
    /// The four container frames: declare, clear, delete, add a line.
    ///
    /// Grouped here rather than as four arms in `GameState::apply` because
    /// they are one feed -- `plan/18` step 4 -- and because Rule 4.1 puts the
    /// work in the module rather than in the facade.
    ///
    /// Anything else is ignored: the caller matches exactly these four, so a
    /// fifth frame arriving here would be a caller bug, not wire data.
    pub(super) fn apply_container(&mut self, frame: &cena_protocol::Frame) {
        use cena_protocol::Frame;
        match frame {
            Frame::Container { id, title, target } => {
                self.inventory
                    .declare(id, title.as_deref(), target.as_deref());
            }
            Frame::ClearContainer { id } => self.inventory.clear(id),
            Frame::DeleteContainer { id } => self.inventory.delete(id),
            Frame::ContainerItem {
                container_id,
                content,
            } => self.inventory.add_line(container_id, content),
            _ => {}
        }
    }
}
