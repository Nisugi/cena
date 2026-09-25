//! One chunk's facts, written: an arm per [`LootFact`], and the item linking
//! loottracker's processors did between chunks.

use cena_model::state::containers::ItemRef;
use cena_model::state::ledger::{Appraiser, Buyer, Find, LootChunk, LootFact};
use rusqlite::{OptionalExtension, params};

use super::{Error, Half, PAIR_WINDOW, Quote, Writer};

/// An `exist` id as the wire wrote it, or NULL for one that is not a number.
fn exist(item: &ItemRef) -> Option<i64> {
    item.id.parse().ok()
}

/// A silver figure for an `INTEGER` column. Saturating rather than wrapping:
/// no figure the game prints approaches the bound.
fn silver(n: u64) -> i64 {
    i64::try_from(n).unwrap_or(i64::MAX)
}

const fn appraiser(by: Appraiser) -> &'static str {
    match by {
        Appraiser::Gem => "gem",
        Appraiser::Skin => "skin",
        Appraiser::Loresong => "loresong",
        Appraiser::Shop => "shop",
    }
}

const fn buyer(to: Buyer) -> &'static str {
    match to {
        Buyer::Pawn => "pawn",
        Buyer::Gemshop => "gemshop",
        Buyer::Furrier => "furrier",
        Buyer::Chronomage => "chronomage",
    }
}

