//! Society abilities: Voln symbols, Council signs, Sunfist sigils.
//!
//! Ports `lib/gemstone/society.rb` and the three modules under
//! `lib/gemstone/societies/` (1,424 lines together), in two halves.
//!
//! **Which society and what rank** is [`membership`], a classifier over the
//! wire lines that state it. The [`Society`] enum it reports was already in
//! `character/vocabulary.rs` from M3, with the `max_rank` a Master's line
//! leaves unstated; nothing held the lines themselves until now.
//!
//! **What a member can then do** is this module and its three tables: what
//! each ability costs, and whether it is affordable right now.
//!
//! # Three societies, three currencies
//!
//! | Society | Ability | Costs | Ranks |
//! |---|---|---|---|
//! | Order of Voln | symbol | favor, scaled by character level | 26 |
//! | Council of Light | sign | spirit and/or mana | 20 |
//! | Guardians of Sunfist | sigil | stamina and/or mana | 20 |
//!
//! That is why [`Cost`] is an enum rather than one struct with three optional
//! fields: a Voln symbol has no mana cost in the sense that a sign does, and a
//! struct with two always-`None` fields invites a caller to ask a question the
//! society cannot answer.
//!
//! # The tables were verified against the wiki, not transcribed
//!
//! `order_of_voln.rb:10` cites <https://gswiki.play.net/Favor#Symbol_Use_Favor_Cost>
//! as the source for its favor table, and that page is in
//! `reference/wiki_clean/Favor.txt`. Every number below was diffed against it
//! (and against the Sunfist and Council pages) before being written here,
//! because a cost table transcribed from a single source has no oracle:
//!
//! | Checked | Values | Disagreements |
//! |---|---|---|
//! | Voln [`voln::BASE_FAVOR_COST_BY_LEVEL`] | 98 levels | **0** |
//! | Voln cost modifiers | 22 factors | **1**, see [`voln`] |
//! | Sunfist sigils | 20 x rank/stamina/mana | **0** |
//! | Council signs | 20 x rank/spirit/mana/timing | **0** |
//!
//! The wiki is the weaker source where it is silent -- it omits the stamina
//! column for nine sigils and calls Sigil of Intimidation's mana "Variable"
//! where Lich commits to 5 -- but it contradicts Lich in exactly one place,
//! recorded in the [`voln`] module.
//!
//! # What is NOT ported
//!
//! **`define_name_methods`** (`society.rb:171-183`) generates a singleton
//! method per ability so a script can write `OrderOfVoln.holiness`. That is
//! Ruby solving a Ruby problem: the same access here is
//! `voln::symbol("holiness")`, and 66 generated accessors would be 66 names to
//! keep in sync with the tables for no gain.
//!
//! **`use`** sends the command and waits on roundtime. Sending is a session
//! concern and `cena-model` sends nothing, so [`Ability::command`] builds the
//! string and stops there -- the split `Society.command` (`society.rb:148-152`)
//! already makes in Lich, and the one `plan/12` §3a requires here.
//!
//! **`pending_spirit_loss`** (`council_of_light.rb:381-386`) reads
//! `Effects::Buffs` to account for spirit that active `dissipates` signs will
//! still claim. It needs live effect state, so it belongs above the model
//! beside the other stateful consumers. What the model owes it is the timing,
//! which [`CostTiming`] carries.

pub mod col;
pub mod membership;
pub mod sunfist;
pub mod voln;

use crate::state::character::vocabulary::Society;

/// When a Council sign's spirit cost is actually deducted.
///
/// `council_of_light.rb` splits its signs by `:cost_type`, and the difference
/// is not cosmetic: an `Invoked` sign spends spirit now, while a `Dissipates`
/// sign spends it **when the effect expires**. A member with 3 spirit can
/// invoke three 1-spirit signs, but stacking three `Dissipates` signs means
/// owing 3 spirit later, which is why `affordable?` adds the pending loss
/// before answering (`council_of_light.rb:355-357`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum CostTiming {
    /// Spent at once, when the sign is invoked.
    Invoked,
    /// Spent when the effect wears off.
    Dissipates,
}

/// What an ability costs, in the currency its society uses.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Cost {
    /// Free: Recognition, Signal, Location, Kai's Strike, Kai's Smite, Seeking.
    Free,
    /// Voln favor, as a multiple of the level-indexed base cost.
    ///
    /// The absolute number depends on character level, so the table stores the
    /// modifier and [`voln::favor_cost`] resolves it.
    Favor {
        /// Multiplied by `BASE_FAVOR_COST_BY_LEVEL[level]`, then rounded up.
        modifier: f64,
        /// A second modifier, for the two symbols that have one.
        ///
        /// Blessing costs 0.04 rather than 0.20 on non-magical gear, and
        /// Retribution 0.30 rather than 0.04 when self-cast.
        alternate: Option<AlternateCost>,
    },
    /// Council of Light: spirit and/or mana, with the spirit timing.
    SpiritMana {
        /// Spirit points.
        spirit: u8,
        /// Mana points.
        mana: u8,
        /// `None` for the three free signs, which have no cost to time.
        paid_when: Option<CostTiming>,
    },
    /// Guardians of Sunfist: stamina and/or mana.
    StaminaMana {
        /// Stamina points.
        stamina: u8,
        /// Mana points.
        mana: u8,
    },
}

