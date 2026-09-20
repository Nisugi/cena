//! The whole-inventory snapshot: `<inventoryManager>` and the item detail
//! responses that enrich it.
//!
//! # Why this is not [`Inventory`](super::Inventory)
//!
//! Two different things, and they answer different questions.
//! [`Inventory`](super::Inventory) is the **passive container model**: what
//! the game streams unasked as `<container>` / `<inv>` lines, covering the
//! containers whose windows are open right now, live and always current.
//!
//! This is a **point-in-time answer to a request**. It carries the entire
//! nested tree -- every container's contents whether its window is open or
//! not, each item's weight and capacity, and its closed/locked state -- none
//! of which the passive stream provides. That is what makes "find item X
//! anywhere", "audit my inventory" and weight budgeting answerable at all.
//!
//! The cost of that completeness is freshness: nothing here tracks a later
//! get, put, loot or hand change. A consumer asking "what am I holding right
//! now" wants the passive model. Lich draws the same line, in the same words
//! (`reference/lich-5/lib/common/inventory.rb:20-31`).
//!
//! MEASURED over the author's September logs: 36 snapshots, 5,364 item rows,
//! about 149 items each.

use std::collections::BTreeMap;

use cena_protocol::{Continuation, InventoryItem, ItemDetail};

/// The most recent `<inventoryManager>` snapshot, and the item details known
/// against it.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct InventorySnapshot {
    /// Items by exist id.
    ///
    /// `BTreeMap` for criterion 7, as everywhere else in this crate: a replay
    /// iterating a `HashMap` would be non-deterministic.
    items: BTreeMap<String, InventoryItem>,
    /// The room the snapshot was taken in.
    room: Option<String>,
    /// Cursors the server sent saying the snapshot is incomplete.
    ///
    /// **Absent from the author's logs** -- 0 across 36 snapshots -- but kept,
    /// because a truncated snapshot that claims to be whole is worse than one
    /// that says so. See [`Self::is_complete`].
    pending: Vec<Continuation>,
    /// The server's error marker on the last response, e.g. `stale`.
    state: Option<String>,
    /// Detail sections by exist id, from `<inventoryViewItem>`.
    details: BTreeMap<String, Vec<ItemDetail>>,
    /// Whether a snapshot has ever arrived.
    known: bool,
}

impl InventorySnapshot {
    /// Absorb one `<inventoryManager>` response.
    ///
    /// A response carrying a `state` is the server reporting a failed or
    /// stale load. It replaces nothing -- overwriting a good snapshot with a
    /// failure would lose the whole inventory on a transient error -- but the
    /// marker is recorded so a caller can see it and re-request.
    pub fn apply_snapshot(
        &mut self,
        room: &str,
        items: &[InventoryItem],
        continuations: &[Continuation],
        state: Option<&str>,
    ) {
        self.state = state.map(str::to_owned);
        if state.is_some() {
            return;
        }
        // A snapshot IS the inventory: anything absent was dropped, sold or
        // stored. Merging would keep an item forever.
        self.items.clear();
        for item in items {
            self.items.insert(item.id.clone(), item.clone());
        }
        self.room = Some(room.to_owned());
        self.pending = continuations.to_vec();
        self.known = true;
        // Details describe items; the ones that left are no longer ours.
        self.details.retain(|id, _| self.items.contains_key(id));
    }

    /// Absorb one `<inventoryViewItem>` response.
    ///
    /// A torn or failed response is not recorded: a partial description
    /// replacing a complete one loses information.
    pub fn apply_detail(&mut self, exist: &str, results: &[ItemDetail], state: Option<&str>) {
        if state.is_some() || results.is_empty() {
            return;
        }
        self.details.insert(exist.to_owned(), results.to_vec());
    }

    /// One item by exist id.
    #[must_use]
    pub fn get(&self, id: &str) -> Option<&InventoryItem> {
        self.items.get(id)
    }

    /// Every item held, in id order.
    pub fn all(&self) -> impl Iterator<Item = &InventoryItem> {
        self.items.values()
    }

    /// What is directly inside a container, by its exist id.
    pub fn contents_of<'a>(&'a self, id: &'a str) -> impl Iterator<Item = &'a InventoryItem> + 'a {
        self.items.values().filter(move |i| i.parent == id)
    }

    /// What the character is wearing or holding: everything whose parent is
    /// the player rather than a container.
    pub fn on_person(&self) -> impl Iterator<Item = &InventoryItem> {
        self.items.values().filter(|i| i.parent == "player")
    }

    /// Every item with this noun, anywhere in the tree.
    ///
    /// The find-anywhere question the passive model cannot answer, because it
    /// only holds containers whose windows are open.
    pub fn find<'a>(&'a self, noun: &'a str) -> impl Iterator<Item = &'a InventoryItem> + 'a {
        self.items.values().filter(move |i| i.noun == noun)
    }

    /// Total weight carried, over items that state one.
    ///
    /// Fixtures are excluded: a `-1` weight is the wire's sentinel for
    /// something that cannot be picked up, and summing it would subtract.
    #[must_use]
    pub fn total_weight(&self) -> i32 {
        self.items
            .values()
            .filter(|i| i.can_pick_up())
            .filter_map(|i| i.weight)
            .sum()
    }

    /// The detail sections known for an item.
    #[must_use]
    pub fn details(&self, id: &str) -> Option<&[ItemDetail]> {
        self.details.get(id).map(Vec::as_slice)
    }

    /// One detail section by its command, e.g. `look` or `analyze`.
    #[must_use]
    pub fn detail(&self, id: &str, command: &str) -> Option<&ItemDetail> {
        self.details.get(id)?.iter().find(|d| d.command == command)
    }

    /// The room the snapshot was taken in.
    #[must_use]
    pub fn room(&self) -> Option<&str> {
        self.room.as_deref()
    }

    /// The server's error marker on the last response, if any.
    #[must_use]
    pub fn state(&self) -> Option<&str> {
        self.state.as_deref()
    }

    /// Cursors saying the snapshot was cut off at a page boundary.
    #[must_use]
    pub fn pending(&self) -> &[Continuation] {
        &self.pending
    }

    /// Whether the snapshot is the whole tree rather than a truncated page.
    #[must_use]
    pub fn is_complete(&self) -> bool {
        self.known && self.pending.is_empty()
    }

    /// How many items are held.
    #[must_use]
    pub fn len(&self) -> usize {
        self.items.len()
    }

    /// Whether none are held.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }

    /// Whether a snapshot has ever arrived.
    ///
    /// `false` with none held means nobody has asked; `true` with none held
    /// means the game answered and the character carries nothing.
    #[must_use]
    pub const fn is_known(&self) -> bool {
        self.known
    }
}

impl super::GameState {
    /// Apply either inventory response.
    ///
    /// Split out of [`GameState::apply`](super::GameState::apply) under Rule
    /// 4.1 (`plan/05:352-353`) -- move code down, do not raise the cap. The
    /// two arms together put that function at 113 of 100 lines.
    pub(super) fn apply_inventory(&mut self, frame: &cena_protocol::Frame) {
        match frame {
            cena_protocol::Frame::InventoryManager(snap) => self.inventory_snapshot.apply_snapshot(
                &snap.room,
                &snap.items,
                &snap.continuations,
                snap.state.as_deref(),
            ),
            cena_protocol::Frame::InventoryViewItem(view) => self.inventory_snapshot.apply_detail(
                &view.exist,
                &view.results,
                view.state.as_deref(),
            ),
            // The caller matches exactly the two arms above; anything else
            // reaching here would be a caller bug, not a wire fact, so there
            // is nothing to record.
            _ => {}
        }
    }
}
