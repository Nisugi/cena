//! The spell table: what each spell is, costs, says, and locks out.
//!
//! Ports Lich's `data/effect-list.xml` through
//! `tools/extract_spells.rb`, the route `plan/13` §4a specifies for static
//! game knowledge. MEASURED: **515 spells, 628 messages.**
//!
//! # `common/spell.rb` is not the table
//!
//! It is the **reader** for one, and this document's earlier drafts called its
//! 954 lines "the unported spell table" and deferred four other items behind
//! it. The table itself ships with Lich and updates with it, which makes it
//! the same shape as the crit tables, the bestiary and the armament aliases.
//!
//! # The durations are Ruby, and 40 of them are real code
//!
//! The one place this port cannot be faithful, so it is measured rather than
//! glossed. Of 338 `<duration>` bodies:
//!
//! | | Count | Here |
//! |---|---:|---|
//! | a plain number | 178 | [`Duration::Fixed`] |
//! | arithmetic over circle ranks, level, `known?` | 120 | [`Duration::Derived`] |
//! | **real Ruby** | 40 | [`Duration::Unknown`] |
//!
//! The 40 include five that `reget` the scrollback and re-parse a CS/TD line
//! to work out how long a spell landed for. That is a scripting language
//! embedded in a data file, and an embedded scripting language is ruled out
//! here by settled decision.
//!
//! So they keep their Ruby verbatim and read as `Unknown`. §5.2: absent is not
//! zero, and a consumer asking gets `None` rather than a fabricated `0.25`.
//! [`Duration::Derived`] likewise keeps its expression rather than evaluating
//! it — turning `20 + Spells.minorspiritual` into a number needs the
//! character, so it belongs to whoever has one.
//!
//! # Cooldowns are two cast mechanics, not two data sources
//!
//! > *"the ones looking at member are ones that are self cast but affect your
//! > group."* — the author, 2026-09-20
//!
//! | Kind | Cast | How the target is learned |
//! |---|---|---|
//! | [`CooldownKind::Group`] | **on yourself**, lands on everyone grouped | it is not — stamp all members |
//! | [`CooldownKind::Target`] | at one character | [`Spell::target_start`] names them |
//!
//! VERIFIED as a structural invariant over the whole table, not a coincidence
//! of five rows: **every `target` cooldown has a `target-start` message and no
//! `group` cooldown has one.** There is nothing to capture in a self-cast,
//! because no name appears in the line. Pinned by
//! `tests/spells.rs::the_two_cooldown_kinds_split_on_cast_mechanics`.

use std::collections::BTreeMap;
use std::sync::OnceLock;

const SPELLS_TSV: &str = include_str!("../data/spells.tsv");

/// Between the parts of one packed value, e.g. cast-type / kind / body.
///
/// ASCII US. **Not `|` and not `~`**: the duration bodies are Ruby and four
/// contain a literal `|` inside a block parameter
/// (`history.find { |l| ... }`), so joining with `|` split them mid-expression
/// and this reader saw fragments — misclassifying five as
/// [`Duration::Unknown`]. The table looked green; a parity test counting the
/// kinds is what caught it.
const UNIT: char = '\u{1f}';

/// Between packed values in one column. ASCII RS, for the same reason.
const REC: char = '\u{1e}';

/// How long a spell lasts, as the table states it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Duration {
    /// A plain number of minutes.
    Fixed(String),
    /// Arithmetic over facts a character has: circle ranks, level, whether a
    /// spell is known.
    ///
    /// **Kept as its expression, not evaluated.** Resolving
    /// `20 + Spells.minorspiritual` needs a character, so it belongs to
    /// whoever has one rather than to a static table.
    Derived(String),
    /// Ruby this port does not evaluate.
    ///
    /// Five of these scrape the scrollback and re-parse a CS/TD line. The
    /// source is kept so a later port has it, and a consumer gets `None`
    /// rather than a guess.
    Unknown(String),
}

impl Duration {
    /// The expression or number, whatever kind this is.
    #[must_use]
    pub fn source(&self) -> &str {
        match self {
            Self::Fixed(s) | Self::Derived(s) | Self::Unknown(s) => s,
        }
    }

    /// The duration in minutes, when the table stated a plain number.
    ///
    /// `None` for both other kinds — including [`Self::Derived`], which is
    /// knowable but not from here.
    #[must_use]
    pub fn minutes(&self) -> Option<f32> {
        match self {
            Self::Fixed(s) => s.parse().ok(),
            _ => None,
        }
    }
}

