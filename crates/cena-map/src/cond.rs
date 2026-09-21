//! A question about whoever is walking (`plan/21` §4.0).
//!
//! One vocabulary for a step's guard and an exit's price: "is this a bard",
//! "is Haste up", "does the profile say to run the ice". [`Walker`] is the
//! plain struct of facts the questions are asked of, filled from the model by
//! whoever plans the walk -- this crate never sees the model.
//!
//! # Unknown answers no
//!
//! Every fact in [`Walker`] can be missing, and a question about a missing
//! fact has **no answer** -- not `false`, which `not` would turn into `true`.
//! [`Cond::ask`] is three-valued for that reason, and [`Cond::holds`] is the
//! one place "no answer" becomes "no": a guard that cannot be answered does
//! not fire, and a price that cannot be answered is impassable. Refusing a
//! route is recoverable; walking one the character cannot take is not.

use std::collections::{HashMap, HashSet};

use serde::{Deserialize, Serialize};

/// What pricing and guards may ask about the walker. `None` and a missing key
/// both mean *the model has not been told*.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Walker {
    /// The travel profile: `ice_mode` → `auto`. A setting the profile does not
    /// carry is unknown; its default belongs to the profile, not to the map.
    pub settings: HashMap<String, String>,
    /// What earlier crossings wrote down (`Action::Remember`): which town an
    /// event ground or Mist Harbor was entered from, which side of the Red
    /// Forest. Named as upstream names them, less the `mapdb_` prefix
    /// (DECIDED, author, 2026-09-20: use Lich's list -- these are not added
    /// often). Kept per character and across logins: a character enters
    /// Duskruin on Friday and leaves on Sunday. A name never written is
    /// unknown, which prices the way back impassable -- correctly.
    pub memories: HashMap<String, String>,
    /// Prices the planner works out, by table and then by room
    /// (`Cost::Table`). `instability`: seconds from the instability the walker
    /// entered the Confluence by to each town -- leaving the plane puts it
    /// back there, so that walk is what the way out really costs. Filled when
    /// the walker enters, by one search from that room. A table or a room
    /// that is absent is unknown, and the exit is impassable.
    pub tables: HashMap<String, HashMap<u32, f64>>,
    /// Yes-or-no facts the planner works out, by name: `urchin_access` (the
    /// guides are paid for and have not expired -- a comparison against the
    /// clock, which this crate never reads), `hidden`, `invisible`,
    /// `mounted`; `premium_account` (the account, which the urchins' premium
    /// hall asks about); `hunting` (a hunting or wandering behaviour holds the
    /// character, which some exits must not be wandered through); `platinum`
    /// (the Platinum instance, where two shops swap doors). A name that is
    /// absent is unknown.
    pub flags: HashMap<String, bool>,
    /// As the game spells it: `Bard`.
    pub profession: Option<String>,
    /// As the game spells it: `Forest Gnome`. Asked about by a word of it.
    pub race: Option<String>,
    pub gender: Option<String>,
    pub level: Option<u32>,
    /// The town, as the game spells it: `Wehnimer's Landing`. `Some("")` is a
    /// walker known to be a citizen of nowhere; `None` is not known.
    pub citizenship: Option<String>,
    /// `Order of Voln`. `Some("")` is known to belong to none.
    pub society: Option<String>,
    pub society_rank: Option<u32>,
    /// `standing`, `sitting`, `kneeling`, `prone`.
    pub posture: Option<String>,
    /// The obvious exits of the room the walker is in, as the game lists them
    /// short: `ne`, `nw`, `up`, `out`.
    pub exits: Option<Vec<String>>,
    /// Whether the walker is still in the room this crossing began in. Only
    /// the walk knows, and only part-way through a crossing; while planning
    /// it is `None`.
    pub still_here: Option<bool>,
    /// The month of the game's calendar day, 1-12. A fact passed in, never
    /// read from a clock here, so a replay plans the same route.
    pub month: Option<u32>,
    /// Percent of capacity carried.
    pub encumbrance: Option<u32>,
    /// Ranks by skill name, lowercase. `None` until the skills are known at
    /// all, so an untrained skill (absent from a known table) is zero.
    pub skills: Option<HashMap<String, u32>>,
    /// Names of spells in effect. `None` until the game has listed them.
    pub active_spells: Option<HashSet<String>>,
    /// Names of everything the walker can cast: spells, and society powers
    /// such as sigils. `None` until known.
    pub known_spells: Option<HashSet<String>>,
    /// Of those, the ones it can pay for *now* -- mana, stamina or spirit,
    /// whichever the spell takes. Working that out needs the spell table and
    /// the vitals, which is the planner's business; this crate is told.
    pub affordable_spells: Option<HashSet<String>>,
}

