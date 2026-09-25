//! The parts of a creature: its attacks, what it drops, the lines it prints
//! and the abilities it declares.
//!
//! Moved down out of `creature.rs` (2026-09-24) when the port took everything
//! a template says and that file passed its split-parent cap; `creature.rs`
//! re-exports all of it, so every path is unchanged.

use super::Stat;

/// Which of the bestiary's attack lists an attack came from.
///
/// The template keeps six (`attack_attributes`), and they differ in what they
/// record: a physical attack and a bolt spell carry an attack strength, a
/// warding spell a casting strength, the rest only a name.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum AttackCategory {
    /// A weapon, claw, bite or charge: `physical_attacks`.
    Physical,
    /// A bolt spell, against the target's bolt DS: `bolt_spells`.
    BoltSpell,
    /// A warding spell, against the target's TD: `warding_spells`.
    WardingSpell,
    /// A spell that is neither: `offensive_spells`.
    OffensiveSpell,
    /// A combat maneuver: `maneuvers`.
    Maneuver,
    /// Anything else it does to you: `special_abilities`.
    SpecialAbility,
}

impl AttackCategory {
    /// Every category.
    pub const ALL: [Self; 6] = [
        Self::Physical,
        Self::BoltSpell,
        Self::WardingSpell,
        Self::OffensiveSpell,
        Self::Maneuver,
        Self::SpecialAbility,
    ];

    /// The TSV spelling.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Physical => "physical",
            Self::BoltSpell => "bolt_spell",
            Self::WardingSpell => "warding_spell",
            Self::OffensiveSpell => "offensive_spell",
            Self::Maneuver => "maneuver",
            Self::SpecialAbility => "special_ability",
        }
    }

    /// Parse the TSV spelling.
    #[must_use]
    pub fn parse(text: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|c| c.as_str() == text)
    }
}

/// One of a creature's attacks, of any category.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Attack {
    /// Which list it came from.
    pub category: AttackCategory,
    /// What the attack is called: `Claw`, `Major Cold (907)`, `Disarm`.
    pub name: String,
    /// Its attack strength, where the bestiary records a usable number:
    /// physical attacks and bolt spells.
    pub attack_strength: Option<Stat>,
    /// The AS field verbatim, when it could not be parsed.
    ///
    /// **Fourteen strengths across the 627 templates are malformed** --
    /// `"566 to"`, `"(lunge) 245-276"`, `"390 UAF"`, `"???"`, `""`. That is
    /// data-entry damage in Lich's bestiary rather than a shape worth
    /// modelling, so the text is kept and the number is `None`. Rule 2.2:
    /// nothing is dropped without saying so, and
    /// [`unparsed_attack_strengths`](super::unparsed_attack_strengths)
    /// counts them.
    pub attack_strength_raw: Option<String>,
    /// Its casting strength: warding spells, against the target's TD.
    pub casting_strength: Option<Stat>,
    /// The CS field verbatim, when it could not be parsed.
    pub casting_strength_raw: Option<String>,
    /// A special ability's note, where the bestiary writes one.
    pub note: Option<String>,
    /// A special ability's type, where the bestiary writes one.
    pub kind: Option<String>,
}

/// What a creature drops when it dies.
///
/// Its own type rather than fields on [`Creature`](super::Creature): these are one question --
/// *is this worth killing for loot* -- and a looting consumer wants them
/// together. `Treasure` is also the name Lich gives the same group
/// (`creature.rb:816-840`).
///
/// **Every flag is three-valued.** `None` is "nobody recorded it", which the
/// source says 164 times for boxes alone.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Treasure {
    /// What it skins into, where the bestiary names it.
    pub skin: Option<String>,
    /// Whether it can be skinned at all.
    ///
    /// Separate from [`Self::skin`] because the source says four things: a
    /// name (307), nothing (314), `false` (5, it does not skin) and `true` (1,
    /// it skins into something nobody wrote down). Until 2026-09-24 the last
    /// two arrived as skin *names*, `"false"` and `"true"`.
    pub skins: Option<bool>,
    /// Drops coins.
    pub coins: Option<bool>,
    /// Drops boxes.
    pub boxes: Option<bool>,
    /// Drops gems.
    pub gems: Option<bool>,
    /// Drops magic items.
    pub magic_items: Option<bool>,
    /// Whether its skin needs a blunt weapon.
    pub blunt_required: Option<bool>,
    /// Equipment drops that are real loot rather than "useless equipment".
    pub armaments: Vec<String>,
    /// Any other drop the bestiary names.
    pub other: Vec<String>,
}

