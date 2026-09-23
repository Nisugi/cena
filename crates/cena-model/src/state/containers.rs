//! Which container holds what, and which weapon comes to hand.
//!
//! Two lists the game keeps for you, taught by `stow list` and `ready list`
//! and by the one-line confirmations that follow a `stow set` or `ready`.
//! Ports `gemstone/stowlist.rb` (78 lines), `gemstone/readylist.rb` (96) and
//! the twelve patterns in `infomon/xmlparser.rb:514-527` that actually fill
//! them -- the two class files hold no patterns of their own.
//!
//! # The port is shorter for §3a's reason, again
//!
//! Every one of Lich's patterns here matches **raw XML**, because `GameObj`
//! does not hand it the link. `StowListContainer` (`xmlparser.rb:516`) spends
//! 80 of its 190 characters re-tokenizing `<a exist= noun=>`, and the same
//! fragment is repeated in six more patterns. Cena's parser already did that:
//! [`ChunkLine::links`] yields [`LinkKind::Exist`] typed, so a pattern here
//! only has to say **which slot** a line is about.
//!
//! # A defect the typed port makes unrepresentable
//!
//! Lich stores the store-mode as a **raw captured string**, and the two
//! messages that teach it disagree about its wording:
//!
//! | Taught by | Pattern | "wear it if you can" is spelled |
//! |---|---|---|
//! | `ready list` | `ReadyListNormal` (`:522`) | `worn if possible, stowed otherwise` |
//! | `store set` | `ReadyStoreSet` (`:526`) | `worn if possible and stowed if not` |
//!
//! Same state, two spellings, both written to the same key
//! (`xmlparser.rb:605` and `:617`). A consumer comparing that string gets a
//! different answer depending on which message happened to teach it last.
//! The sheath spelling splits the same way -- `put in secondary sheath`
//! against `stored in your secondary sheath`. [`StoreMode`] reads both and
//! stores one value, so the question "will this be worn?" has one answer.
//!
//! # What is deliberately NOT ported
//!
//! `valid?` (`stowlist.rb:45`, `readylist.rb:59`) re-checks every held id
//! against `GameObj.inv` and clears the `checked` flag if one has gone. That
//! is a *cache-coherence* check, and it belongs to whoever owns inventory --
//! not to a record of what the game said. `Containers`'s `checked` flag reports
//! whether the list was ever taught; a consumer that needs "and the items
//! still exist" asks inventory, which is the layer that knows.

use cena_protocol::frame::LinkKind;

use crate::state::chunks::ChunkLine;

/// An item the game named, held by the id a command targets it with.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ItemRef {
    /// `exist=`, verbatim. The identity.
    pub id: String,
    /// `noun=`. What a command targets it by.
    pub noun: String,
    /// The link's display text, e.g. `"leather backpack"`.
    pub text: String,
}

impl ItemRef {
    /// Read the first game object named on a line, if there is one.
    ///
    /// [`ChunkLine::objects`], not `links`: in `ready list` the item's own
    /// text is clickable and clicking it sends `store WEAPON clear`, so the
    /// markup states both what the text NAMES and what a click SENDS. `links`
    /// gives the command; `objects` gives the thing. Asking for links here
    /// reported no item at all -- which is how the parser's loss was found.
    fn first_on(line: &ChunkLine) -> Option<Self> {
        line.objects().find_map(|link| match &link.kind {
            LinkKind::Exist { id, noun } => Some(Self {
                id: id.clone(),
                noun: noun.clone(),
                text: link.text.clone(),
            }),
            _ => None,
        })
    }
}

/// A stow category: which container a kind of loot goes into.
///
/// `stowlist.rb:6`'s `ORIGINAL_STOW_LIST`, ported whole. A fourteenth
/// category added by the game is a compile error at [`StowSlot::parse`]'s
/// match rather than a silent miss.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum StowSlot {
    Box,
    Gem,
    Herb,
    Skin,
    Wand,
    Scroll,
    Potion,
    Trinket,
    Reagent,
    Lockpick,
    Treasure,
    Forageable,
    Collectible,
    /// Where anything uncategorised goes -- and what `bank.rb:155` reaches
    /// for when it needs somewhere to put your notes.
    Default,
}