/// A second favor cost, for a different way of using the same symbol.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct AlternateCost {
    /// The modifier this use costs instead.
    pub modifier: f64,
    /// What makes it apply: `"non-magical"`, `"self-cast"`.
    pub reason: &'static str,
}

/// What an ability does, for a caller choosing between them.
///
/// `:type` in all three tables.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum AbilityKind {
    /// Used against a creature.
    Attack,
    /// Raises a defence.
    Defense,
    /// Raises an attack.
    Offense,
    /// Everything else.
    Utility,
}

impl AbilityKind {
    /// The Ruby symbol spelling.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Attack => "attack",
            Self::Defense => "defense",
            Self::Offense => "offense",
            Self::Utility => "utility",
        }
    }
}

/// One society ability.
///
/// The three tables are the same shape, so they are the same type. Where the
/// societies genuinely diverge is [`Cost`].
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Ability {
    /// Which society grants it.
    pub society: Society,
    /// The rank at which it is learned. Also its position in the table.
    pub rank: u8,
    /// The word the command takes: `holiness`, `striking`, `contact`.
    pub short_name: &'static str,
    /// The display name: `Symbol of Holiness`.
    pub long_name: &'static str,
    /// What it does.
    pub kind: AbilityKind,
    /// What it costs.
    pub cost: Cost,
    /// Its pseudo-spell number, as effect lists report it.
    ///
    /// Voln 9801-9826, Sunfist 9701-9720, Council 9901-9920.
    pub spell_number: u16,
    /// A verb that replaces the usual `<prefix> <short_name>` form.
    ///
    /// Two abilities are invoked by their own verb rather than by naming the
    /// ability: Kai's Smite is `smite`, Sign of Signal is `signal`
    /// (`society.rb:148`, `entry[:usage]`).
    pub usage: Option<&'static str>,
}

impl Ability {
    /// The command that uses this ability, without sending it.
    ///
    /// Ports `Society.command` (`society.rb:148-152`). A target is appended as
    /// `#id` when it is a numeric id, or verbatim when it is a name.
    ///
    /// ```
    /// use cena_model::state::societies::{Target, voln};
    /// let holiness = voln::symbol("holiness").expect("rank 9 symbol");
    /// assert_eq!(holiness.command(Target::None), "symbol of holiness");
    /// assert_eq!(holiness.command(Target::Id(12345)), "symbol of holiness #12345");
    /// ```
    #[must_use]
    pub fn command(&self, target: Target<'_>) -> String {
        let base = match self.usage {
            Some(verb) => verb.to_owned(),
            None => format!("{} {}", self.society.command_prefix(), self.short_name),
        };
        match target {
            Target::None => base,
            Target::Id(id) => format!("{base} #{id}"),
            Target::Named(name) => format!("{base} {name}"),
        }
    }

    /// Does a member of this rank know this ability?
    ///
    /// `known?` in all three modules: rank-gated, nothing more.
    #[must_use]
    pub const fn known_at(&self, member_rank: u8) -> bool {
        self.rank <= member_rank
    }
}

/// What an ability is used on.
///
/// Lich passes a `String`, an `Integer` or a `GameObj` and branches on the
/// class (`society.rb:148-152`). Three cases in one enum say the same thing
/// without the runtime type test, and make "no target" the value it already is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Target<'a> {
    /// Untargeted.
    None,
    /// A wire object id, sent as `#id`.
    Id(u32),
    /// A name, sent as given.
    Named(&'a str),
}

impl Society {
    /// The words a command uses before an ability's short name.
    ///
    /// `symbol of holiness`, `sign of striking`, `sigil of contact`. Lich
    /// passes this in at each call site (`order_of_voln.rb:344` and its two
    /// siblings); here it is a property of the society, so the three cannot
    /// drift apart.
    #[must_use]
    pub const fn command_prefix(self) -> &'static str {
        match self {
            Self::OrderOfVoln => "symbol of",
            Self::CouncilOfLight => "sign of",
            Self::GuardiansOfSunfist => "sigil of",
        }
    }

    /// Every ability this society grants, in rank order.
    #[must_use]
    pub const fn abilities(self) -> &'static [Ability] {
        match self {
            Self::OrderOfVoln => voln::SYMBOLS,
            Self::CouncilOfLight => col::SIGNS,
            Self::GuardiansOfSunfist => sunfist::SIGILS,
        }
    }

    /// Look up one ability by its short or long name, case-insensitively.
    ///
    /// Ports `Society.lookup` (`society.rb:110-118`), which normalises both
    /// names and matches either. Lich normalises through
    /// `Lich::Util.normalize_name` because it also turns these names into
    /// method identifiers; here only case and surrounding space matter.
    #[must_use]
    pub fn ability(self, name: &str) -> Option<&'static Ability> {
        let wanted = name.trim();
        self.abilities().iter().find(|a| {
            a.short_name.eq_ignore_ascii_case(wanted) || a.long_name.eq_ignore_ascii_case(wanted)
        })
    }
}
