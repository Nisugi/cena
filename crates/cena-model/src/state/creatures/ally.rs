//! Someone's familiar, companion or summon: a creature that is nobody's
//! target, whatever a hunt's `any` says.
//!
//! # The gap this closes
//!
//! A hunt skips a creature the server calls unhostile (`hostile() ==
//! Some(false)`). One with no `<crtrStatus>` yet -- registered off a combat
//! line, `creatures.rs`'s deviation 2 -- reads `None`, and a target list
//! holding `any` attacks it (`inventory/12` §2). Neither Lich's
//! `gameobj-data.xml` nor Cena's copy has a `familiar` or `companion` type.
//! Two scripts patch them in, and these are their tables.
//!
//! # Sources, all under `reference/lich_repo_mirror/lib/`
//!
//! | [`Ally`] | read from | the table |
//! |---|---|---|
//! | `Familiar` | the name | 34 families: `xmlpatch.lic:127`, `:207-209`; `recolor.lic:740-745` |
//! | `Companion` | the noun | 128 nouns: `xmlpatch.lic:199-203`; `recolor.lic:747-753` |
//! | `Demon` | the name | minor demons and the illusions they wear: `recolor.lic:148-189` |
//! | `Passive` | name or noun | `data/gameobj-data.tsv`'s `passive npc`, friendly to `recolor.lic:338` |
//!
//! MEASURED with a scratch script: the familiar string, the companion nouns
//! and the companion exclusions are identical in the two scripts.
//!
//! **Spirit servants are `Passive`.** recolor keeps its own pattern for them
//! (`recolor.lic:124-145`, 93 demeanors by 39 kinds). Every one of the 3,627
//! names it spells is already `passive npc` in Cena's table, MEASURED, so it
//! is not ported a second time.
//!
//! # Three rules outrank the tables
//!
//! 1. **The server.** A creature the wire calls `hostile` is a foe, and
//!    [`CreatureInstance::ally`] answers `None` for it. It is recolor's own
//!    rule since 0.3: *"Creatures that are targetable ... will always be
//!    forced to hostile"* (`recolor.lic:68`, applied at `:345`).
//! 2. **recolor's named exceptions.** A name ending `abyran'ra` or `slush` is
//!    hostile (`recolor.lic:195`).
//! 3. **The bestiary.** A name with a template, a boon adjective stripped as
//!    the bestiary retries, is a creature of the world and nobody's.
//!    xmlpatch's companion `exclude` list is that rule, frozen in 2019:
//!    MEASURED, all 35 of its names are templates. Frozen, it went stale.
//!    Five templates carry a companion noun and are not in it (`mongrel
//!    wolfhound`, `storm hound`, `vapor hound`, `water hound`, `wharf rat`),
//!    and two templates spell a familiar (`large black cat`, `large
//!    ring-tailed lemur`). So the bestiary is the exclusion, and the list is
//!    not ported.
//!
//! # One deviation
//!
//! recolor's Igaesha pattern is `cloud\ of\.*\ (?:haze|...)$` under `/x`
//! (`recolor.lic:156-158`). `\.*` is a run of literal dots, so it matches
//! `cloud of haze` and never the `a cloud of {color} {subtype}` its own
//! comment describes and the wiki lists (`reference/wiki_clean/Igaesha.txt`,
//! "Possible Adjectives"). Ported as the comment says.
//!
//! # What stays UNVERIFIED
//!
//! Whether the game's own target list (`dDBTarget`, `targeting.rs`) already
//! leaves these out. recolor's rule 1 treats a friendly name on that list as
//! hostile, which is INFERRED evidence that the list leaves them out, not a
//! measurement.

use std::sync::OnceLock;

use super::instance::{CreatureInstance, without_boon};
use crate::state::creature::by_name;
use crate::state::creature::status::Classification;
use crate::state::ledger::text::Pat;

/// Whose a creature is, as far as its name and noun say.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Ally {
    /// A wizard's familiar, by its name.
    Familiar,
    /// A ranger's animal companion, by its noun.
    Companion,
    /// A sorcerer's minor demon, or the illusion it wears.
    Demon,
    /// `passive npc` in `gameobj-data.tsv`: spirit servants, and the town's
    /// harmless residents.
    Passive,
}