impl StowSlot {
    /// Every slot, in `stowlist.rb:6`'s order.
    pub const ALL: [Self; 14] = [
        Self::Box,
        Self::Gem,
        Self::Herb,
        Self::Skin,
        Self::Wand,
        Self::Scroll,
        Self::Potion,
        Self::Trinket,
        Self::Reagent,
        Self::Lockpick,
        Self::Treasure,
        Self::Forageable,
        Self::Collectible,
        Self::Default,
    ];

    /// The wire's word for this slot, lowercase.
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Box => "box",
            Self::Gem => "gem",
            Self::Herb => "herb",
            Self::Skin => "skin",
            Self::Wand => "wand",
            Self::Scroll => "scroll",
            Self::Potion => "potion",
            Self::Trinket => "trinket",
            Self::Reagent => "reagent",
            Self::Lockpick => "lockpick",
            Self::Treasure => "treasure",
            Self::Forageable => "forageable",
            Self::Collectible => "collectible",
            Self::Default => "default",
        }
    }

    /// Read a slot from the wire's word.
    ///
    /// **Case-insensitive**, because the game shouts it in one message and
    /// whispers it in the other: `stow list` prints `(box)` and the
    /// confirmation prints `your STOW BOX container` (`xmlparser.rb:516`
    /// against `:517`). Lich downcases at the call site
    /// (`xmlparser.rb:593`); doing it here keeps the knowledge in the type.
    #[must_use]
    pub fn parse(word: &str) -> Option<Self> {
        Self::ALL
            .into_iter()
            .find(|slot| slot.as_str().eq_ignore_ascii_case(word))
    }
}

/// A ready slot: which item comes to hand for a job.
///
/// `readylist.rb:6`'s `ORIGINAL_READY_LIST`, ported whole.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ReadySlot {
    Shield,
    Weapon,
    SecondaryWeapon,
    RangedWeapon,
    AmmoBundle,
    Ammo2Bundle,
    Sheath,
    SecondarySheath,
    Wand,
}

impl ReadySlot {
    /// Every slot, in `readylist.rb:6`'s order.
    pub const ALL: [Self; 9] = [
        Self::Shield,
        Self::Weapon,
        Self::SecondaryWeapon,
        Self::RangedWeapon,
        Self::AmmoBundle,
        Self::Ammo2Bundle,
        Self::Sheath,
        Self::SecondarySheath,
        Self::Wand,
    ];

    /// The wire's phrase for this slot, as `ready list` labels its rows.
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Shield => "shield",
            Self::Weapon => "weapon",
            Self::SecondaryWeapon => "secondary weapon",
            Self::RangedWeapon => "ranged weapon",
            Self::AmmoBundle => "ammo bundle",
            Self::Ammo2Bundle => "ammo2 bundle",
            Self::Sheath => "sheath",
            Self::SecondarySheath => "secondary sheath",
            Self::Wand => "wand",
        }
    }

    /// Read a slot from the wire's phrase.
    ///
    /// Lich reaches the same key through `Util.normalize_name`
    /// (`util/util.rb:59`), which downcases and turns spaces into
    /// underscores -- `"ammo bundle"` into `:ammo_bundle`. The mapping is the
    /// same; here it is a table rather than a string rewrite, so a phrase the
    /// game adds later returns `None` instead of inventing a key nothing
    /// reads.
    #[must_use]
    pub fn parse(phrase: &str) -> Option<Self> {
        Self::ALL
            .into_iter()
            .find(|slot| slot.as_str().eq_ignore_ascii_case(phrase))
    }

    /// Whether this slot has a store-mode.
    ///
    /// `readylist.rb:22`'s `@store_list` holds six of the nine: the two
    /// sheaths and the second ammo bundle are *where things go*, so asking
    /// where to store them is not a question the game offers.
    #[must_use]
    pub fn has_store_mode(self) -> bool {
        matches!(
            self,
            Self::Shield
                | Self::Weapon
                | Self::SecondaryWeapon
                | Self::RangedWeapon
                | Self::AmmoBundle
                | Self::Wand
        )
    }
}