/// Which form of a casting a duration describes.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum CastType {
    /// Cast on yourself. The table's default when it names no form.
    SelfCast,
    /// Cast at someone else.
    Target,
}

/// Why a character is locked out of a spell.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum CooldownKind {
    /// Cast **on yourself**, landing on everyone grouped.
    ///
    /// The game tells the caster nothing about who it reached, so a consumer
    /// stamps every member optimistically. See the module doc.
    Group,
    /// Cast at one character, who is named by [`Spell::target_start`].
    Target,
}

/// One spell.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Spell {
    /// The spell number, e.g. 215.
    pub number: u16,
    /// Its name, e.g. `"Heroism"`.
    pub name: String,
    /// The table's `type=` tag, verbatim. **Free text, not a vocabulary.**
    ///
    /// MEASURED over the 514: **19 distinct values**, slash-separated, with
    /// the same idea spelled more than one way — `offense` (26) and
    /// `offensive` (2), `attack/utility` (12) and `utility/attack` (1). So it
    /// is a `String`: C21 reserves typed fields for *closed* vocabularies and
    /// this is an open, inconsistent tag list. Absent on 12.
    ///
    /// **It does not say what the spell does.** `offense` means it improves
    /// your offence, not that it deals damage — that is `attack`:
    ///
    /// > *"heroism provides an offensive bonus, so probably a utility spell,
    /// > it does not damage on it's own so it's not an attack spell, which may
    /// > be different than offense/defense."* — the author, 2026-09-20
    ///
    /// MEASURED, and the split is stark: **23 of 34** `offense` spells carry
    /// an attack-strength bonus (`bolt-as`, `physical-as`, `*-cs`), against
    /// **2 of 136** `attack` spells. A behavior asking "will this hurt
    /// something" must not read `offense` as yes. What to read instead is
    /// [`Self::bonuses`], which states what is actually conferred.
    pub kind: Option<String>,
    /// `all`, `self-cast`, `group`… Absent on one.
    pub availability: Option<String>,
    /// Mana, spirit, stamina and renew costs, when stated.
    pub mana: Option<u16>,
    /// Spirit cost.
    pub spirit: Option<u16>,
    /// Stamina cost.
    pub stamina: Option<u16>,
    /// The cost to renew, which differs from the cost to cast.
    pub renew: Option<u16>,
    /// Durations by cast form.
    pub durations: Vec<(CastType, Duration)>,
    /// Combat bonuses this spell confers, by the table's own type names.
    pub bonuses: Vec<(String, String)>,
    /// The message when it goes up on you.
    pub message_up: Option<String>,
    /// The message when it drops.
    pub message_down: Option<String>,
    /// The third-person message naming who it landed on.
    ///
    /// **Only ever present with a [`CooldownKind::Target`] cooldown** — see
    /// the module doc.
    pub target_start: Option<String>,
    /// Seconds a character is locked out, by kind.
    pub cooldowns: Vec<(CooldownKind, u32)>,
}

impl Spell {
    /// The spell circle this number belongs to.
    ///
    /// `spell.rb`: a three-digit number's circle is its first digit, anything
    /// else its first two. So 215 is circle 2 and 1215 is circle 12.
    #[must_use]
    pub fn circle(&self) -> u16 {
        let text = self.number.to_string();
        let head = if text.len() == 3 {
            &text[..1]
        } else {
            &text[..2]
        };
        head.parse().unwrap_or(0)
    }

    /// The duration for a cast form, if the table states one.
    #[must_use]
    pub fn duration(&self, cast: CastType) -> Option<&Duration> {
        self.durations
            .iter()
            .find(|(form, _)| *form == cast)
            .map(|(_, duration)| duration)
    }

    /// How long a character is locked out of this spell, by kind.
    #[must_use]
    pub fn cooldown(&self, kind: CooldownKind) -> Option<u32> {
        self.cooldowns
            .iter()
            .find(|(held, _)| *held == kind)
            .map(|(_, seconds)| *seconds)
    }
}

/// The spell table, parsed once.
fn table() -> &'static BTreeMap<u16, Spell> {
    static TABLE: OnceLock<BTreeMap<u16, Spell>> = OnceLock::new();
    TABLE.get_or_init(|| {
        SPELLS_TSV
            .lines()
            .filter(|line| !line.starts_with('#'))
            .skip(1) // the header
            .filter(|line| !line.trim().is_empty())
            .filter_map(|line| read_row(line).map(|spell| (spell.number, spell)))
            .collect()
    })
}

