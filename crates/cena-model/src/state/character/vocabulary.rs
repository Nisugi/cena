//! The closed vocabularies of a character sheet.
//!
//! `research/04-inherited-decisions.md:1878` (C21) asks for "typed named fields
//! for the ~120 closed-vocabulary values". These are the sets that are genuinely
//! closed -- the game has a fixed list and the wire spells them exactly one way.
//!
//! # Where these came from
//!
//! **Every set below is the alternation of a Lich regex, transcribed verbatim.**
//! Lich stores them as bare strings, so the enum is Cena's addition, not a port:
//! it makes a typo a compile error and an unknown value a visible `None` rather
//! than a row that silently never matches.
//!
//! What is NOT here, and deliberately:
//!
//! * **Race** -- `parser.rb:10` captures `(?<race>[A-z]+|[A-z]+(?: |-)[A-z]+)`.
//!   No race list exists anywhere in Lich. Genuinely open.
//! * **Profession** -- captured as `(?<profession>[-A-z]+)`, and compared as a
//!   bare string exactly once in the whole tree (`spellsong.rb:27`,
//!   `Stats.prof != 'Bard'`). Also unenumerated.
//! * **Citizenship town** -- `(?<town>.*)`. Free text.
//!
//! Those three stay `String`. Inventing an enum for them would turn a new
//! Simutronics race into a parse failure, which is worse than a string.
//!
//! # The pattern
//!
//! Each enum carries the `crit/types.rs` trio -- `ALL`, `as_str`, `parse` -- so
//! round-tripping is testable and the list lives in one place. `parse` is an
//! inherent fn returning `Option<Self>`, matching `DamageType::parse`
//! (`crit/types.rs:116`) rather than `FromStr`, because a wire value that is not
//! in the set is data we have not seen, not an error to propagate.

use std::fmt;

/// How badly death has worn on the character.
///
/// `parser.rb:19`, the `TotalExp` regex:
/// `Death's Sting: (?<deaths_sting>None|Light|Moderate|Sharp|Harsh|Piercing|Crushing)`.
///
/// Ordered as the game orders it, least to worst, and `Ord` follows that order
/// so a behavior can ask whether the sting is worse than `Light` without a
/// lookup table.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum DeathsSting {
    None,
    Light,
    Moderate,
    Sharp,
    Harsh,
    Piercing,
    Crushing,
}

impl DeathsSting {
    /// Every variant, in the game's own severity order.
    pub const ALL: [Self; 7] = [
        Self::None,
        Self::Light,
        Self::Moderate,
        Self::Sharp,
        Self::Harsh,
        Self::Piercing,
        Self::Crushing,
    ];

    /// The wire spelling.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::None => "None",
            Self::Light => "Light",
            Self::Moderate => "Moderate",
            Self::Sharp => "Sharp",
            Self::Harsh => "Harsh",
            Self::Piercing => "Piercing",
            Self::Crushing => "Crushing",
        }
    }

    /// Parse the wire spelling.
    #[must_use]
    pub fn parse(text: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|v| v.as_str() == text)
    }
}

impl fmt::Display for DeathsSting {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// The three societies a character may belong to.
///
/// `parser.rb:38`, the `Society` regex:
/// `the (?<society>Order of Voln|Council of Light|Guardians of Sunfist)`.
///
/// **"Not a member" is absence, not a variant.** Lich stores the string
/// `'None'` in `society.status` (`parser.rb:410`), which is a sentinel that a
/// reader must know to special-case; `Option<Society>` says the same thing in
/// the type. See [`Society::max_rank`] for the one piece of arithmetic the
/// societies do not share.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Society {
    OrderOfVoln,
    CouncilOfLight,
    GuardiansOfSunfist,
}

impl Society {
    /// Every society.
    pub const ALL: [Self; 3] = [
        Self::OrderOfVoln,
        Self::CouncilOfLight,
        Self::GuardiansOfSunfist,
    ];

