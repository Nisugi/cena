//! Hazards: what in a room hurts whoever stays, whether the game draws it as
//! an object or as a creature.
//!
//! # Two tables, because the game draws hazards two ways
//!
//! | drawn as | table | from, under `reference/lich_repo_mirror/lib/` |
//! |---|---|---|
//! | a room object, unbolded in `room objs` | [`OBJECTS`], 13 names | `creaturewindow.lic:778-786`; `hazardwindow.lic:93-99` has 11 of them |
//! | a creature, bolded, with a `<crtrStatus>` | [`CREATURES`], 2 names | `Ashborne.lic:17458-17467`; `huntpro.lic:2016-2026` |
//!
//! Neither Lich's `gameobj-data.xml` nor Cena's `data/gameobj-data.tsv` has
//! a hazard category (`cut -f2 gameobj-data.tsv | sort -u`), which is why
//! the scripts keep these lists. Ashborne's `gas cloud` object
//! (`Ashborne.lic:17329`) is one of [`OBJECTS`]' `cloud`.
//!
//! # The two creatures a hunt would otherwise attack
//!
//! Both are ordinary bestiary rows with hit points (`data/creatures.tsv`,
//! `shimmering_fungus` and `wasp_nest`), and `gameobj-data.tsv`'s
//! `aggressive npc` names both, so a target list holding `any` attacks them
//! (`inventory/12` §2). Three hunting scripts refuse them: Ashborne drops
//! them from its targets and leaves when nothing else is left
//! (`Ashborne.lic:17487-17502`), huntpro changes rooms
//! (`huntpro.lic:2016-2026`), and chuntpro walks away from a wasp nest
//! (`chuntpro.lic:5899-5902`). The fungus summons maw spores
//! (`creatures.tsv`'s `special_other`), and its shudder is one of Ashborne's
//! warnings that the ground is about to erupt (`Ashborne.lic:16560`).
//!
//! # NOT ported
//!
//! Ashborne's gas-cloud warning lines (`Ashborne.lic:17315-17336`: a cloud
//! forming above you, sparks, rumbling, dispersing). They are a room's
//! state over time, which the hunt's `flee.messages` already reads as
//! phrases (`inventory/12` §2, FIXED).

use std::sync::OnceLock;

use super::creatures::CreatureInstance;
use super::creatures::instance::without_boon;
use super::ledger::text::Pat;
use super::room::{Room, RoomItem};

/// The room objects creaturewindow calls hazards (`creaturewindow.lic:784`),
/// in its order. Each matches as whole words anywhere in the name, ignoring
/// case, so `cloud` is also a gas cloud and `web` a sticky web.
pub const OBJECTS: [&str; 13] = [
    "acidic cloud of mist",
    "glimmering boltstone apparatus",
    "pale hovering runestone",
    "cloud",
    "unearthly silvery blue globe",
    "spiraling ghostly rift",
    "sandstorm",
    "vine",
    "black metal bomb",
    "black void",
    "windy vortex",
    "web",
    "whirlwind",
];

/// The creatures a hunt never targets (`Ashborne.lic:17458-17460`), matched
/// as the whole name, ignoring case.
pub const CREATURES: [&str; 2] = ["shimmering fungus", "wasp nest"];

fn objects() -> &'static Pat {
    static P: OnceLock<Pat> = OnceLock::new();
    P.get_or_init(|| {
        let names = OBJECTS.map(regex::escape).join("|");
        Pat::new(&format!(r"(?i)\b(?:{names})\b"))
    })
}

/// Is a room object with this name a hazard? One of [`OBJECTS`] as whole
/// words, and not a disk: creaturewindow excludes any name holding `disk`
/// (`creaturewindow.lic:783`).
#[must_use]
pub fn is_object(name: &str) -> bool {
    !name.to_ascii_lowercase().contains("disk") && objects().is_match(name)
}

/// Is a creature with this display name a hazard? The whole name is one of
/// [`CREATURES`], ignoring case, as Ashborne compares it
/// (`Ashborne.lic:17462-17467`).
#[must_use]
pub fn is_creature(name: &str) -> bool {
    let name = name.trim();
    CREATURES.iter().any(|c| c.eq_ignore_ascii_case(name))
}

impl Room {
    /// The hazard objects lying in the room (creaturewindow's
    /// `current_hazards`, `creaturewindow.lic:778-786`). Empty is the
    /// ordinary answer, and also the answer before `room objs` has been seen.
    pub fn hazards(&self) -> impl Iterator<Item = &RoomItem> {
        self.objects.iter().filter(|item| is_object(&item.text))
    }
}

impl CreatureInstance {
    /// **Is it a hazard rather than a target?** A wasp nest or a shimmering
    /// fungus, by its name or its name with a boon adjective stripped, as the
    /// bestiary retries a lookup. `false` is every other creature: the list
    /// is two names long, so `false` means "not one of those", not "safe".
    #[must_use]
    pub fn hazard(&self) -> bool {
        is_creature(&self.name) || without_boon(&self.name).is_some_and(|n| is_creature(&n))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_object_pattern_compiles() {
        assert!(objects().compiled());
    }
}