/// One TSV row.
fn read_row(line: &str) -> Option<Spell> {
    let mut field = line.split('\t');
    let number: u16 = field.next()?.parse().ok()?;
    let name = field.next()?.to_owned();
    let kind = optional(field.next()?);
    let availability = optional(field.next()?);
    let mana = field.next()?.parse().ok();
    let spirit = field.next()?.parse().ok();
    let stamina = field.next()?.parse().ok();
    let renew = field.next()?.parse().ok();
    let durations = field.next()?.split(REC).filter_map(read_duration).collect();
    let bonuses = field
        .next()?
        .split(REC)
        .filter_map(|b| b.split_once(UNIT))
        .map(|(kind, value)| (kind.to_owned(), value.to_owned()))
        .collect();
    let message_up = optional(field.next()?);
    let message_down = optional(field.next()?);
    let target_start = optional(field.next()?);
    let mut cooldowns = Vec::new();
    if let Some(seconds) = field.next().and_then(|f| f.parse().ok()) {
        cooldowns.push((CooldownKind::Group, seconds));
    }
    if let Some(seconds) = field.next().and_then(|f| f.parse().ok()) {
        cooldowns.push((CooldownKind::Target, seconds));
    }
    Some(Spell {
        number,
        name,
        kind,
        availability,
        mana,
        spirit,
        stamina,
        renew,
        durations,
        bonuses,
        message_up,
        message_down,
        target_start,
        cooldowns,
    })
}

/// One `cast-type~kind~body` duration cell.
fn read_duration(cell: &str) -> Option<(CastType, Duration)> {
    let (cast, rest) = cell.split_once(UNIT)?;
    let (kind, body) = rest.split_once(UNIT)?;
    let cast = match cast {
        "target" => CastType::Target,
        _ => CastType::SelfCast,
    };
    let body = body.to_owned();
    let duration = match kind {
        "fixed" => Duration::Fixed(body),
        "derived" => Duration::Derived(body),
        _ => Duration::Unknown(body),
    };
    Some((cast, duration))
}

/// An empty TSV cell is an absent value, not an empty one.
fn optional(field: &str) -> Option<String> {
    (!field.is_empty()).then(|| field.to_owned())
}

/// A spell by number.
#[must_use]
pub fn spell(number: u16) -> Option<&'static Spell> {
    table().get(&number)
}

/// A spell by name, case-insensitively.
#[must_use]
pub fn spell_named(name: &str) -> Option<&'static Spell> {
    table()
        .values()
        .find(|spell| spell.name.eq_ignore_ascii_case(name.trim()))
}

/// Every spell, by number.
pub fn all() -> impl Iterator<Item = &'static Spell> {
    table().values()
}

/// Every spell that locks a character out, with its kind and seconds.
///
/// MEASURED: five, and the split is by **cast mechanic** — see the module doc.
pub fn with_cooldowns() -> impl Iterator<Item = &'static Spell> {
    table().values().filter(|s| !s.cooldowns.is_empty())
}

/// The name of a spell circle (`spells.rb:6`).
///
/// Ported whole, including the gaps: there is no circle 13, 14 or 15, and the
/// numbering jumps to 65, 66 and then the 90s. A circle the game adds later
/// reads `None` rather than a wrong name.
#[must_use]
pub fn circle_name(circle: u16) -> Option<&'static str> {
    Some(match circle {
        1 => "Minor Spirit",
        2 => "Major Spirit",
        3 => "Cleric",
        4 => "Minor Elemental",
        5 => "Major Elemental",
        6 => "Ranger",
        7 => "Sorcerer",
        8 => "Old Healing List",
        9 => "Wizard",
        10 => "Bard",
        11 => "Empath",
        12 => "Minor Mental",
        16 => "Paladin",
        17 => "Arcane",
        65 => "Imbedded Enchantment",
        66 => "Death",
        // Lich spells this "Micellaneous" (`spells.rb:26`). Corrected here:
        // it is a display string with no wire meaning, nothing matches on it,
        // and shipping a typo because the original had one is not fidelity.
        90 => "Miscellaneous",
        95 => "Armor Specialization",
        96 => "Combat Maneuvers",
        97 => "Guardians of Sunfist",
        98 => "Order of Voln",
        99 => "Council of Light",
        // Lich returns the string "Unknown Circle" here. `None` instead: a
        // caller that wants that text can write it, and one that wants to
        // know whether the circle is real can now tell.
        _ => return None,
    })
}