    /// The wire spelling.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::OrderOfVoln => "Order of Voln",
            Self::CouncilOfLight => "Council of Light",
            Self::GuardiansOfSunfist => "Guardians of Sunfist",
        }
    }

    /// Parse the wire spelling.
    #[must_use]
    pub fn parse(text: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|v| v.as_str() == text)
    }

    /// The rank a Master holds, which differs by society.
    ///
    /// **The wire does not send it.** `parser.rb:38`'s `(?<rank>[0-9]+)` group
    /// is optional and is absent for a Master -- the line reads "You are a
    /// Master in the Order of Voln." with no number -- so the rank has to be
    /// supplied from knowledge of the game. Lich does this at
    /// `parser.rb:400-407`: Voln 26, the other two 20.
    ///
    /// That asymmetry is real (Voln has 26 steps, the others 20) and is the
    /// reason this is a method rather than one constant.
    #[must_use]
    pub const fn max_rank(self) -> u8 {
        match self {
            Self::OrderOfVoln => 26,
            Self::CouncilOfLight | Self::GuardiansOfSunfist => 20,
        }
    }
}

impl fmt::Display for Society {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// The per-profession resource, one each.
///
/// `parser.rb:51-52`, the `Resource` and `Suffused` regexes, which share this
/// alternation.
///
/// **The `Resource` line does not say which one it is** -- its alternation is a
/// non-capturing group, so the type is only learned from a `Suffused` line. A
/// character with no suffused resource has amounts but no type, which is why
/// the type is stored separately and independently optional.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ResourceType {
    Essence,
    NecroticEnergy,
    LoreKnowledge,
    MotesOfTranquility,
    Devotion,
    NaturesGrace,
    Grit,
    LuckInspiration,
    Guile,
    Vitality,
}

impl ResourceType {
    /// Every resource type, in the wire's own alternation order.
    pub const ALL: [Self; 10] = [
        Self::Essence,
        Self::NecroticEnergy,
        Self::LoreKnowledge,
        Self::MotesOfTranquility,
        Self::Devotion,
        Self::NaturesGrace,
        Self::Grit,
        Self::LuckInspiration,
        Self::Guile,
        Self::Vitality,
    ];

    /// The wire spelling.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Essence => "Essence",
            Self::NecroticEnergy => "Necrotic Energy",
            Self::LoreKnowledge => "Lore Knowledge",
            Self::MotesOfTranquility => "Motes of Tranquility",
            Self::Devotion => "Devotion",
            Self::NaturesGrace => "Nature's Grace",
            Self::Grit => "Grit",
            Self::LuckInspiration => "Luck Inspiration",
            Self::Guile => "Guile",
            Self::Vitality => "Vitality",
        }
    }

    /// Parse the wire spelling.
    #[must_use]
    pub fn parse(text: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|v| v.as_str() == text)
    }
}

impl fmt::Display for ResourceType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// The six Warrior Guild war cries.
///
/// `parser.rb:43`, the `Warcries` regex.
///
/// # Lich stores these under two different key spellings
///
/// Learning one writes `warcry.<second word>` -- `parser.rb:379` does
/// `match[:name].split(' ')[1]`, so "Bertrandt's Bellow" becomes
/// `warcry.bellow`. Not being a warrior zeroes a *different* set,
/// `warcry.bertrandts_bellow` and friends (`parser.rb:369-374`).
///
/// The two never meet, so in Lich a learned war cry can never be un-learned:
/// the zeroing writes keys the reader does not consult. A named variant has one
/// identity and the bug cannot be expressed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Warcry {
    BertrandtsBellow,
    YertiesYowlp,
    GerrellesGrowl,
    SeanettesShout,
    CarnsCry,
    HorlandsHoller,
}

impl Warcry {
    /// Every war cry, in the wire's own alternation order.
    pub const ALL: [Self; 6] = [
        Self::BertrandtsBellow,
        Self::YertiesYowlp,
        Self::GerrellesGrowl,
        Self::SeanettesShout,
        Self::CarnsCry,
        Self::HorlandsHoller,
    ];

