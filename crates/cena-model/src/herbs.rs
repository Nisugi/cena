//! The herbs: every restorative eherbs knows, what it treats and where it is
//! sold (`plan/36` Stage 1).
//!
//! Cut whole from eherbs' `known_herbs` (`eherbs.lic:827-1113`) by
//! `tools/extract_herbs.rb` into `data/herbs.tsv`: 247 herbs, each with its
//! name as a shop sells it, the shorter name eherbs also matches, what it
//! treats, the doses a store-bought one holds, and the places it comes from.
//! Most places are towns with a herbalist; four are eherbs' markers for a
//! herb no shop sells ([`Source`]).
//!
//! A static table, parsed once on first use and never mutated, like the
//! spells and the bestiary.

use std::fmt;
use std::sync::OnceLock;

const HERBS_TSV: &str = include_str!("../data/herbs.tsv");

/// How bad an injury is: eherbs' `major` and `minor`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Severity {
    /// Rank 2 or 3.
    Major,
    /// Rank 1.
    Minor,
}

/// Where on the body a herb works, in eherbs' four words.
///
/// Lich's five wound areas fold into these (`eherbs.lic:486`): head and neck
/// are `head`, the torso (with the eyes) is `organ`, the limbs `limb`, the
/// nervous system `nerve`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Area {
    /// Head and neck.
    Head,
    /// Chest, abdomen, back and eyes.
    Organ,
    /// Arms, hands and legs.
    Limb,
    /// The nervous system.
    Nerve,
}

/// A wound, or the scar one leaves.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Hurt {
    /// A fresh wound.
    Wound,
    /// A scar.
    Scar,
}

/// What a herb treats: eherbs' `type`, a closed vocabulary of 23.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum HerbKind {
    /// Lost health: acantha and its kin.
    Blood,
    /// One of the sixteen wound and scar kinds.
    Injury {
        /// Major or minor.
        severity: Severity,
        /// Where.
        area: Area,
        /// Wound or scar.
        hurt: Hurt,
    },
    /// A severed limb: sovyn clove.
    SeveredLimb,
    /// A missing eye: bur-clover.
    MissingEye,
    /// Poison.
    Poison,
    /// Disease.
    Disease,
    /// A lifekeeping herb (for the dead).
    Lifekeep,
    /// Raising the dead.
    RaiseDead,
}

impl HerbKind {
    /// Read eherbs' word for it: `blood`, `major head wound`, `missing eye`.
    #[must_use]
    pub fn parse(word: &str) -> Option<Self> {
        Some(match word {
            "blood" => Self::Blood,
            "severed limb" => Self::SeveredLimb,
            "missing eye" => Self::MissingEye,
            "poison" => Self::Poison,
            "disease" => Self::Disease,
            "lifekeep" => Self::Lifekeep,
            "raisedead" => Self::RaiseDead,
            _ => {
                let mut words = word.split(' ');
                let severity = match words.next()? {
                    "major" => Severity::Major,
                    "minor" => Severity::Minor,
                    _ => return None,
                };
                let area = match words.next()? {
                    "head" => Area::Head,
                    "organ" => Area::Organ,
                    "limb" => Area::Limb,
                    "nerve" => Area::Nerve,
                    _ => return None,
                };
                let hurt = match words.next()? {
                    "wound" => Hurt::Wound,
                    "scar" => Hurt::Scar,
                    _ => return None,
                };
                if words.next().is_some() {
                    return None;
                }
                Self::Injury {
                    severity,
                    area,
                    hurt,
                }
            }
        })
    }

    /// Every kind, in eherbs' own listing order for the injuries.
    #[must_use]
    pub fn all() -> Vec<Self> {
        let mut out = vec![Self::Blood];
        for area in [Area::Head, Area::Organ, Area::Limb, Area::Nerve] {
            for (severity, hurt) in [
                (Severity::Major, Hurt::Wound),
                (Severity::Minor, Hurt::Wound),
                (Severity::Major, Hurt::Scar),
                (Severity::Minor, Hurt::Scar),
            ] {
                out.push(Self::Injury {
                    severity,
                    area,
                    hurt,
                });
            }
        }
        out.extend([
            Self::SeveredLimb,
            Self::MissingEye,
            Self::Poison,
            Self::Disease,
            Self::Lifekeep,
            Self::RaiseDead,
        ]);
        out
    }
}

impl fmt::Display for HerbKind {
    /// eherbs' word, as [`HerbKind::parse`] reads it.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Blood => f.write_str("blood"),
            Self::SeveredLimb => f.write_str("severed limb"),
            Self::MissingEye => f.write_str("missing eye"),
            Self::Poison => f.write_str("poison"),
            Self::Disease => f.write_str("disease"),
            Self::Lifekeep => f.write_str("lifekeep"),
            Self::RaiseDead => f.write_str("raisedead"),
            Self::Injury {
                severity,
                area,
                hurt,
            } => {
                let severity = match severity {
                    Severity::Major => "major",
                    Severity::Minor => "minor",
                };
                let area = match area {
                    Area::Head => "head",
                    Area::Organ => "organ",
                    Area::Limb => "limb",
                    Area::Nerve => "nerve",
                };
                let hurt = match hurt {
                    Hurt::Wound => "wound",
                    Hurt::Scar => "scar",
                };
                write!(f, "{severity} {area} {hurt}")
            }
        }
    }
}

