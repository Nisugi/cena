//! The loot ledger's classifier: what a chunk says about loot, silver and
//! selling, as typed facts.
//!
//! Ports the pattern half of the author's `loottracker.lic` (`plan/34` §2).
//! That script keeps **55 regexes over the raw XML**, 37 of them triggers, and
//! dispatches each prompt-bounded chunk to one of fourteen processors in a
//! fixed order (`Parser.process`, `loottracker.lic:816-893`). Here the same
//! chunk arrives already parsed -- every `<a exist= noun=>` is a typed link on
//! its [`ChunkLine`](super::chunks::ChunkLine), and bold is a style, not a `<pushBold/>` to match around
//! -- so each pattern says only **which line** and **which link**, against the
//! plain text. No pattern here reads `exist=`.
//!
//! # Stateless, per chunk
//!
//! [`classify`] is `plan/12` §3a's classifier: it looks at one closed chunk and
//! returns what it states. What needs memory across chunks is **not** here and
//! is the ledger's (`cena-session`, `plan/34` §4): pairing a loresong's value
//! line with the item sung over in the chunk before, a pawn offer with the sale
//! that follows, a returned box with the box dropped in the pool. Within one
//! chunk those pairs are made here, because a chunk is one command's output
//! and the two lines are that command's two lines; an unpaired half is
//! reported with its missing side `None` rather than dropped.
//!
//! # What the markup answers that the script had to guess
//!
//! | loottracker | here |
//! |---|---|
//! | the creature is the link inside `<pushBold/>…<popBold/>` | the first **bolded** `exist` link -- `state/room.rs:207`, the wire's own mark for a creature |
//! | the item is the link that is not | the first unbolded `exist` link |
//! | `<inv id=>` lines are a box's contents | never a chunk line: the model's `inventory` holds them, and the ledger reads the box there |
//! | `<left exist=>` is the duplicated wand | the hands model's fact; [`LootFact::WandDuplicated`] carries the donor only |
//!
//! The room a fact happened in is not in the chunk either: it is
//! `state.room.id`, the wire's `<nav rm=>`, which is what `plan/34` §3's Red
//! Forest bug is about.
//!
//! # NOT ported
//!
//! The cross-character proxy (`;loottracker proxy`) -- the author, 2026-09-24:
//! *"can wait until multi-session makes it a session-to-session fact."* And the
//! trading-bonus arithmetic under `cap`, which the author says lootcap has made
//! moot (`plan/34` §7).

mod boxes;
mod hunt;
pub mod pending;
mod text;
mod town;

use super::chunks::Chunk;
use super::containers::ItemRef;

pub use pending::{LootChunk, LootQueue, MAX_PENDING_LOOT};

/// One thing a chunk stated about loot, silver or selling.
///
/// Items are [`ItemRef`]s read off the line's links: the game's `exist` id,
/// its noun and its display text. The variants follow the script's
/// processors, in its dispatch order, so a fact can be traced to its source.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum LootFact {
    /// `You search the <creature>.` and what followed it in the chunk.
    Searched {
        /// The corpse searched.
        creature: ItemRef,
        /// `had N silvers on`: zero when the line said it carried none.
        silvers: u64,
        /// What it had, carried or left, and what turned up rifling through it.
        items: Vec<ItemRef>,
        /// The special finds the script recorded apart from items.
        finds: Vec<Find>,
    },
    /// `You skinned the <creature>, yielding <skin>.`
    Skinned {
        /// The corpse.
        creature: ItemRef,
        /// The skin.
        skin: ItemRef,
    },
    /// A skin went into a bundle.
    Bundled {
        /// The skin added, when the line named one (the manual arrange does not).
        skin: Option<ItemRef>,
        /// The bundle.
        bundle: ItemRef,
        /// The container it sits in, when the line named one.
        container: Option<ItemRef>,
        /// Whether this line made the bundle rather than adding to one.
        created: bool,
    },
    /// `[You have earned N bounty points, N experience points, and N silver.]`
    Bounty {
        /// Bounty points.
        points: u64,
        /// Experience points.
        experience: u64,
        /// Silver.
        silvers: u64,
    },
    /// Spell 918 made a copy of a wand. The copy is in a hand, which the hands
    /// model states; this carries the wand gestured at.
    WandDuplicated {
        /// The wand the character gestured at, when the chunk showed it.
        donor: Option<ItemRef>,
    },
    /// Coins gathered from inside an opened box, by hand or by a charm's swarm.
    BoxOpened {
        /// The box.
        item: ItemRef,
        /// The coins.
        silvers: u64,
    },
    /// A locksmith handed the box back.
    BoxReturned {
        /// The box.
        item: ItemRef,
    },
    /// The pool's quote for a box.
    PoolQuoted {
        /// The box.
        item: ItemRef,
        /// The tip asked.
        tip: u64,
        /// The fee due up front.
        fee: u64,
    },
    /// The pool took a box: the line names it only by its noun.
    PoolDropped {
        /// The noun in `takes your <noun>`.
        noun: String,
        /// The tip recorded.
        tip: u64,
        /// The fee collected.
        fee: u64,
    },
    /// A value was put on an item without a sale.
    ///
    /// `item` or `value` is `None` when the two halves of a two-line appraisal
    /// fell in different chunks; the ledger pairs them.
    Appraised {
        /// The item, when this chunk named it.
        item: Option<ItemRef>,
        /// The silver figure, when this chunk gave it.
        value: Option<u64>,
        /// Who or what valued it.
        by: Appraiser,
    },
    /// Something was sold, or turned in for credit.
    Sold {
        /// The item, when the chunk named it (a bulk sale names the container).
        item: Option<ItemRef>,
        /// The silver received or credited.
        silvers: u64,
        /// Who bought it.
        to: Buyer,
        /// The note or chit handed over instead of coins, if one was.
        note: Option<ItemRef>,
    },
    /// The pawnbroker refused it as worthless.
    Worthless {
        /// The item offered, when the chunk named it.
        item: Option<ItemRef>,
    },
    /// The gem shop refused it as too valuable to buy today.
    TooValuable {
        /// The item asked about, when the chunk named it.
        item: Option<ItemRef>,
    },
    /// `You offer to sell your <item> to …` or `You ask <who> … <item>`, with
    /// no answer in the same chunk. The merchant's answer is usually a prompt
    /// later, so the ledger holds this and pairs it with the sale, the
    /// refusal or the chit that follows.
    Offered {
        /// The item held out.
        item: ItemRef,
    },
    /// A gem shattered under a purification song.
    Shattered {
        /// The gem.
        item: ItemRef,
    },
    /// `You deposit N silvers into your account.`
    Deposited(u64),
    /// The teller handed over N silvers.
    Withdrew(u64),
    /// A note's total was added to the balance.
    NoteDeposited(u64),
}