    /// The wire spelling, as the `warcry` command prints it.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::BertrandtsBellow => "Bertrandt's Bellow",
            Self::YertiesYowlp => "Yertie's Yowlp",
            Self::GerrellesGrowl => "Gerrelle's Growl",
            Self::SeanettesShout => "Seanette's Shout",
            Self::CarnsCry => "Carn's Cry",
            Self::HorlandsHoller => "Horland's Holler",
        }
    }

    /// The one-word command form, e.g. `bellow`.
    ///
    /// This is Lich's `warcry.<x>` key suffix and the PSM `:short_name` in
    /// `psms/warcry.rb`. Kept because it is what the player types.
    #[must_use]
    pub const fn short_name(self) -> &'static str {
        match self {
            Self::BertrandtsBellow => "bellow",
            Self::YertiesYowlp => "yowlp",
            Self::GerrellesGrowl => "growl",
            Self::SeanettesShout => "shout",
            Self::CarnsCry => "cry",
            Self::HorlandsHoller => "holler",
        }
    }

    /// Parse the wire spelling.
    #[must_use]
    pub fn parse(text: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|v| v.as_str() == text)
    }
}

impl fmt::Display for Warcry {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// The five PSM categories.
///
/// > **AUTHOR, 2026-09-19:** *"there's a few .. cman, shield, weapon, armor,
/// > feat are psms"*
///
/// This enum had **six** variants, including `Ascension`, taken from
/// `infomon/cli.rb`'s sync list where six `<x> list all` commands sit together.
/// That grouping is Lich's sync convenience, not the game's taxonomy, and the
/// wire agrees with the author -- see [`AscensionTable`](super::psm::AscensionTable) for the three ways
/// ascension's output differs.
///
/// Each PSM has its own `<category> list all` command and its own rank table.
///
/// **The header phrase and the key prefix differ**, and not uniformly:
/// "Combat Maneuvers" is stored under `cman`. Lich maps between them with a
/// regex `case` (`parser.rb:205-220`); here they are two methods on one value,
/// so the pair cannot drift apart.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum PsmCategory {
    Armor,
    CombatManeuver,
    Feat,
    Shield,
    Weapon,
}

impl PsmCategory {
    /// Every category.
    pub const ALL: [Self; 5] = [
        Self::Armor,
        Self::CombatManeuver,
        Self::Feat,
        Self::Shield,
        Self::Weapon,
    ];

    /// The key prefix, e.g. `cman` (`parser.rb:205-220`, lowercased).
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Armor => "armor",
            Self::CombatManeuver => "cman",
            Self::Feat => "feat",
            Self::Shield => "shield",
            Self::Weapon => "weapon",
        }
    }

    /// The phrase the game prints in the `list all` header.
    ///
    /// `parser.rb:29`, the `PSMStart` regex: "the following
    /// `<heading>` are available:".
    #[must_use]
    pub const fn heading(self) -> &'static str {
        match self {
            Self::Armor => "Armor Specializations",
            Self::CombatManeuver => "Combat Maneuvers",
            Self::Feat => "Feats",
            Self::Shield => "Shield Specializations",
            Self::Weapon => "Weapon Techniques",
        }
    }

    /// Parse the key prefix.
    #[must_use]
    pub fn parse(text: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|v| v.as_str() == text)
    }

    /// Parse the `list all` header phrase.
    #[must_use]
    pub fn parse_heading(text: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|v| v.heading() == text)
    }
}

impl fmt::Display for PsmCategory {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// The account's subscription tier.
///
/// `parser.rb:78-79` reads `F2P|Standard|Premium|Platinum` off the wire.
///
/// # This is four wire values, not three
///
/// Lich renames on write (`parser.rb:579-580`): `F2P` -> `Free`, `Standard` ->
/// `Normal`, and **`Platinum` -> `Premium`**, upcased. So `account.type` has
/// three possible values and a Platinum subscriber is indistinguishable from a
/// Premium one -- a lossy collapse with no way back.
///
/// Cena keeps all four. Platinum is a different game instance with different
/// mechanics, so conflating it with Premium loses a fact a behavior may need.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum AccountType {
    Free,
    Normal,
    Premium,
    Platinum,
}