impl Writer<'_> {
    pub(super) fn record(&mut self, chunk: &LootChunk) -> Result<(), Error> {
        self.expire();
        for fact in &chunk.facts {
            self.fact(fact, chunk)?;
        }
        Ok(())
    }

    /// Let go of a first half, or a quote, older than the window.
    fn expire(&mut self) {
        let at = self.at;
        if self
            .live
            .half
            .as_ref()
            .is_some_and(|(_, since)| at - since > PAIR_WINDOW)
        {
            self.live.half = None;
        }
        self.live.quotes.retain(|q| at - q.at <= PAIR_WINDOW);
    }

    fn fact(&mut self, fact: &LootFact, chunk: &LootChunk) -> Result<(), Error> {
        match fact {
            LootFact::Searched {
                creature,
                silvers,
                items,
                finds,
            } => {
                let event = self.event("search", Some(creature), *silvers)?;
                for item in items {
                    self.item(item, "item", Some(event))?;
                }
                for find in finds {
                    self.find(find, event)?;
                }
            }
            LootFact::Skinned { creature, skin } => {
                let row = self.item(skin, "skin", None)?;
                self.tx.execute(
                    "INSERT INTO skin_events (character, creature_exist, creature_noun, \
                     creature_name, skin_item_id, room_id, at) VALUES (?, ?, ?, ?, ?, ?, ?)",
                    params![
                        self.character,
                        exist(creature),
                        creature.noun,
                        creature.text,
                        row,
                        self.room,
                        self.at
                    ],
                )?;
            }
            LootFact::Bundled {
                skin,
                bundle,
                container,
                created,
            } => {
                self.tx.execute(
                    "INSERT INTO bundle_events (character, skin_exist, bundle_exist, bundle_noun, \
                     bundle_name, container_exist, created, at) VALUES (?, ?, ?, ?, ?, ?, ?, ?)",
                    params![
                        self.character,
                        skin.as_ref().and_then(exist),
                        exist(bundle),
                        bundle.noun,
                        bundle.text,
                        container.as_ref().and_then(exist),
                        i64::from(*created),
                        self.at
                    ],
                )?;
            }
            LootFact::Bounty {
                points,
                experience,
                silvers,
            } => {
                self.tx.execute(
                    "INSERT INTO bounty_rewards (character, points, experience, silvers, at) \
                     VALUES (?, ?, ?, ?, ?)",
                    params![
                        self.character,
                        silver(*points),
                        silver(*experience),
                        silver(*silvers),
                        self.at
                    ],
                )?;
                self.transaction("bounty", *silvers, None, "bounty")?;
            }
            LootFact::WandDuplicated { donor } => {
                self.event("wand_dupe", donor.as_ref(), 0)?;
            }
            LootFact::BoxOpened { item, silvers } => {
                let event = self.event("box", Some(item), *silvers)?;
                let row = self.find_or_seen(item)?;
                self.tx.execute(
                    "UPDATE loot_items SET opened_event_id = ? WHERE id = ?",
                    params![event, row],
                )?;
                if let Some(inside) = chunk.contents.get(&item.id) {
                    for content in inside {
                        self.item(content, "content", Some(event))?;
                    }
                }
            }
            other => self.town_fact(other)?,
        }
        Ok(())
    }

    /// The arms for what happens in town, and to a box.
    fn town_fact(&mut self, fact: &LootFact) -> Result<(), Error> {
        match fact {
            LootFact::PoolQuoted { item, tip, fee } => {
                let row = self.find_or_seen(item)?;
                self.tx.execute(
                    "UPDATE loot_items SET pool_tip = ?, pool_fee = ? WHERE id = ?",
                    params![silver(*tip), silver(*fee), row],
                )?;
                self.live.quotes.push(Quote {
                    row,
                    noun: item.noun.clone(),
                    at: self.at,
                });
            }
            LootFact::PoolDropped { noun, tip, fee } => self.pool_dropped(noun, *tip, *fee)?,
            LootFact::BoxReturned { item } => self.returned(item)?,
            LootFact::Appraised { item, value, by } => {
                self.appraised(item.as_ref(), *value, *by)?;
            }
            LootFact::Offered { item } => {
                self.live.half = Some((Half::Offered(item.clone()), self.at));
            }
            LootFact::Sold {
                item,
                silvers,
                to,
                note,
            } => self.sold(item.as_ref(), *silvers, *to, note.as_ref())?,
            LootFact::Worthless { item } => self.refused(item.as_ref(), "worthless")?,
            LootFact::TooValuable { item } => self.refused(item.as_ref(), "too_valuable")?,
            LootFact::Shattered { item } => {
                let row = self.find_or_seen(item)?;
                self.tx.execute(
                    "UPDATE loot_items SET shattered_at = ? WHERE id = ?",
                    params![self.at, row],
                )?;
            }
            LootFact::Deposited(n) => self.transaction("deposit", *n, None, "bank")?,
            LootFact::Withdrew(n) => self.transaction("withdrawal", *n, None, "bank")?,
            LootFact::NoteDeposited(n) => self.transaction("note_deposit", *n, None, "bank")?,
            // The hunt's arms, handled by the caller.
            LootFact::Searched { .. }
            | LootFact::Skinned { .. }
            | LootFact::Bundled { .. }
            | LootFact::Bounty { .. }
            | LootFact::WandDuplicated { .. }
            | LootFact::BoxOpened { .. } => {}
        }
        Ok(())
    }

    fn event(&self, kind: &str, source: Option<&ItemRef>, silvers: u64) -> Result<i64, Error> {
        self.tx.execute(
            "INSERT INTO loot_events (character, kind, source_exist, source_noun, source_name, \
             silvers, room_id, at) VALUES (?, ?, ?, ?, ?, ?, ?, ?)",
            params![
                self.character,
                kind,
                source.and_then(exist),
                source.map(|s| s.noun.as_str()),
                source.map(|s| s.text.as_str()),
                silver(silvers),
                self.room,
                self.at
            ],
        )?;
        Ok(self.tx.last_insert_rowid())
    }

    /// A new item row.
    fn item(&self, item: &ItemRef, kind: &str, event: Option<i64>) -> Result<i64, Error> {
        self.tx.execute(
            "INSERT INTO loot_items (character, exist_id, noun, name, kind, event_id, \
             first_seen_at, first_room_id) VALUES (?, ?, ?, ?, ?, ?, ?, ?)",
            params![
                self.character,
                exist(item),
                item.noun,
                item.text,
                kind,
                event,
                self.at,
                self.room
            ],
        )?;
        Ok(self.tx.last_insert_rowid())
    }

    fn find(&self, find: &Find, event: i64) -> Result<(), Error> {
        let (item, noun, name): (Option<&ItemRef>, &str, String) = match find {
            Find::Klock(piece) => (Some(piece), &piece.noun, piece.text.clone()),
            Find::Jewel(jewel) => (Some(jewel), &jewel.noun, jewel.text.clone()),
            Find::GemDust => (None, "dust", "mote of gem dust".to_owned()),
            Find::Boost(n) => (None, "boost", format!("{n} Long-Term Experience Boost")),
        };
        self.tx.execute(
            "INSERT INTO loot_items (character, exist_id, noun, name, kind, event_id, \
             first_seen_at, first_room_id) VALUES (?, ?, ?, ?, 'find', ?, ?, ?)",
            params![
                self.character,
                item.and_then(exist),
                noun,
                name,
                event,
                self.at,
                self.room
            ],
        )?;
        Ok(())
    }

    /// The most recent row with this `exist` id that is not yet gone.
    fn find_item(&self, item: &ItemRef) -> Result<Option<i64>, Error> {
        let Some(id) = exist(item) else {
            return Ok(None);
        };
        self.tx
            .query_row(
                "SELECT id FROM loot_items WHERE exist_id = ? AND sold_at IS NULL \
                 AND shattered_at IS NULL ORDER BY id DESC LIMIT 1",
                [id],
                |r| r.get(0),
            )
            .optional()
    }

    /// The item's row, or a `seen` row for one never looted here.
    fn find_or_seen(&self, item: &ItemRef) -> Result<i64, Error> {
        match self.find_item(item)? {
            Some(row) => Ok(row),
            None => self.item(item, "seen", None),
        }
    }

    fn transaction(
        &self,
        category: &str,
        amount: u64,
        item: Option<i64>,
        counterparty: &str,
    ) -> Result<(), Error> {
        self.tx.execute(
            "INSERT INTO transactions (character, category, amount, item_id, counterparty, \
             room_id, at) VALUES (?, ?, ?, ?, ?, ?, ?)",
            params![
                self.character,
                category,
                silver(amount),
                item,
                counterparty,
                self.room,
                self.at
            ],
        )?;
        Ok(())
    }

    /// The offered item, if an offer is waiting, taken.
    fn take_offered(&mut self) -> Option<ItemRef> {
        match self.live.half.take() {
            Some((Half::Offered(item), _)) => Some(item),
            other => {
                self.live.half = other;
                None
            }
        }
    }

    fn pool_dropped(&mut self, noun: &str, tip: u64, fee: u64) -> Result<(), Error> {
        // The most recent quote for a box of this noun; failing that, the
        // most recent box of this noun looted and not yet dropped.
        let quoted = self
            .live
            .quotes
            .iter()
            .rposition(|q| q.noun == noun)
            .map(|i| self.live.quotes.remove(i).row);
        let row = match quoted {
            Some(row) => Some(row),
            None => self
                .tx
                .query_row(
                    "SELECT id FROM loot_items WHERE noun = ? AND pool_dropped_at IS NULL \
                     AND sold_at IS NULL ORDER BY id DESC LIMIT 1",
                    [noun],
                    |r| r.get(0),
                )
                .optional()?,
        };
        if let Some(row) = row {
            self.tx.execute(
                "UPDATE loot_items SET pool_tip = ?, pool_fee = ?, pool_room_id = ?, \
                 pool_dropped_at = ? WHERE id = ?",
                params![silver(tip), silver(fee), self.room, self.at, row],
            )?;
        }
        self.transaction("pool_fee", fee, row, "locksmith")?;
        self.transaction("pool_tip", tip, row, "locksmith")
    }

    /// A box handed back: by its id, or -- since the pool may hand back a
    /// new object -- the latest box of its noun dropped in THIS room and not
    /// yet returned, which then takes the new id.
    fn returned(&self, item: &ItemRef) -> Result<(), Error> {
        let mut row = self.find_item(item)?;
        if row.is_none() {
            row = self
                .tx
                .query_row(
                    "SELECT id FROM loot_items WHERE noun = ? AND pool_dropped_at IS NOT NULL                      AND returned_at IS NULL AND pool_room_id IS ? ORDER BY id DESC LIMIT 1",
                    params![item.noun, self.room],
                    |r| r.get(0),
                )
                .optional()?;
            if let Some(found) = row {
                self.tx.execute(
                    "UPDATE loot_items SET exist_id = ? WHERE id = ?",
                    params![exist(item), found],
                )?;
            }
        }
        let row = match row {
            Some(row) => row,
            None => self.item(item, "seen", None)?,
        };
        self.tx.execute(
            "UPDATE loot_items SET returned_at = ? WHERE id = ?",
            params![self.at, row],
        )?;
        Ok(())
    }

    fn appraised(
        &mut self,
        item: Option<&ItemRef>,
        value: Option<u64>,
        by: Appraiser,
    ) -> Result<(), Error> {
        let item = match (item, value) {
            (Some(item), Some(_)) => item.clone(),
            // The first half: hold it for the figure.
            (Some(item), None) => {
                let half = match by {
                    Appraiser::Loresong => Half::Singing(item.clone()),
                    _ => Half::ShopLooking(item.clone()),
                };
                self.live.half = Some((half, self.at));
                return Ok(());
            }
            // The figure alone: the item is the half we hold, if it matches.
            (None, Some(_)) => match (by, self.live.half.take()) {
                (Appraiser::Loresong, Some((Half::Singing(item), _)))
                | (Appraiser::Shop, Some((Half::ShopLooking(item), _))) => item,
                (_, other) => {
                    self.live.half = other;
                    return Ok(());
                }
            },
            (None, None) => return Ok(()),
        };
        let row = self.find_or_seen(&item)?;
        self.tx.execute(
            "UPDATE loot_items SET appraised_value = ?, appraised_by = ?, appraised_at = ? \
             WHERE id = ?",
            params![value.map(silver), appraiser(by), self.at, row],
        )?;
        Ok(())
    }

    fn sold(
        &mut self,
        item: Option<&ItemRef>,
        silvers: u64,
        to: Buyer,
        note: Option<&ItemRef>,
    ) -> Result<(), Error> {
        let offered = self.take_offered();
        let item = item.cloned().or(offered);
        let row = match &item {
            Some(item) => Some(self.find_or_seen(item)?),
            None => None,
        };
        if let Some(row) = row {
            self.tx.execute(
                "UPDATE loot_items SET sold_value = ?, sold_to = ?, sold_note_exist = ?, \
                 sold_at = ?, sold_room_id = ? WHERE id = ?",
                params![
                    silver(silvers),
                    buyer(to),
                    note.and_then(exist),
                    self.at,
                    self.room,
                    row
                ],
            )?;
        }
        let category = if to == Buyer::Chronomage {
            "credit"
        } else {
            "sale"
        };
        self.transaction(category, silvers, row, buyer(to))
    }

    fn refused(&mut self, item: Option<&ItemRef>, how: &str) -> Result<(), Error> {
        let offered = self.take_offered();
        let Some(item) = item.cloned().or(offered) else {
            return Ok(());
        };
        let row = self.find_or_seen(&item)?;
        self.tx.execute(
            "UPDATE loot_items SET refused = ? WHERE id = ?",
            params![how, row],
        )?;
        Ok(())
    }
}