/// A find the search reported apart from the corpse's items.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Find {
    /// A klock key or lock that `appears on the ground!`; the noun says which.
    Klock(ItemRef),
    /// `You notice a scintillating mote of gemstone dust … and gather it quickly.`
    GemDust,
    /// The gemstone jewel `at your feet!`.
    Jewel(ItemRef),
    /// `You have been awarded N Long-Term Experience Boost!`
    Boost(u32),
}

/// Who put a value on an item.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Appraiser {
    /// `You peer intently at the <gem>`: the character's own gem appraisal.
    Gem,
    /// `You turn the <hide> over in your hands`, or a bundle's total.
    Skin,
    /// A bard's loresong.
    Loresong,
    /// A shopkeeper's offer without a sale.
    Shop,
}

/// Who bought.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Buyer {
    /// The pawnbroker.
    Pawn,
    /// The gem shop.
    Gemshop,
    /// The furrier.
    Furrier,
    /// The Chronomage's halfling, who credits travel rather than paying.
    Chronomage,
}

/// Every loot fact one closed chunk states, in line order.
///
/// Lines are read once each, in wire order, and offered to the hunt readers,
/// then the box readers, then the town readers -- the script's dispatch
/// order, flattened to per-line since a chunk here can hold more than one
/// kind of event.
#[must_use]
pub fn classify(chunk: &Chunk) -> Vec<LootFact> {
    let mut cursor = Cursor::default();
    for line in chunk.lines() {
        let text = line.text();
        let _ = hunt::read(&mut cursor, line, &text)
            || boxes::read(&mut cursor, line, &text)
            || town::read(&mut cursor, line, &text);
    }
    cursor.finish()
}

/// What one pass over a chunk carries between its lines.
#[derive(Default)]
struct Cursor {
    out: Vec<LootFact>,
    /// The search whose result lines are still arriving.
    search: Option<LootFact>,
    /// An item a first line named and a second line values or sells.
    pending: Option<Pending>,
}

/// The first half of a two-line event.
#[derive(Clone, Debug, PartialEq, Eq)]
enum Pending {
    /// `You offer to sell your <item> to` / `You ask <who> … <item>`.
    Offered(ItemRef),
    /// `turns the <item> over in his hands`.
    ShopLooking(ItemRef),
    /// `As you sing, you feel a faint resonating vibration from the <item>`.
    Singing(ItemRef),
    /// `You gesture at <wand>.`
    Gestured(ItemRef),
}

impl Cursor {
    /// Close the open search, and report an unpaired first half rather than
    /// drop it.
    fn finish(mut self) -> Vec<LootFact> {
        self.close_search();
        match self.pending.take() {
            Some(Pending::ShopLooking(item)) => self.out.push(LootFact::Appraised {
                item: Some(item),
                value: None,
                by: Appraiser::Shop,
            }),
            Some(Pending::Singing(item)) => self.out.push(LootFact::Appraised {
                item: Some(item),
                value: None,
                by: Appraiser::Loresong,
            }),
            // The answer to an offer is usually a prompt later: reported so
            // the ledger can pair it. A gesture that did not duplicate is not
            // a loot fact.
            Some(Pending::Offered(item)) => self.out.push(LootFact::Offered { item }),
            Some(Pending::Gestured(_)) | None => {}
        }
        self.out
    }

    fn close_search(&mut self) {
        if let Some(search) = self.search.take() {
            self.out.push(search);
        }
    }

    /// The item a first line named, taken.
    fn take_offered(&mut self) -> Option<ItemRef> {
        match self.pending.take() {
            Some(Pending::Offered(item)) => Some(item),
            other => {
                self.pending = other;
                None
            }
        }
    }
}