/// The familiar families: `xmlpatch.lic:127`'s `familiars`, one entry per
/// top-level alternative, joined back as `^(?:...)$` (`:209`).
/// Case-sensitive, as the source is.
const FAMILIARS: [&str; 34] = [
    r"(?:pink-cheeked|ruby-cheeked|croak-beaked|beady-eyed|green-faced|white-winged|white-faced|white-banded|grey-banded)?.*?(?:green|grey|yellow|albino) cockatiel",
    r"(?:short-nosed|bare-backed|little-collar|long-tailed|long-tongued)?.*?(?:small|large|fruit|spotted|pygmy) bat",
    r"(?:(?:blue-eyed|disheveled|fat|fluffy|green-eyed|long-haired|orange-eyed|plump|shabby|short-haired|sleek|emerald-eyed)?.*?(?:black|calico|charcoal|ginger|grey|tabby|white) cat|(?:silver-eyed pure white cat|silver-ruffed charcoal black cat))",
    r"(?:large|small)?.*?(?:black|brown|dwarf|forest|green|grey|leaf|mountain|ochre|spotted|veiled) chameleon",
    r"(?:iridescent)?.*?(?:purple-plumed) cormorant",
    r"lithe golden-eyed coyote",
    r"(?:large|ruffled|sleek|small)?.*?(?:long-winged|narrow-winged|red-footed|red-necked) falcon",
    r"(?:(?:disheveled|sleek)?.*?(?:arctic|bat-eared|black|blue|desert|grey|kit|red|swift) fox|black-footed plump red fox)",
    r"(?:(?:large|plump|slimy|small|speckled)?.*?(?:brown|bull|green|green tree|horned|tree) frog|heavy-set bull frog)",
    r"gyrfalcon",
    r"(?:black-tailed|white-tailed)?.*?(?:black|broom|cape|mountain|red|savanna|scrub|snowshoe|woolly) hare",
    r"(?:(?:large|ruffled|sleek|small)?.*?(?:black|black-mantled|grey-bellied|grey-headed|red-chested|red-tailed|spot-tailed|white-bellied) hawk|dappled broad-winged hawk)",
    r"rangy dappled grey jackal",
    r"(?:roly-poly)?.*?(?:flamepoint) sand kitten",
    r"(?:greater|lesser|sleek|small)?.*?(?:bamboo|black|brown|crowned|dwarf|fork-crowned|golden|fork-marked|grey|grey-headed|mouse|red-bellied|ring-tailed|red-collared|ruffled|white-collared) lemur",
    r"slim dove grey merlin",
    r"(?:fat|plump|sleek|tiny)?.*?(?:black|brown|field|forest|furry|pocket|grey|white)?.*?mouse",
    r"(?:(?:greater|lesser|ruffled|sleek|pale-faced)?.*?(?:barn|pygmy|barred|screech|bay|snowy|crested|sooty|eagle|speckled|elf|spectacled|grass|spotted|great white|striped|grey|tawny|hawk|white|horned|white-faced|maned|wood|masked) owl|dark-eyed grey barred owl)",
    r"(?:brilliant)?.*?parrot",
    r"(?:large|ruffled|small|sleek)?.*?(?:brown|pink-backed|white|spot-billed) pelican",
    r"(?:crested|emperor|king|royal|yellow-eyed|little blue) penguin",
    r"(?:rusty|hooded|red-brown|black|brown|crested) pitohui",
    r"rock ptarmigan",
    r"(?:(?:large|ruffled|small|sleek)?.*?raven|glossy coal black raven)",
    r"squat pebble-backed toad",
    r"(?:black-banded|ivory-billed|black-billed|red-billed|black-necked|red-necked|blue-banded|spot-billed|curl-crested|stripe-billed|fiery-billed|yellow-browed|grey-breasted|yellow-ridged|groove-billed)?.*?(?:blue|bright blue|collared|emerald|green|hooded|speckled)?.*?toucan",
    r"(?:(?:arctic|blue|great white|grey) wolf|amber-eyed grizzled timber wolf)",
    r"(?:cloudy-eyed|long-eared|ethereal|long-nosed|foggy-eyed|soft-featured|lithe|white-eyed)?.*?air wyrdling",
    r"(?:amber-eyed|large-nosed|crumbling|rough|crystal-eyed|round-eared|dusty|squat|dusty-eyed|stout)?.*?earth wyrdling",
    r"(?:angular|sharp-eared|ember-eyed|sharp-nosed|lanky|smoky|red-eyed|smoky-eyed|scaled|smoldering)?.*?fire wyrdling",
    r"(?:angular|nebulous|hook-nosed|scaled|jagged-eared|silver-eyed|lanky|spark-eyed)?.*?lightning wyrdling",
    r"(?:blue-eyed|misty-eyed|frothy-eyed|sleek|lithe|small-nosed|long-eared|smooth|misty)?.*?water wyrdling",
    r"(?:pale green|leafy green|grass green|russet brown|light viridian|jade green|emerald|deep green|spike-backed|short-snouted|long-snouted|three-tied|drab|aged|scaly|split-tailed)?.*?iguana",
    r"hummingbird",
];