/// A question. Externally tagged in JSON: `{"spell_active": "Haste"}`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Cond {
    All(Vec<Cond>),
    Any(Vec<Cond>),
    Not(Box<Cond>),
    /// True unless the inner question is *known* to hold -- so it is never
    /// unknown. For the second of two ways to cross: `go` when Water Walking
    /// is up, `swim` **otherwise**, and a walker nobody has looked at swims.
    /// `not` would leave that walker with no way across at all.
    Otherwise(Box<Cond>),
    /// The profile setting `.0` is exactly `.1`.
    Setting(String, String),
    /// The profile carries the setting `.0`, with any value that is not empty:
    /// "a trinket has been named". A profile always answers this, so it is
    /// never unknown.
    SettingIsSet(String),
    /// See [`Walker::flags`].
    Flag(String),
    /// The memory `.0` is exactly `.1`. See [`Walker::memories`].
    Remembered(String, String),
    Profession(String),
    /// The walker's race **contains** this word: `Gnome` is true of a Forest
    /// Gnome and a Burghal Gnome, as upstream's `=~ /Gnome/` is.
    Race(String),
    Gender(String),
    LevelAtLeast(u32),
    Citizenship(String),
    Society(String),
    SocietyRankAtLeast(u32),
    Posture(String),
    /// The room the walker is in lists this obvious exit.
    Exit(String),
    /// The crossing has not moved the walker yet: what an earlier
    /// `Action::TryMove` left to be done.
    StillHere,
    Month(u32),
    EncumbranceOver(u32),
    /// Ranks in skill `.0` are below `.1`.
    SkillUnder(String, u32),
    SpellActive(String),
    SpellKnown(String),
    SpellAffordable(String),
}

impl Cond {
    /// The answer, or `None` when a fact it needs is missing.
    ///
    /// `all` and `any` short-circuit the way the logic allows and no further:
    /// one `false` settles an `all` whatever else is unknown, one `true`
    /// settles an `any`, and otherwise an unknown part leaves the whole
    /// unknown.
    #[must_use]
    pub fn ask(&self, walker: &Walker) -> Option<bool> {
        match self {
            Cond::All(parts) => settle(parts, walker, false),
            Cond::Any(parts) => settle(parts, walker, true),
            Cond::Not(inner) => inner.ask(walker).map(|answer| !answer),
            Cond::Otherwise(inner) => Some(!inner.holds(walker)),
            Cond::Setting(name, value) => walker.settings.get(name).map(|is| is == value),
            Cond::SettingIsSet(name) => Some(
                walker
                    .settings
                    .get(name)
                    .is_some_and(|value| !value.is_empty()),
            ),
            Cond::Flag(name) => walker.flags.get(name).copied(),
            Cond::Remembered(name, value) => walker.memories.get(name).map(|is| is == value),
            Cond::Profession(name) => walker.profession.as_ref().map(|is| is == name),
            Cond::Race(word) => walker.race.as_ref().map(|is| is.contains(word.as_str())),
            Cond::Gender(name) => walker.gender.as_ref().map(|is| is == name),
            Cond::LevelAtLeast(level) => walker.level.map(|is| is >= *level),
            Cond::Citizenship(town) => walker.citizenship.as_ref().map(|is| is == town),
            Cond::Society(name) => walker.society.as_ref().map(|is| is == name),
            Cond::SocietyRankAtLeast(rank) => walker.society_rank.map(|is| is >= *rank),
            Cond::Exit(exit) => walker
                .exits
                .as_ref()
                .map(|exits| exits.iter().any(|is| is == exit)),
            Cond::StillHere => walker.still_here,
            Cond::Posture(name) => walker.posture.as_ref().map(|is| is == name),
            Cond::Month(month) => walker.month.map(|is| is == *month),
            Cond::EncumbranceOver(percent) => walker.encumbrance.map(|is| is > *percent),
            Cond::SkillUnder(skill, ranks) => walker
                .skills
                .as_ref()
                .map(|skills| skills.get(skill).copied().unwrap_or(0) < *ranks),
            Cond::SpellActive(spell) => has(walker.active_spells.as_ref(), spell),
            Cond::SpellKnown(spell) => has(walker.known_spells.as_ref(), spell),
            Cond::SpellAffordable(spell) => has(walker.affordable_spells.as_ref(), spell),
        }
    }

    /// [`Self::ask`], with no answer read as no.
    #[must_use]
    pub fn holds(&self, walker: &Walker) -> bool {
        self.ask(walker) == Some(true)
    }
}

fn has(names: Option<&HashSet<String>>, name: &str) -> Option<bool> {
    names.map(|names| names.contains(name))
}

/// `decisive` is the answer that settles the whole: `false` for `all`, `true`
/// for `any`.
fn settle(parts: &[Cond], walker: &Walker, decisive: bool) -> Option<bool> {
    let mut unknown = false;
    for part in parts {
        match part.ask(walker) {
            Some(answer) if answer == decisive => return Some(decisive),
            Some(_) => {}
            None => unknown = true,
        }
    }
    (!unknown).then_some(!decisive)
}