impl AccountType {
    /// Every tier.
    pub const ALL: [Self; 4] = [Self::Free, Self::Normal, Self::Premium, Self::Platinum];

    /// The canonical name.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Free => "Free",
            Self::Normal => "Normal",
            Self::Premium => "Premium",
            Self::Platinum => "Platinum",
        }
    }

    /// Parse the **wire** spelling, which is not the canonical name.
    ///
    /// The wire says `F2P` and `Standard` where this type says `Free` and
    /// `Normal`; both spellings are accepted so a caller need not know which
    /// side it is on.
    #[must_use]
    pub fn parse(text: &str) -> Option<Self> {
        match text {
            "F2P" | "Free" => Some(Self::Free),
            "Standard" | "Normal" => Some(Self::Normal),
            "Premium" => Some(Self::Premium),
            "Platinum" => Some(Self::Platinum),
            _ => None,
        }
    }
}

impl fmt::Display for AccountType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// The seventeen Chronicles of Elanthia Houses (CHE).
///
/// `parser.rb:82`, the `ProfileHouseCHE` regex -- whose alternation is repeated
/// verbatim in three separate patterns there (`:82`, `:83`, `:84`), one per way
/// of learning the house. Transcribed once here.
///
/// **Absence is `Option::None`, not a variant.** Lich stores the string
/// `'none'` (`parser.rb:596`) for "No House affiliation", the same sentinel
/// pattern as `society.status`.
///
/// The wire form reaches Lich through `Lich::Util.normalize_name`, so the
/// stored string is already canonical -- which means this set *is* the stored
/// vocabulary, not merely the wire's.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Che {
    ArgentAspis,
    RisingPhoenix,
    Paupers,
    ArcaneMasters,
    Brigatta,
    TwilightHall,
    SilvergateInn,
    Sovyn,
    Sylvanfair,
    HeldenHall,
    WhiteHaven,
    BeaconHall,
    RoneAcademy,
    WillowHall,
    MoonstoneAbbey,
    ObsidianTower,
    CairnfangManor,
}

impl Che {
    /// Every House, in the regex's own order.
    pub const ALL: [Self; 17] = [
        Self::ArgentAspis,
        Self::RisingPhoenix,
        Self::Paupers,
        Self::ArcaneMasters,
        Self::Brigatta,
        Self::TwilightHall,
        Self::SilvergateInn,
        Self::Sovyn,
        Self::Sylvanfair,
        Self::HeldenHall,
        Self::WhiteHaven,
        Self::BeaconHall,
        Self::RoneAcademy,
        Self::WillowHall,
        Self::MoonstoneAbbey,
        Self::ObsidianTower,
        Self::CairnfangManor,
    ];

    /// The wire spelling.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::ArgentAspis => "Argent Aspis",
            Self::RisingPhoenix => "Rising Phoenix",
            Self::Paupers => "Paupers",
            Self::ArcaneMasters => "Arcane Masters",
            Self::Brigatta => "Brigatta",
            Self::TwilightHall => "Twilight Hall",
            Self::SilvergateInn => "Silvergate Inn",
            Self::Sovyn => "Sovyn",
            Self::Sylvanfair => "Sylvanfair",
            Self::HeldenHall => "Helden Hall",
            Self::WhiteHaven => "White Haven",
            Self::BeaconHall => "Beacon Hall",
            Self::RoneAcademy => "Rone Academy",
            Self::WillowHall => "Willow Hall",
            Self::MoonstoneAbbey => "Moonstone Abbey",
            Self::ObsidianTower => "Obsidian Tower",
            Self::CairnfangManor => "Cairnfang Manor",
        }
    }

    /// Parse the wire spelling.
    #[must_use]
    pub fn parse(text: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|v| v.as_str() == text)
    }
}

impl fmt::Display for Che {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}