/// Where a herb comes from.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Source {
    /// A town's herbalist, by the location name the map gives the town
    /// (`the town of Wehnimer's Landing`, `Ta'Illistim`).
    Shop(&'static str),
    /// eherbs' `Do Not Buy`: a backroom bundle, known so it can be used.
    DoNotBuy,
    /// Found by foraging only.
    Forageable,
    /// Skinned from a creature only.
    Skinnable,
    /// Made by alchemy only.
    Alchemical,
}

impl Source {
    fn parse(text: &'static str) -> Self {
        match text {
            "Do Not Buy" => Self::DoNotBuy,
            "Forageable" => Self::Forageable,
            "Skinnable" => Self::Skinnable,
            "Alchemical" => Self::Alchemical,
            shop => Self::Shop(shop),
        }
    }
}

/// One herb.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Herb {
    /// The name as a shop sells it: `some acantha leaf`.
    pub name: &'static str,
    /// The name eherbs also matches it by.
    pub short_name: &'static str,
    /// What it treats.
    pub kind: HerbKind,
    /// The doses a store-bought one holds.
    pub store_doses: u32,
    /// Where it comes from, in eherbs' order.
    pub sources: Vec<Source>,
}

impl Herb {
    /// Is it drunk rather than eaten? eherbs' rule, a word of the name
    /// (`:365`, `\b(?:potion|tea|elixir|brew|tincture|ale|soup|porter)\b`).
    #[must_use]
    pub fn is_drinkable(&self) -> bool {
        is_drinkable(self.name)
    }

    /// A yabathilium-class restorative: eherbs' major blood (`:207`).
    #[must_use]
    pub fn is_major_blood(&self) -> bool {
        self.kind == HerbKind::Blood && MAJOR_BLOOD.contains(&self.short_name)
    }

    /// Does the herbalist at this location sell it? eherbs matches a place
    /// that contains the location's name (`:251`, `:2257`).
    #[must_use]
    pub fn sold_in(&self, location: &str) -> bool {
        !location.is_empty()
            && self
                .sources
                .iter()
                .any(|source| matches!(source, Source::Shop(place) if place.contains(location)))
    }

    /// Is this item this herb? eherbs matches the item's name inside the
    /// herb's name or its short name (`h[:name] =~ /#{i.name}/`, `:1924`).
    #[must_use]
    pub fn matches(&self, item_name: &str) -> bool {
        !item_name.is_empty()
            && (self.name.contains(item_name) || self.short_name.contains(item_name))
    }
}

/// eherbs' major-blood short names (`MAJOR_BLOOD_SHORT_NAMES`, `:197-204`).
const MAJOR_BLOOD: &[&str] = &[
    "yabathilium fruit",
    "tincture of yabathilium",
    "Bloody Krolvin ale",
    "Olak's Ol'style ale",
    "green mushroom potion",
    "sassafras tea",
];

/// The words that make a herb drunk rather than eaten.
const DRINKABLE: &[&str] = &[
    "potion", "tea", "elixir", "brew", "tincture", "ale", "soup", "porter",
];

/// eherbs' drinkable rule over any name: one of eight words, whole, any case.
#[must_use]
pub fn is_drinkable(name: &str) -> bool {
    name.split(|c: char| !c.is_alphanumeric())
        .any(|word| DRINKABLE.iter().any(|d| word.eq_ignore_ascii_case(d)))
}

/// eherbs' herbs that do not bundle (`cant_bundle`, `:474`): a name ending
/// in one of these words.
#[must_use]
pub fn bundles(name: &str) -> bool {
    const CANT: &[&str] = &[
        "tart", "feather", "special", "blubber", "pie", "porridge", "soup", "fruit",
    ];
    !name
        .rsplit(' ')
        .next()
        .is_some_and(|last| CANT.iter().any(|c| last.eq_ignore_ascii_case(c)))
}

/// Every herb, in eherbs' order.
#[must_use]
pub fn herbs() -> &'static [Herb] {
    static TABLE: OnceLock<Vec<Herb>> = OnceLock::new();
    TABLE.get_or_init(parse)
}

/// The herb this item is, by eherbs' match: the first whose name or short
/// name contains the item's name.
#[must_use]
pub fn herb_for(item_name: &str) -> Option<&'static Herb> {
    let exact = herbs()
        .iter()
        .find(|h| h.name == item_name || h.short_name == item_name);
    exact.or_else(|| herbs().iter().find(|h| h.matches(item_name)))
}

/// What the herbalist at this location sells.
pub fn sold_in(location: &str) -> impl Iterator<Item = &'static Herb> + '_ {
    herbs().iter().filter(move |h| h.sold_in(location))
}

/// Whether any herb is sold at this location (`location_stocked?`, `:249`).
#[must_use]
pub fn location_stocked(location: &str) -> bool {
    sold_in(location).next().is_some()
}

fn parse() -> Vec<Herb> {
    HERBS_TSV
        .lines()
        .filter(|line| !line.starts_with('#') && !line.starts_with("name\t"))
        .filter_map(|line| {
            let mut cols = line.split('\t');
            let name = cols.next()?;
            let short_name = cols.next()?;
            let kind = HerbKind::parse(cols.next()?)?;
            let store_doses = cols.next()?.parse().ok()?;
            let sources = cols.next()?.split('|').map(Source::parse).collect();
            Some(Herb {
                name,
                short_name,
                kind,
                store_doses,
                sources,
            })
        })
        .collect()
}
