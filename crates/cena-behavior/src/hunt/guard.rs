//! The guard vocabulary: the preconditions a routine step may carry
//! (`plan/30` §5; `plan/33` for every word bigshot has and what became of it).
//!
//! A guard names **when the step runs**. `(hidden)` runs a step only while I
//! am hidden; `(!hidden)` only while I am not. That is one rule for every
//! word, where bigshot has two (`plan/33` §1: `frozen`, `prone`, `rooted`
//! and `voidweaver` read the other way round), and the importer turns
//! bigshot's readings into this one.
//!
//! # Three answers, and unknown skips
//!
//! [`Condition::holds`] answers `Some(true)`, `Some(false)` or `None`: the
//! game has not said (`plan/12` §5.2). **A step whose guard is unknown does
//! not run.** That is bigshot's rule for `thp`, where unknown health skips
//! (`bigshot.lic:4336`), made the rule for every word: a guard is there to
//! hold a step back, and acting on a belief nothing supports is the failure
//! `StatusInfo::known` exists to prevent.
//!
//! # The words
//!
//! Each takes `!` in front. `plan/33` §2 gives the bigshot word each came
//! from and the line its branch is on.
//!
//! | Word | Runs when | Reads |
//! |---|---|---|
//! | `hidden` | I am hidden | `status.known().hidden()` |
//! | `self_kneeling` | I am kneeling | `status.known().kneeling()` |
//! | `disease`, `poison` | I am diseased, poisoned | `status.known()` |
//! | `outside` | the room is outdoors: its exits read `Obvious paths:` | the `room exits` component |
//! | `splashy`, `nomagic` | the map tags the room `meta:splashy`, `meta:nomagic` | the map room; unknown when unplaced |
//! | `alone` | no player outside my group is here | the room's claim (`state/claim.rs`) |
//! | `health_at_least N` | my health is N percent or more | the health bar |
//! | `mana_at_least N`, `stamina_at_least N`, `spirit_at_least N` | I have N **points** or more | the bar's amount |
//! | `encumbrance_at_least N` | I am N percent encumbered or more | the encumbrance bar |
//! | `buff`, `spell`, `cooldown`, `debuff` `"<name>"` | an effect of that name is up in that dialog | the effects, once the dialog has been seen |
//! | `expiring "<name>" N` | the named effect is down, or up with N seconds or less left | the effects, whichever dialog |
//! | `empowered_below N` | no Empowered buff of +N or more is up | the Buffs dialog |
//! | `stunned`, `webbed`, `sleeping`, `calm`, `disoriented`, `kneeling`, `sitting`, `flying`, `hovering`, `immobilized`, `rooted` | the **target** has that status | `has_status` |
//! | `down` | the target is sleeping, webbed, stunned, kneeling, sitting, prone or immobilized | bigshot's `PRONE_STATUSES` (`:2736`) |
//! | `ascended`, `ascension_boss`, `challenging`, `disengaged`, `inferior`, `mini_boss`, `mount`, `rider`, `sympathetic` | the target's `<crtrStatus>` says so | its flags, once any were sent |
//! | `undead`, `noncorporeal` | the target is of that type | `gameobj-data`, as bigshot's `npc.type` |
//! | `ancient` | its name starts `grizzled` or `ancient`, but it is no ancient ghoul master | the name (`:4404`) |
//! | `thp N` | the target's health is **known** and at or below N percent | `hp_percent`, when stated or the bestiary knows it |
//! | `fatalcrit`, `smote` | a fatal crit landed on the target; it is smitten | the creature |
//! | `position N`, `position_at_least N` | my unarmed position on the target is tier N; N or better | `ucs_position`; none is tier 0 |
//! | `ucstierup` | a tier-up attack has been named against the target | `ucs_tierup` |
//! | `injured "<part>" N` | the target's wound on that part is rank N or worse | `injury(part)` |
//! | `stunned_for N` | the target is stunned for N seconds more at least | the stun estimate |
//! | `helpless` | the target cannot act: dead, webbed, stunned, asleep, immobilized or rooted | `muckled`, Lich's `muckled?` |
//! | `coup_ready` | a coup de grace qualifies at my trained rank | `coup_eligible` (`creature.rb:622-629`) |
//! | `targets_at_least N`, `targets_at_most N` | N or more, N or fewer, valid targets are here | `valid_target` |
//! | `once` | this step has not yet been sent at this target in this room | [`Used`] |
//! | `once_here` | this step has not yet been sent in this room | [`Used`] |
//! | `every N` | this step was last sent in this room N seconds ago or more, or never | [`Used`] |
//!
//! Words for facts the model cannot state yet are left out and import
//! **held**: `essence_at_least` (the `resource` capture) and `justice` (a
//! Swift Justice charge count), `plan/33`'s **later**.
//!
//! # `expiring` means what the author meant by `buff5`, not what bigshot does
//!
//! `kweed (expiring "Tangleweed Vigor" 5)` runs kweed when Tangleweed Vigor
//! is **down, or up with five seconds or less left**: refresh it before it
//! lapses, and otherwise leave it. That is what the author meant by
//! `kweed(buff5)` (2026-09-24), and it is the natural reading of the word.
//!
//! It is **not** what bigshot does. `bigshot.lic:4263` vetoes the step while
//! the buff is up with N seconds or less left and runs it otherwise, the
//! inverse; and the comment beside it (`:4246`) records that before that
//! commit `buffN` "was always nil and a silent no-op". So the author's
//! profile never had the guard enforced either way, and the code that now
//! enforces it enforces the opposite of the intent. Hydra keeps the intent.
//!
//! An effect name matches by **prefix, ignoring case** (author, `plan/33`
//! question 3): the Buffs dialog cuts long names off, so a bar reads
//! `Nature's Touch Arcane Ref`, and a person writes what they see.