/// Where an item goes when you store it.
///
/// **Three states, two vocabularies.** See the module doc: `ready list` and
/// `store set` word the same fact differently, and Lich keeps whichever
/// string arrived last. This type reads both spellings into one value.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum StoreMode {
    /// Worn if it can be, stowed if not.
    WornIfPossible,
    /// Stowed, into the stow container for its kind.
    Stowed,
    /// Into the sheath.
    Sheath,
    /// Into the secondary sheath.
    SecondarySheath,
}

impl StoreMode {
    /// Read either vocabulary.
    ///
    /// The secondary spellings are tested **before** their primaries, because
    /// `"stored in your secondary sheath"` contains `"sheath"` and would
    /// otherwise answer [`Self::Sheath`].
    #[must_use]
    pub fn parse(phrase: &str) -> Option<Self> {
        let phrase = phrase.trim();
        if phrase.starts_with("worn if possible") {
            return Some(Self::WornIfPossible);
        }
        if phrase == "stowed" {
            return Some(Self::Stowed);
        }
        if phrase.ends_with("secondary sheath") {
            return Some(Self::SecondarySheath);
        }
        if phrase.ends_with("sheath") {
            return Some(Self::Sheath);
        }
        None
    }
}

/// What one line said about the two lists.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ContainerEvent {
    /// `You have the following containers set as stow targets:` -- the list
    /// is about to be restated, so forget what is held.
    StowListBegins,
    /// One row of `stow list`, or a `stow set` confirmation.
    StowSet {
        /// Which category.
        slot: StowSlot,
        /// The container it goes in.
        item: ItemRef,
    },
    /// `Your current settings are:` -- `ready list` is about to restate.
    ReadyListBegins,
    /// One row of `ready list`, or a `ready` confirmation.
    ReadySet {
        /// Which slot.
        slot: ReadySlot,
        /// The item, or `None` where the row said `none`.
        item: Option<ItemRef>,
        /// The store-mode, where the row carried one.
        store: Option<StoreMode>,
    },
    /// `Cleared your default <slot>.`
    ReadyCleared(ReadySlot),
    /// `When storing your <slot>, it will be <mode>.`
    StoreModeSet {
        /// Which slot.
        slot: ReadySlot,
        /// Where it will go.
        store: StoreMode,
    },
    /// The closing line of `ready list`, which is how we know it was whole.
    ReadyListEnds,
}

/// Read a line as a container event, or `None` if it is not one.
#[must_use]
pub fn classify(line: &ChunkLine) -> Option<ContainerEvent> {
    classify_text(line, &line.text())
}

