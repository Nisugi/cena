//! Floating disks: whose they are, and what they are called.
//!
//! # A disk is a room object whose name is someone's
//!
//! `Ryeka's disk`, `Nisugi's chest`. Eleven nouns carry them
//! (`gemstone/disk.rb:4`), and the display text is a possessive of the owner's
//! name -- which is what makes a disk findable without the game ever labelling
//! one as such.
//!
//! # Where this differs from the port
//!
//! Lich tests `thing.name =~ /\b([A-Z][a-z]+) #{Regexp.union(NOUNS)}\b/`
//! (`disk.rb:7`) -- a regex over the DISPLAY NAME, because `GameObj` carries
//! the noun as text and Lich re-derives it. Cena's [`RoomItem`] already carries
//! `noun` typed off the link, so the noun half is a set lookup and only the
//! owner's name needs reading. §3a's bargain: the parser kept what the markup
//! encoded, so the classifier does not re-tokenize it.
//!
//! **That is a simplification, not a fix**, and an earlier draft of this file
//! claimed otherwise. It said Lich's `[A-Z][a-z]+` misses owners with an
//! apostrophe or a second capital, and offered `D'iel` and `McTavish` as
//! examples:
//!
//! > **AUTHOR, 2026-09-20:** *"you're making up names now, Gemstone doesn't let
//! > you do McTavish or D'iel as a name."*
//!
//! Quite so. `[A-Z][a-z]+` is not a defect in Lich; it is a correct encoding of
//! what the game permits, and the "bug" was invented to make a difference look
//! like an improvement. What is left is the real reason: reading a typed noun
//! is simpler than re-deriving one from display text, and it happens to place
//! no constraint on the name's shape.

use super::room::RoomItem;

/// The eleven nouns a floating disk can be.
///
/// `gemstone/disk.rb:4`, verbatim and in its order. A twelfth would be a
/// visible change here rather than a silent miss.
pub const DISK_NOUNS: [&str; 11] = [
    "cassone", "chest", "coffer", "coffin", "coffret", "disk", "hamper", "saucer", "sphere",
    "trunk", "tureen",
];

/// One floating disk seen in the room.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Disk {
    /// `exist=`, verbatim -- what a command targets.
    pub id: String,
    /// The noun, one of [`DISK_NOUNS`].
    pub noun: String,
    /// Whose it is, read off the possessive: `Ryeka's disk` -> `Ryeka`.
    pub owner: String,
}

impl Disk {
    /// Read a room object as a disk, or `None` if it is not one.
    ///
    /// Requires BOTH halves: a disk noun, and a possessive owner. A plain
    /// `chest` sitting in a room is furniture, not somebody's disk, and
    /// treating it as one would have a behavior try to loot the scenery.
    #[must_use]
    pub fn read(item: &RoomItem) -> Option<Self> {
        if !DISK_NOUNS.contains(&item.noun.as_str()) {
            return None;
        }
        let owner = owner_of(&item.text, &item.noun)?;
        Some(Self {
            id: item.id.clone(),
            noun: item.noun.clone(),
            owner,
        })
    }
}

/// `"Ryeka's disk"` -> `"Ryeka"`, given the noun.
///
/// The owner is what precedes `'s ` before the noun. Read from the noun
/// backwards rather than by splitting on the first apostrophe, because a name
/// may contain one -- `D'iel's disk` is two apostrophes and the owner is
/// `D'iel`.
fn owner_of(text: &str, noun: &str) -> Option<String> {
    let before = text.strip_suffix(noun)?.trim_end();
    let owner = before.strip_suffix("'s")?.trim();
    // A leading article means it is not a possessive at all: `a wooden chest`
    // has no owner, and `the chest` is furniture.
    if owner.is_empty() {
        return None;
    }
    Some(owner.to_owned())
}

impl crate::state::Room {
    /// Every disk in the room, in wire order.
    pub fn disks(&self) -> impl Iterator<Item = Disk> + '_ {
        self.objects.iter().filter_map(Disk::read)
    }

    /// The disk belonging to `owner`, if it is here.
    ///
    /// Exact, not a substring. Lich matches with `item.name.include?(name)`
    /// (`disk.rb:12`), so a character called `Rye` would find `Ryeka's disk`
    /// and try to loot a stranger's property.
    #[must_use]
    pub fn disk_of(&self, owner: &str) -> Option<Disk> {
        self.disks().find(|disk| disk.owner == owner)
    }
}