use std::fmt;

use cena_session::creature::status::Classification;
use cena_session::{BodyPart, GameState, StatusName};

mod read;
mod used;

pub use used::Used;

/// A word with nothing after it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Fact {
    /// `hidden`: I am hidden.
    Hidden,
    /// `self_kneeling`: I am kneeling.
    SelfKneeling,
    /// `disease`: I am diseased.
    Diseased,
    /// `poison`: I am poisoned.
    Poisoned,
    /// `outside`: the room is outdoors.
    Outside,
    /// `splashy`: the map tags the room `meta:splashy`.
    Splashy,
    /// `nomagic`: the map tags the room `meta:nomagic`.
    NoMagic,
    /// `alone`: no player outside my group is here.
    Alone,
    /// A status on the target: `stunned`, `rooted` and the rest.
    Status(StatusName),
    /// `down`: any of bigshot's `PRONE_STATUSES` on the target.
    Down,
    /// A `<crtrStatus>` flag on the target: `ascended` and the rest.
    Flag(Classification),
    /// `undead`: the target is undead.
    Undead,
    /// `noncorporeal`: the target is noncorporeal.
    Noncorporeal,
    /// `ancient`: the target's name starts `grizzled` or `ancient`.
    Ancient,
    /// `fatalcrit`: a fatal crit landed on the target.
    FatalCrit,
    /// `smote`: the target is smitten.
    Smote,
    /// `ucstierup`: a tier-up attack has been named against the target.
    TierUp,
    /// `helpless`: the target cannot act.
    Helpless,
    /// `coup_ready`: a coup de grace qualifies at my trained rank.
    CoupReady,
    /// `once`: not yet sent at this target in this room.
    Once,
    /// `once_here`: not yet sent in this room.
    OnceHere,
}

/// A word and the number after it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Measure {
    /// `health_at_least N`: percent.
    HealthAtLeast,
    /// `mana_at_least N`: points.
    ManaAtLeast,
    /// `stamina_at_least N`: points.
    StaminaAtLeast,
    /// `spirit_at_least N`: points.
    SpiritAtLeast,
    /// `encumbrance_at_least N`: percent.
    EncumbranceAtLeast,
    /// `thp N`: the target's known health, percent, at most.
    TargetHealthAtMost,
    /// `empowered_below N`: no Empowered of `+N` or more is up.
    EmpoweredBelow,
    /// `position N`: my unarmed position on the target is exactly this tier.
    Position,
    /// `position_at_least N`: this tier or better.
    PositionAtLeast,
    /// `stunned_for N`: seconds of stun left, at least.
    StunnedFor,
    /// `targets_at_least N`: valid targets here, at least.
    TargetsAtLeast,
    /// `targets_at_most N`: valid targets here, at most.
    TargetsAtMost,
    /// `every N`: seconds since this step was last sent here, at least.
    Every,
}

