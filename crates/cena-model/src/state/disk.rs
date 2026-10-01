//! Floating disks: whose they are, and what they are called.
//!
//! # A disk is a room object named for someone
//!
//! `Bagagwa disk`, `fiery red Vasstryke disk`, `four-toned Desorceri coffret`.
//! Eleven nouns carry them (`gemstone/disk.rb:4`), and the display text puts
//! the owner's name, capitalised, right before the noun -- which is what makes
//! a disk findable without the game ever labelling one as such. Lich reads it
//! so: `thing.name =~ /\b([A-Z][a-z]+) #{Regexp.union(NOUNS)}\b/`
//! (`disk.rb:7`). Here the noun comes typed off the link, so only the owner's
//! word is read: the word before the noun, `[A-Z][a-z]+` as Lich has it. A
//! name may not be any other shape:
//!
//! > **AUTHOR, 2026-09-20:** *"you're making up names now, Gemstone doesn't let
//! > you do McTavish or D'iel as a name."*
//!
//! > **CORRECTED 2026-10-01.** This read the owner off a possessive,
//! > `Ryeka's disk`, and every test here was written in that form. The game
//! > never sends one. MEASURED over Hydra's 45 wire logs of 2026-09-23 to
//! > 2026-09-30 (`logs/**/*.bytes`, every `<a ... noun="<disk noun>">...</a>`):
//! > 2,953 links, 83 distinct; **0** possessive, 76 a capitalised name before
//! > the noun (`Bagagwa disk`, `rusty iron Duffield disk`), and 7 furniture or
//! > loot (`iron-bound walnut chest`, `crimson sphere`). The author's own room
//! > in `tests/claim.rs` already said it: `faenor Demandred disk`. So this
//! > found no disk at all on the wire, and the hunt's rule that a stranger's
//! > disk holds a room (`hunt/engine.rs`, `hold_room`) never fired. The test
//! > was the bug's twin: an invented line agrees with the code that was
//! > written from it.

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
    /// Whose it is, the name before the noun: `Bagagwa disk` -> `Bagagwa`.
    pub owner: String,
}

impl Disk {
    /// Read a room object as a disk, or `None` if it is not one.
    ///
    /// Requires BOTH halves: a disk noun, and an owner's name before it. A
    /// plain `chest` sitting in a room is furniture, not somebody's disk, and
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

/// `"fiery red Vasstryke disk"` -> `"Vasstryke"`, given the noun: the word
/// before it, when it is a name, `[A-Z][a-z]+` (`disk.rb:7`).
fn owner_of(text: &str, noun: &str) -> Option<String> {
    let before = text.strip_suffix(noun)?.strip_suffix(' ')?;
    let owner = before.rsplit(' ').next()?;
    let mut letters = owner.chars();
    let named = letters
        .next()
        .is_some_and(|first| first.is_ascii_uppercase())
        && owner.len() > 1
        && letters.all(|letter| letter.is_ascii_lowercase());
    named.then(|| owner.to_owned())
}

impl crate::state::Room {
    /// Every disk in the room, in wire order.
    pub fn disks(&self) -> impl Iterator<Item = Disk> + '_ {
        self.objects.iter().filter_map(Disk::read)
    }

    /// The disk belonging to `owner`, if it is here.
    ///
    /// Exact, not a substring. Lich matches with `item.name.include?(name)`
    /// (`disk.rb:12`), so a character called `Rye` would find `Ryeka disk`
    /// and try to loot a stranger's property.
    #[must_use]
    pub fn disk_of(&self, owner: &str) -> Option<Disk> {
        self.disks().find(|disk| disk.owner == owner)
    }
}