/// The companion nouns, `xmlpatch.lic:201` in its order. Matched whole, as
/// its `^(?:...)$` does.
const COMPANION_NOUNS: [&str; 128] = [
    "albatross",
    "badger",
    "banishara",
    "barrowrat",
    "baza",
    "beaver",
    "boarrat",
    "bobcat",
    "burrowvine",
    "bushwag",
    "buzzard",
    "caiverine",
    "canid",
    "capybara",
    "caracal",
    "caracara",
    "catamount",
    "cavecail",
    "cheetah",
    "condor",
    "cougar",
    "coyote",
    "crow",
    "curhound",
    "curwolf",
    "cygnet",
    "dhole",
    "dobrem",
    "dole",
    "eagle",
    "elf-owl",
    "falcon",
    "felid",
    "fennec",
    "fenvaok",
    "ferret",
    "fox",
    "foxhound",
    "goshawk",
    "graiphel",
    "greatfang",
    "groundhog",
    "gyrefalcon",
    "gyrfalcon",
    "harrier",
    "hawk",
    "hawk-eagle",
    "hedgehog",
    "heron",
    "hound",
    "hyena",
    "jackal",
    "jaguar",
    "jaguarundi",
    "karet",
    "kestrel",
    "kingfisher",
    "kite",
    "kodkod",
    "kuvasz",
    "ledisa",
    "leopard",
    "lion",
    "loper",
    "lugger",
    "lynx",
    "margay",
    "marmot",
    "mastiff",
    "mink",
    "mole",
    "mongoose",
    "mudcat",
    "muskrat",
    "muzzlerat",
    "narmo",
    "nutria",
    "ocelot",
    "osprey",
    "owl",
    "panther",
    "passo",
    "peregrine",
    "pigeonhawk",
    "porcupine",
    "puma",
    "raccoon",
    "raptor",
    "rasper",
    "rat",
    "raven",
    "rockrat",
    "rowl",
    "saker",
    "samoyed",
    "sandrat",
    "scrabbler",
    "screech-owl",
    "sea-eagle",
    "seagull",
    "seahawk",
    "serval",
    "shrika",
    "shrike",
    "sloth",
    "snowcat",
    "snow-owl",
    "sparrowhawk",
    "stratis",
    "swift",
    "tiger",
    "tothis",
    "trakel",
    "tunnelcat",
    "veercat",
    "vole",
    "vulture",
    "weasel",
    "whiskrat",
    "wildcat",
    "wolf",
    "wolfhound",
    "wolverine",
    "wombat",
    "woodchuck",
    "woodpecker",
    "woodshrew",
    "yowler",
];

/// Minor demons by name, `recolor.lic:150-164`: case-insensitive and
/// unanchored at the start, as its `/ix` is.
const DEMONS: [&str; 3] = [
    // Simple demons: two words and the archetype, `abyran'ra`'s suffix allowed.
    r"(?i)\w \w+ (?:abyran|aishan|arashan|grantris|grik|imp|verlok)(?:'.*)?$",
    // Igaesha, `a cloud of {color} {subtype}`: the deviation in the module doc.
    r"(?i)cloud of \S.* (?:haze|rouk|brume|haar|murk|nyle|mist|smoke|vapor|fog)$",
    // Shien.
    r"(?i)(?:blurred|hazy|indistinct|misty|nebulous|shifting|vaporous) .* (?:darkling|shadowling)$",
];