/// [`classify`], given the line's text already rendered (the chunk renders
/// each line once and shares it).
#[allow(clippy::too_many_lines)]
pub(crate) fn classify_text(line: &ChunkLine, text: &str) -> Option<ContainerEvent> {
    let trimmed = text.trim();

    if trimmed == "You have the following containers set as stow targets:" {
        return Some(ContainerEvent::StowListBegins);
    }
    if trimmed == "Your current settings are:" {
        return Some(ContainerEvent::ReadyListBegins);
    }
    if trimmed.starts_with("To change your default item for a category") {
        return Some(ContainerEvent::ReadyListEnds);
    }

    // `Cleared your default shield.` -- no link, so it is checked before the
    // arms that need one.
    if let Some(rest) = trimmed.strip_prefix("Cleared your default ") {
        let slot = ReadySlot::parse(rest.trim_end_matches('.'))?;
        return Some(ContainerEvent::ReadyCleared(slot));
    }

    // `When storing your weapon, it will be stowed.`
    if let Some(rest) = trimmed.strip_prefix("When storing your ") {
        let (slot, mode) = rest.split_once(", it will be ")?;
        return Some(ContainerEvent::StoreModeSet {
            slot: ReadySlot::parse(slot)?,
            store: StoreMode::parse(mode.trim_end_matches('.'))?,
        });
    }

    // **The two list rows are INDENTED, and Lich requires it.** Both
    // `StowListContainer` (`xmlparser.rb:516`) and the three `ReadyList*`
    // row patterns (`:522-524`) open with `^  `: two spaces, the listing's
    // own layout. The port matched the trimmed text, so any main-window line
    // ending `(gem)` with a link on it taught a stow container, and any line
    // shaped `shield: ...` taught a ready slot (review). Indentation is what
    // separates the listing from prose that happens to look like a row.
    let indented = text.starts_with("  ");

    // A `stow list` row: `  <a ...>my backpack</a> (gem)`. The category is
    // the parenthesised word at the end, and the container is the link.
    if indented && let Some(slot) = trailing_parenthetical(trimmed).and_then(StowSlot::parse) {
        return Some(ContainerEvent::StowSet {
            slot,
            item: ItemRef::first_on(line)?,
        });
    }

    // `Set "<a ...>a pouch</a>" to be your STOW GEM container.` and the
    // `default` spelling, which `xmlparser.rb:518` splits into a second
    // pattern only because the game omits `^` on that one line. Both begin
    // `Set "` in Lich (`:517` anchored, `:518` not), so both require it here.
    if trimmed.contains("Set \"")
        && trimmed.contains("\" to be your ")
        && trimmed.ends_with(" STOW container.")
    {
        let slot = trimmed
            .rsplit_once("\" to be your ")
            .map(|(_, tail)| tail.trim_end_matches(" STOW container."))
            .and_then(StowSlot::parse)?;
        return Some(ContainerEvent::StowSet {
            slot,
            item: ItemRef::first_on(line)?,
        });
    }
    if trimmed.starts_with("Set \"")
        && let Some((_, tail)) = trimmed.rsplit_once("\" to be your STOW ")
    {
        let slot = StowSlot::parse(tail.trim_end_matches(" container."))?;
        return Some(ContainerEvent::StowSet {
            slot,
            item: ItemRef::first_on(line)?,
        });
    }

    // `Setting <a ...>a sword</a> to be your default weapon.`
    //
    // `ReadyItemSet` (`xmlparser.rb:527`) requires the link, so a line with
    // none is not this event -- `?`, not an `Option` that would clear.
    if trimmed.starts_with("Setting ")
        && let Some((_, tail)) = trimmed.rsplit_once(" to be your default ")
    {
        return Some(ContainerEvent::ReadySet {
            slot: ReadySlot::parse(tail.trim_end_matches('.'))?,
            item: Some(ItemRef::first_on(line)?),
            store: None,
        });
    }

    // A `ready list` row: `  weapon: (<a ...>a sword</a>) (stowed)`, or with
    // `none` where nothing is set, or with no store-mode at all for the
    // sheaths and the second ammo bundle.
    //
    // Indented, and carrying the row's own `<d cmd='store ...'>` or
    // `<d cmd='ready ...'>` link around the item or the `none`: every one of
    // Lich's three row patterns requires that command (`xmlparser.rb:522-524`),
    // and it is what makes the row a row of THIS listing.
    if !indented || !has_slot_command(line) {
        return None;
    }
    let (label, rest) = trimmed.split_once(": ")?;
    let slot = ReadySlot::parse(label)?;
    let store = trailing_parenthetical(rest).and_then(StoreMode::parse);
    Some(ContainerEvent::ReadySet {
        slot,
        item: ItemRef::first_on(line),
        store,
    })
}

/// Whether the line carries a `ready list` row's slot command:
/// `<d cmd='store WEAPON clear'>`, `<d cmd='ready SHIELD'>`. The store-mode
/// link, `<d cmd='store set'>`, is on every row too and is not this.
fn has_slot_command(line: &ChunkLine) -> bool {
    line.links().any(|link| match &link.kind {
        LinkKind::Direct { cmd } => {
            (cmd.starts_with("store ") || cmd.starts_with("ready ")) && cmd != "store set"
        }
        _ => false,
    })
}

/// The contents of a trailing `(...)`, if the line ends in one.
fn trailing_parenthetical(text: &str) -> Option<&str> {
    let inner = text.strip_suffix(')')?;
    let (_, last) = inner.rsplit_once('(')?;
    Some(last)
}

/// The two lists the game keeps: stow targets and ready items.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Containers {
    stow: Vec<(StowSlot, ItemRef)>,
    ready: Vec<(ReadySlot, ItemRef)>,
    store: Vec<(ReadySlot, StoreMode)>,
    stow_checked: bool,
    ready_checked: bool,
}