impl Treasure {
    /// Is it known to drop nothing worth stopping for?
    ///
    /// `true` when no drop is *known*: an unrecorded flag counts as nothing,
    /// because a looter cannot plan on what nobody has seen.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        let known = [
            self.skins,
            self.coins,
            self.boxes,
            self.gems,
            self.magic_items,
        ];
        !known.contains(&Some(true)) && self.armaments.is_empty() && self.other.is_empty()
    }
}

/// What kind of line a creature message is: the template's `messaging` keys.
///
/// Every line is matchable text the game prints (`creature_message.rs`);
/// [`Self::Attack`] and [`Self::Trigger`] also carry a key saying which.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum MessageKind {
    /// What the room prints when it dies.
    Death,
    /// What it prints when it runs.
    Flee,
    /// What it prints when it arrives.
    Arrival,
    /// What it prints when the corpse decays.
    Decay,
    /// What LOOK shows.
    Description,
    /// What searching its corpse prints.
    Search,
    /// It is preparing a spell: the warning before the cast lands.
    SpellPrep,
    /// It rises from prone.
    Stand,
    /// It shakes off a stun.
    StunBreak,
    /// Idle flavor with no mechanical event: howls, clicking.
    Ambient,
    /// One of its attacks, keyed by the attack's snake-case name; the generic
    /// weapon swing is `attack` (`_creature_template.rb`).
    Attack,
    /// A special it does, keyed by the effect: `bind`, `web`, `silence`.
    /// *"Parsing cues. Keys here are the event ids your runtime
    /// understands"* (`_creature_template.rb`).
    Trigger,
}

impl MessageKind {
    /// Every kind.
    pub const ALL: [Self; 12] = [
        Self::Death,
        Self::Flee,
        Self::Arrival,
        Self::Decay,
        Self::Description,
        Self::Search,
        Self::SpellPrep,
        Self::Stand,
        Self::StunBreak,
        Self::Ambient,
        Self::Attack,
        Self::Trigger,
    ];

    /// The TSV spelling, which is Lich's key.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Death => "death",
            Self::Flee => "flee",
            Self::Arrival => "arrival",
            Self::Decay => "decay",
            Self::Description => "description",
            Self::Search => "search",
            Self::SpellPrep => "spell_prep",
            Self::Stand => "stand",
            Self::StunBreak => "stun_break",
            Self::Ambient => "ambient",
            Self::Attack => "attacks",
            Self::Trigger => "triggers",
        }
    }

    /// Parse the TSV spelling.
    #[must_use]
    pub fn parse(text: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|k| k.as_str() == text)
    }
}

/// One line a creature prints.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Message {
    /// Which attack or which special, for [`MessageKind::Attack`] and
    /// [`MessageKind::Trigger`]; `None` for every other kind.
    pub key: Option<String>,
    /// The line, with its `{placeholders}`.
    pub text: String,
}

/// A named entry with an optional note: a defensive ability.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Entry {
    /// What it is called.
    pub name: String,
    /// What the bestiary says about it.
    pub note: Option<String>,
}

/// An ability the bestiary declares, with its effects.
///
/// *"Informational only. Runtime applies effects from code via these ids"*
/// (`_creature_template.rb`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Ability {
    /// The id a runtime would key on: `frenzy`.
    pub id: String,
    /// The display name.
    pub name: String,
    /// `buff`, `debuff`, `aura`, `proc`.
    pub kind: Option<String>,
    /// `self`, `opponent`, `area`.
    pub target: Option<String>,
    /// How long it usually lasts, in seconds.
    pub typical_duration_s: Option<u32>,
    /// Whether it can be dispelled. Unrecorded for all 79 today.
    pub dispellable: Option<bool>,
    /// Its effects, in the source's order: a flag (`rooted`), or a flag
    /// with a value (`blocks_spells_at_or_above`, `50`).
    pub effects: Vec<(String, Option<String>)>,
    /// What the bestiary says about it.
    pub notes: Option<String>,
}