/// The illusions a minor demon wears, by exact name: `recolor.lic:167-188`,
/// 33 entries with 4 repeated.
const ILLUSIONS: [&str; 29] = [
    "caped figure",
    "cloaked figure",
    "cowled figure",
    "masked figure",
    "robed figure",
    "coat-wrapped figure",
    "dark-mantled figure",
    "tethered hairy black tarantula",
    "bound large brown arachnid",
    "restrained enormous web weaver",
    "harnessed huge grey spinner",
    "shackled vast grey-blue spider",
    "slender chestnut brown stoat",
    "sleek beady-eyed mink",
    "long light brown weasel",
    "medium-sized lithe ferret",
    "small bushy-tailed ermine",
    "brightly plumed toucan",
    "large glossy raven",
    "lanky white stork",
    "old blue parrot",
    "small brown pelican",
    "aging dusty gnoll",
    "haggard grey-haired gnoll",
    "stooped and wizened gnoll",
    "portly wrinkled gnoll",
    "old fork-bearded gnoll",
    "thin elderly gnoll",
    "ancient bespectacled gnoll",
];

/// The wild-animal illusions, `recolor.lic:189`.
const ILLUSION_PATTERN: &str =
    r"^(?:lazy wild .+ dog|listless .+ jackal|docile .+ coyote|languid .+ wolf|mangy .+ fox)$";

/// Names recolor never calls friendly, `recolor.lic:195`.
const HOSTILE: &str = r"(?:abyran'ra|slush)$";

struct Tables {
    familiar: Pat,
    demons: [Pat; 3],
    illusion: Pat,
    hostile: Pat,
}

fn tables() -> &'static Tables {
    static T: OnceLock<Tables> = OnceLock::new();
    T.get_or_init(|| Tables {
        familiar: Pat::new(&format!("^(?:{})$", FAMILIARS.join("|"))),
        demons: DEMONS.map(Pat::new),
        illusion: Pat::new(ILLUSION_PATTERN),
        hostile: Pat::new(HOSTILE),
    })
}

/// Whose a creature is by its display name and noun. `None`: nothing says
/// it is anyone's, which is not proof that it is a foe.
///
/// Stateless: [`CreatureInstance::ally`] adds the server's word.
#[must_use]
pub fn classify(name: &str, noun: Option<&str>) -> Option<Ally> {
    let t = tables();
    if t.hostile.is_match(name) || in_bestiary(name) {
        return None;
    }
    if t.familiar.is_match(name) {
        return Some(Ally::Familiar);
    }
    if noun.is_some_and(|n| COMPANION_NOUNS.contains(&n)) {
        return Some(Ally::Companion);
    }
    if ILLUSIONS.contains(&name)
        || t.illusion.is_match(name)
        || t.demons.iter().any(|d| d.is_match(name))
    {
        return Some(Ally::Demon);
    }
    crate::state::gameobj::classify(noun.unwrap_or_default(), name)
        .is("passive npc")
        .then_some(Ally::Passive)
}

/// A bestiary template has this name, or has it with a boon adjective
/// stripped.
fn in_bestiary(name: &str) -> bool {
    !by_name(name).is_empty() || without_boon(name).is_some_and(|n| !by_name(&n).is_empty())
}

impl CreatureInstance {
    /// **Whose is it?** Someone's familiar, companion, summon or a passive
    /// NPC by its name and noun, looked up when it was first seen.
    ///
    /// `None` when the server has called it `hostile`, whatever its name
    /// (rule 1 in the module doc), and when nothing says it is anyone's.
    /// So `None` is not proof of a foe: it is the ordinary answer for every
    /// creature a hunt fights.
    #[must_use]
    pub fn ally(&self) -> Option<Ally> {
        if self.flag(Classification::Hostile) {
            return None;
        }
        self.ally
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_pattern_compiles() {
        let t = tables();
        assert!(t.familiar.compiled() && t.illusion.compiled() && t.hostile.compiled());
        assert!(t.demons.iter().all(Pat::compiled));
    }
}