/// The dialog an effect is listed in, for `buff "<name>"` and its kin.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Dialog {
    /// `buff`: the Buffs dialog.
    Buffs,
    /// `spell`: Active Spells.
    Spells,
    /// `cooldown`: Cooldowns.
    Cooldowns,
    /// `debuff`: Debuffs.
    Debuffs,
}

impl Dialog {
    /// The dialog's name, as the game titles it.
    #[must_use]
    pub const fn title(self) -> &'static str {
        match self {
            Self::Buffs => "Buffs",
            Self::Spells => "Active Spells",
            Self::Cooldowns => "Cooldowns",
            Self::Debuffs => "Debuffs",
        }
    }
}

/// The bare words.
const FACTS: &[(&str, Fact)] = &[
    ("hidden", Fact::Hidden),
    ("self_kneeling", Fact::SelfKneeling),
    ("disease", Fact::Diseased),
    ("poison", Fact::Poisoned),
    ("outside", Fact::Outside),
    ("splashy", Fact::Splashy),
    ("nomagic", Fact::NoMagic),
    ("alone", Fact::Alone),
    ("stunned", Fact::Status(StatusName::Stunned)),
    ("webbed", Fact::Status(StatusName::Webbed)),
    ("sleeping", Fact::Status(StatusName::Sleeping)),
    ("calm", Fact::Status(StatusName::Calm)),
    ("disoriented", Fact::Status(StatusName::Disoriented)),
    ("kneeling", Fact::Status(StatusName::Kneeling)),
    ("sitting", Fact::Status(StatusName::Sitting)),
    ("flying", Fact::Status(StatusName::Flying)),
    ("hovering", Fact::Status(StatusName::Hovering)),
    ("immobilized", Fact::Status(StatusName::Immobilized)),
    ("rooted", Fact::Status(StatusName::Rooted)),
    ("down", Fact::Down),
    ("ascended", Fact::Flag(Classification::Ascended)),
    ("ascension_boss", Fact::Flag(Classification::AscensionBoss)),
    ("challenging", Fact::Flag(Classification::Challenging)),
    ("disengaged", Fact::Flag(Classification::Disengaged)),
    ("inferior", Fact::Flag(Classification::Inferior)),
    ("mini_boss", Fact::Flag(Classification::MiniBoss)),
    ("mount", Fact::Flag(Classification::Mount)),
    ("rider", Fact::Flag(Classification::Rider)),
    ("sympathetic", Fact::Flag(Classification::Sympathetic)),
    ("undead", Fact::Undead),
    ("noncorporeal", Fact::Noncorporeal),
    ("ancient", Fact::Ancient),
    ("fatalcrit", Fact::FatalCrit),
    ("smote", Fact::Smote),
    ("ucstierup", Fact::TierUp),
    ("helpless", Fact::Helpless),
    ("coup_ready", Fact::CoupReady),
    ("once", Fact::Once),
    ("once_here", Fact::OnceHere),
];

/// The words that take a number.
const MEASURES: &[(&str, Measure)] = &[
    ("health_at_least", Measure::HealthAtLeast),
    ("mana_at_least", Measure::ManaAtLeast),
    ("stamina_at_least", Measure::StaminaAtLeast),
    ("spirit_at_least", Measure::SpiritAtLeast),
    ("encumbrance_at_least", Measure::EncumbranceAtLeast),
    ("thp", Measure::TargetHealthAtMost),
    ("empowered_below", Measure::EmpoweredBelow),
    ("position", Measure::Position),
    ("position_at_least", Measure::PositionAtLeast),
    ("stunned_for", Measure::StunnedFor),
    ("targets_at_least", Measure::TargetsAtLeast),
    ("targets_at_most", Measure::TargetsAtMost),
    ("every", Measure::Every),
];