impl Containers {
    /// Apply one event. Returns whether anything changed.
    pub fn apply(&mut self, event: &ContainerEvent) -> bool {
        let before = self.clone();
        match event {
            // A restated list replaces the old one. Lich's `reset`
            // (`stowlist.rb:57`) for the same reason: a slot the game no
            // longer lists has been cleared, and keeping it would report a
            // container that is not a target any more.
            ContainerEvent::StowListBegins => {
                self.stow.clear();
                self.stow_checked = true;
            }
            ContainerEvent::ReadyListBegins => {
                self.ready.clear();
                self.store.clear();
                self.ready_checked = false;
            }
            // Lich sets `checked` at the START of `ready list` only via the
            // closing line (`xmlparser.rb:609`), which is the better rule: a
            // list cut off by a disconnect is not a list you have read.
            ContainerEvent::ReadyListEnds => self.ready_checked = true,
            ContainerEvent::StowSet { slot, item } => set(&mut self.stow, *slot, item.clone()),
            ContainerEvent::ReadySet { slot, item, store } => {
                // **A row with no item leaves the slot as it was**, as Lich
                // does: `unless match[:id].nil?` (`xmlparser.rb:600`) writes
                // only when the row named something. A `none` row inside a
                // listing finds the slot already emptied by the opener, so
                // nothing is lost; outside one, it is not evidence enough to
                // throw away an item a confirmation taught (review).
                if let Some(item) = item {
                    set(&mut self.ready, *slot, item.clone());
                }
                if let Some(store) = store {
                    set(&mut self.store, *slot, *store);
                }
            }
            // **Clearing an item clears its store-mode too.** Lich leaves the
            // mode behind (`xmlparser.rb:613` writes only the item), so a
            // slot with no item still reports where that item would be
            // stored -- an answer about nothing.
            ContainerEvent::ReadyCleared(slot) => {
                self.ready.retain(|(held, _)| held != slot);
                self.store.retain(|(held, _)| held != slot);
            }
            ContainerEvent::StoreModeSet { slot, store } => set(&mut self.store, *slot, *store),
        }
        before != *self
    }

    /// The container a kind of loot stows into, if the game has said.
    #[must_use]
    pub fn stow(&self, slot: StowSlot) -> Option<&ItemRef> {
        find(&self.stow, slot)
    }

    /// The item readied for a slot, if the game has said.
    #[must_use]
    pub fn ready(&self, slot: ReadySlot) -> Option<&ItemRef> {
        find(&self.ready, slot)
    }

    /// Where a slot's item goes when stored, if the game has said.
    #[must_use]
    pub fn store_mode(&self, slot: ReadySlot) -> Option<StoreMode> {
        find(&self.store, slot).copied()
    }

    /// Whether a whole `stow list` has been read.
    ///
    /// **Not "and the containers still exist"** -- see the module doc on
    /// `valid?`. This says the game stated the list; whether those ids are
    /// still in inventory is inventory's question.
    #[must_use]
    pub fn stow_checked(&self) -> bool {
        self.stow_checked
    }

    /// Whether a whole `ready list` has been read, closing line included.
    #[must_use]
    pub fn ready_checked(&self) -> bool {
        self.ready_checked
    }

    /// Forget everything, as `Infomon.reset` does.
    ///
    /// Both lists are per-character settings the game holds, so a reconnect
    /// does **not** clear them -- they are re-taught by the sync, not
    /// invalidated by the disconnect.
    pub fn reset(&mut self) {
        *self = Self::default();
    }
}

/// Set a key in an association list, replacing any value already held.
fn set<K: PartialEq, V>(list: &mut Vec<(K, V)>, key: K, value: V) {
    match list.iter_mut().find(|(held, _)| *held == key) {
        Some(slot) => slot.1 = value,
        None => list.push((key, value)),
    }
}

/// Look a key up in an association list.
fn find<K: PartialEq + Copy, V>(list: &[(K, V)], key: K) -> Option<&V> {
    list.iter()
        .find(|(held, _)| *held == key)
        .map(|(_, value)| value)
}