/// The words that take an effect's name.
const DIALOGS: &[(&str, Dialog)] = &[
    ("buff", Dialog::Buffs),
    ("spell", Dialog::Spells),
    ("cooldown", Dialog::Cooldowns),
    ("debuff", Dialog::Debuffs),
];

/// One precondition on a step, from the closed vocabulary above.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Guard {
    /// A word with nothing after it.
    Is(Fact),
    /// A word and its number.
    Amount(Measure, u32),
    /// An effect of this name is up in this dialog.
    Effect(Dialog, String),
    /// `expiring "<name>" N`: the named effect is down, or up with `N`
    /// seconds or less left. The name is a prefix of the display name.
    Expiring {
        /// The effect's display name, or the start of it, as the game lists it.
        name: String,
        /// Seconds left, at most.
        within: u32,
    },
    /// `injured "<part>" N`: the target's wound on `part` is rank `N` or worse.
    Injured {
        /// The body part.
        part: BodyPart,
        /// The rank, at least.
        rank: u32,
    },
}

/// A guard and its polarity: `!hidden` runs the step when I am not hidden.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Condition {
    /// The fact read.
    pub guard: Guard,
    /// `!` was written: the step runs when the guard does **not** hold.
    pub negated: bool,
}

/// What a guard reads: the game, and what the game alone cannot say.
#[derive(Clone, Copy, Debug)]
pub struct Facts<'a> {
    /// The game as it is now.
    pub state: &'a GameState,
    /// The creature the step is aimed at; `None` when none is chosen.
    pub target: Option<i64>,
    /// The map's `meta:` tags for this room, prefix removed; `None` when the
    /// map could not place the character.
    pub tags: Option<&'a [String]>,
    /// What the routine has sent in this room; `None` reads as nothing sent.
    pub used: Option<&'a Used>,
    /// The step asked about, as written: the key [`Used`] keeps.
    pub step: &'a str,
}

impl<'a> Facts<'a> {
    /// The game alone: no room placed, nothing sent.
    #[must_use]
    pub const fn new(state: &'a GameState, target: Option<i64>) -> Self {
        Self {
            state,
            target,
            tags: None,
            used: None,
            step: "",
        }
    }
}

impl Condition {
    /// Read one group of guards, the parentheses already gone:
    /// `thp 20 empowered_below 30`, or `expiring "Tangleweed Vigor" 5`.
    ///
    /// # Errors
    ///
    /// A word Hydra does not know, a number or name missing or malformed, or
    /// a quote left open. The message says which, and lists the words.
    pub fn parse_group(text: &str) -> Result<Vec<Self>, String> {
        let mut out = Vec::new();
        let mut it = tokens(text)?.into_iter();
        while let Some(token) = it.next() {
            let (negated, word) = token
                .strip_prefix('!')
                .map_or((false, token.as_str()), |word| (true, word));
            let guard = if word.is_empty() {
                return Err("`!` needs a guard after it".to_owned());
            } else if let Some((_, fact)) = FACTS.iter().find(|(w, _)| *w == word) {
                Guard::Is(*fact)
            } else if let Some((_, measure)) = MEASURES.iter().find(|(w, _)| *w == word) {
                Guard::Amount(*measure, number(word, it.next())?)
            } else if let Some((_, dialog)) = DIALOGS.iter().find(|(w, _)| *w == word) {
                Guard::Effect(*dialog, quoted(word, it.next())?)
            } else if word == "expiring" {
                let name = quoted(word, it.next())?;
                let within = number(word, it.next())?;
                Guard::Expiring { name, within }
            } else if word == "injured" {
                let named = quoted(word, it.next())?;
                let part = body_part(&named).ok_or_else(|| {
                    format!(
                        "`injured`: `{named}` is not a body part. The parts are: {}",
                        parts()
                    )
                })?;
                let rank = number(word, it.next())?;
                Guard::Injured { part, rank }
            } else {
                return Err(format!(
                    "`{word}` is not a guard Hydra knows. The guards are: {}",
                    words()
                ));
            };
            out.push(Self { guard, negated });
        }
        Ok(out)
    }

    /// Whether the step may run, against the facts as they are now.
    ///
    /// `None` when the game has not said, and the step then does not run.
    #[must_use]
    pub fn holds(&self, facts: &Facts<'_>) -> Option<bool> {
        self.guard.read(facts).map(|holds| holds != self.negated)
    }
}

impl fmt::Display for Condition {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.negated {
            f.write_str("!")?;
        }
        match &self.guard {
            Guard::Is(fact) => f.write_str(name_of(FACTS, fact)),
            Guard::Amount(measure, n) => write!(f, "{} {n}", name_of(MEASURES, measure)),
            Guard::Effect(dialog, name) => write!(f, "{} \"{name}\"", name_of(DIALOGS, dialog)),
            Guard::Expiring { name, within } => write!(f, "expiring \"{name}\" {within}"),
            Guard::Injured { part, rank } => write!(f, "injured \"{}\" {rank}", part.as_str()),
        }
    }
}

/// The word a table gives a value; every value in a table has one.
fn name_of<T: PartialEq>(table: &[(&'static str, T)], value: &T) -> &'static str {
    table
        .iter()
        .find(|(_, v)| v == value)
        .map_or("?", |(word, _)| word)
}

/// Every word, with what follows it: what an error message lists.
fn words() -> String {
    let mut all: Vec<String> = FACTS.iter().map(|(w, _)| (*w).to_owned()).collect();
    all.extend(MEASURES.iter().map(|(w, _)| format!("{w} N")));
    all.extend(DIALOGS.iter().map(|(w, _)| format!("{w} \"<name>\"")));
    all.push("expiring \"<name>\" N".to_owned());
    all.push("injured \"<part>\" N".to_owned());
    all.join(", ")
}

/// A body part in Lich's spelling (`leftArm`), ignoring case and spaces, so
/// `left arm` reads too.
fn body_part(named: &str) -> Option<BodyPart> {
    let squeezed: String = named.chars().filter(|c| !c.is_whitespace()).collect();
    BodyPart::ALL
        .into_iter()
        .find(|part| part.as_str().eq_ignore_ascii_case(&squeezed))
}

/// The body parts `injured` takes.
fn parts() -> String {
    BodyPart::ALL
        .iter()
        .map(|part| part.as_str())
        .collect::<Vec<_>>()
        .join(", ")
}

/// Split on whitespace, keeping `"a quoted name"` as one token.
///
/// # Errors
///
/// A quote opened and not closed.
pub(crate) fn tokens(text: &str) -> Result<Vec<String>, String> {
    let mut out = Vec::new();
    let mut current = String::new();
    let mut quoted = false;
    for c in text.chars() {
        if c == '"' {
            quoted = !quoted;
            current.push(c);
        } else if c.is_whitespace() && !quoted {
            if !current.is_empty() {
                out.push(std::mem::take(&mut current));
            }
        } else {
            current.push(c);
        }
    }
    if quoted {
        return Err(format!("a quote was opened and not closed in `{text}`"));
    }
    if !current.is_empty() {
        out.push(current);
    }
    Ok(out)
}

/// The number a word takes.
fn number(word: &str, token: Option<String>) -> Result<u32, String> {
    let token = token.ok_or_else(|| format!("`{word}` needs a number after it"))?;
    token
        .parse()
        .map_err(|_| format!("`{word}` needs a number after it, not `{token}`"))
}

/// The quoted name a word takes, quotes removed.
fn quoted(word: &str, token: Option<String>) -> Result<String, String> {
    let token = token.ok_or_else(|| format!("`{word}` needs a name in double quotes after it"))?;
    token
        .strip_prefix('"')
        .and_then(|t| t.strip_suffix('"'))
        .filter(|inner| !inner.is_empty())
        .map(str::to_owned)
        .ok_or_else(|| format!("`{word}` needs a name in double quotes after it, not `{token}`"))
}
