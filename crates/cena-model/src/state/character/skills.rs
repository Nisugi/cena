//! The 46 skills and the 12 spell circles, and the classifier that reads a
//! `skills` table row.
//!
//! M3 step 5. Like [`stats`](super::stats) these are C21's typed named fields
//! (`research/04-inherited-decisions.md:1878`), and like that module the
//! classifier is stateless: one reassembled line in, a typed `Option` out.
//!
//! # The table, MEASURED
//!
//! Cut from `GSIV-Nisugi/2026/09/2026-09-18_15-49-13.xml:11197-11266` and
//! committed as `character_skills.xml`. The command was `skills full`:
//!
//! ```text
//!   Skill Name                         | Current Current
//!                                      |   Bonus   Ranks
//!   Two Weapon Combat..................|     312     212
//!   Armor Use..........................|       0       0
//!   ...
//!
//! Spell Lists
//!   Minor Spiritual....................|              40
//!
//! Spell Lists
//!   Ranger.............................|             162
//!
//! Training Points: 3673 Phy 0 Mnt (2866 Phy converted to Mnt)
//! ```
//!
//! Four facts that shape this file, each read off that capture rather than
//! assumed:
//!
//! **1. The columns are `Bonus` then `Ranks`** -- the reverse of how skills are
//! spoken about, and the reverse of the order this module's types list them in.
//! `infomon/parser.rb:23` names them the same way. This is exactly the hazard
//! [`stats`](super::stats) hit from the other direction, where the column
//! meaning changes between `info` and `info full`; the header line is in the
//! fixture so the order is under test rather than in a comment.
//!
//! **2. In `skills full`, all 46 skills arrive, zeros included.**
//! `Armor Use....| 0 0` is present for a character who has never trained it.
//! An untrained skill is a *zero*, not an absence -- which is what makes
//! [`SkillSet::clear`] before applying a table safe, and necessary: it is the
//! only way a skill that *dropped* to zero is ever recorded.
//!
//! > This corrects a recommendation made before the capture was read. I argued
//! > absent rows should be left alone, reasoning from `InfoReport::merge_into`'s
//! > `normal` column. The author chose clear-before-apply; the capture shows why
//! > that is right and the analogy was wrong. `info` genuinely omits a column;
//! > `skills full` omits nothing.
//! >
//! > **CORRECTED 2026-09-25:** this said `skills` omits nothing. Plain `skills`
//! > omits the untrained ones -- see the last section -- so
//! > [`SkillSet::replace`] records every skill a table left out as zero.
//!
//! **3. Bold marks enhancement per CELL, not per row.** Both numbers are
//! individually wrapped, and the fragments arrive with their leading spaces
//! (`"  312"`, `"212"`), which is why matching trims. Same wire signal
//! [`stats`](super::stats) reads, and the same reason: the game is telling us
//! which numbers an item is inflating.
//!
//! **4. Spell circles are NOT rows of the skill table.** Each gets its own
//! `Spell Lists` header and a row carrying **one** number, in the Ranks column,
//! with Bonus blank. Lich discriminates by regex field count and by clause
//! order -- `Pattern::Skill` is tried before `Pattern::SpellRanks`, and must be,
//! because `SpellRanks` (`parser.rb:24`) would otherwise match a skill row and
//! take its *bonus* as the rank.
//!
//! This module does not rely on that ordering. [`SkillLine::classify`] counts
//! the numeric fields itself, so a circle row and a skill row are distinguished
//! by their own shape rather than by which pattern was tried first. That is
//! `plan/12` §3a's stateless-classifier rule doing real work: a classifier that
//! needed to know what had already been tried would not be one.
//!
//! # Why the names come from `SKILL_NAME_MAP`
//!
//! `lib/attributes/enhancive.rb:42-88` is the **only** place in Lich where the
//! 46 wire strings are written down; `attributes/skills.rb:39` has just the
//! symbols. The port takes the display names from there, and
//! `tests/character_skills.rs` asserts every one of the 46 appears in the real
//! capture -- so a typo is a test failure, not a skill that silently never
//! parses.
//!
//! # Folded from the wire by [`read_table`], since 2026-09-25
//!
//! **Nothing folded a table into the model before then.** The classifier was
//! built and tested in M3 and the sync sends `skills full`, but every
//! [`SkillSet::apply`] call was in a test: `Character::consume_chunk` read
//! `info`, the profile, standing, currency, the PSMs and experience, and
//! `character.skills` stayed empty in live play. Found for the hunt, whose
//! `mstrike` reads `Skills.multiopponentcombat` (`bigshot.lic:6184`, `:6201`).
//!
//! A table is Lich's three lines (`infomon/parser.rb:22-25`): the header
//! `<Name> (at level N), your current skill bonuses and ranks`, the rows, and
//! the `Training Points: N Phy N Mnt` footer that commits them. Only a table
//! with both ends is folded, because the next point needs a complete one.
//!
//! **`skills` and `skills full` print the same header and differ in the rows.**
//! `SKILLS - Display your current skills known`; `SKILLS FULL - Display
//! current value of all skills`, *"including those not trained in"*
//! (`reference/wiki_clean/Verb_SKILLS.txt:9-11`, `:97`; its plain example,
//! `:21-37`, has 14 rows and no zero). So a skill a complete plain table does
//! not list is recorded untrained -- `Some(0)` -- rather than left unknown:
//! INFERRED from the wiki, and Lich's reading too, since its `Skills.x` is
//! `Infomon.get(...).to_i` and a skill no table wrote reads 0. `SKILLS BASE`
//! has its own header (`your base skill bonuses, ranks and goals are:`,
//! `:61`) and is never read as this table.

use std::collections::BTreeMap;

use crate::state::chunks::Chunk;

/// One skill's two numbers, and whether the wire bolded them.
///
/// Both are `Option` for the reason [`stats`](super::stats) gives: a column the
/// wire did not carry is *unknown*, which is not the same as zero. A `skills full`
/// table does carry both for all 46, but `skills base` (referenced by the
/// capture's own footer, `(Use SKILLS BASE to display unmodified ranks...)`) is
/// a second form this type must not misreport when it is implemented.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Skill {
    /// Ranks trained.
    pub ranks: Option<u16>,
    /// The bonus those ranks confer, after modifiers.
    pub bonus: Option<u16>,
    /// Whether this row arrived bolded, i.e. inflated by an enhancive.
    ///
    /// **Replaced on every table, never or-ed** -- the same rule
    /// `InfoReport::merge_into` records for stats: a skill that stopped
    /// arriving bolded stopped being enhanced, because the item came off.
    pub enhanced: bool,
}

impl Skill {
    /// Known to be zero, as opposed to never observed.
    ///
    /// Worth asking directly because the capture shows the wire sends `0 0` for
    /// untrained skills, so "zero" is a real answer and `None` means the table
    /// has not been read yet.
    #[must_use]
    pub fn is_untrained(&self) -> bool {
        self.ranks == Some(0)
    }
}

/// The 46 skills, in the wire's print order.
///
/// Ported from `lib/attributes/enhancive.rb:42-88` (`SKILL_NAME_MAP`). The
/// order is the capture's, which is also the map's.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, serde::Serialize, serde::Deserialize,
)]
pub enum SkillKind {
    /// Printed `Two Weapon Combat`.
    TwoWeaponCombat,
    /// Printed `Armor Use`.
    ArmorUse,
    /// Printed `Shield Use`.
    ShieldUse,
    /// Printed `Combat Maneuvers`.
    CombatManeuvers,
    /// Printed `Edged Weapons`.
    EdgedWeapons,
    /// Printed `Blunt Weapons`.
    BluntWeapons,
    /// Printed `Two-Handed Weapons`.
    TwoHandedWeapons,
    /// Printed `Ranged Weapons`.
    RangedWeapons,
    /// Printed `Thrown Weapons`.
    ThrownWeapons,
    /// Printed `Polearm Weapons`.
    PolearmWeapons,
    /// Printed `Brawling`.
    Brawling,
    /// Printed `Ambush`.
    Ambush,
    /// Printed `Multi Opponent Combat`.
    MultiOpponentCombat,
    /// Printed `Physical Fitness`.
    PhysicalFitness,
    /// Printed `Dodging`.
    Dodging,
    /// Printed `Arcane Symbols`.
    ArcaneSymbols,
    /// Printed `Magic Item Use`.
    MagicItemUse,
    /// Printed `Spell Aiming`.
    SpellAiming,
    /// Printed `Harness Power`.
    HarnessPower,
    /// Printed `Elemental Mana Control`.
    ElementalManaControl,
    /// Printed `Mental Mana Control`.
    MentalManaControl,
    /// Printed `Spirit Mana Control`.
    SpiritManaControl,
    /// Printed `Elemental Lore - Air`.
    ElementalLoreAir,
    /// Printed `Elemental Lore - Earth`.
    ElementalLoreEarth,
    /// Printed `Elemental Lore - Fire`.
    ElementalLoreFire,
    /// Printed `Elemental Lore - Water`.
    ElementalLoreWater,
    /// Printed `Spiritual Lore - Blessings`.
    SpiritualLoreBlessings,
    /// Printed `Spiritual Lore - Religion`.
    SpiritualLoreReligion,
    /// Printed `Spiritual Lore - Summoning`.
    SpiritualLoreSummoning,
    /// Printed `Sorcerous Lore - Demonology`.
    SorcerousLoreDemonology,
    /// Printed `Sorcerous Lore - Necromancy`.
    SorcerousLoreNecromancy,
    /// Printed `Mental Lore - Divination`.
    MentalLoreDivination,
    /// Printed `Mental Lore - Manipulation`.
    MentalLoreManipulation,
    /// Printed `Mental Lore - Telepathy`.
    MentalLoreTelepathy,
    /// Printed `Mental Lore - Transference`.
    MentalLoreTransference,
    /// Printed `Mental Lore - Transformation`.
    MentalLoreTransformation,
    /// Printed `Survival`.
    Survival,
    /// Printed `Disarming Traps`.
    DisarmingTraps,
    /// Printed `Picking Locks`.
    PickingLocks,
    /// Printed `Stalking and Hiding`.
    StalkingAndHiding,
    /// Printed `Perception`.
    Perception,
    /// Printed `Climbing`.
    Climbing,
    /// Printed `Swimming`.
    Swimming,
    /// Printed `First Aid`.
    FirstAid,
    /// Printed `Trading`.
    Trading,
    /// Printed `Pickpocketing`.
    Pickpocketing,
}

impl SkillKind {
    /// All 46, in the wire's print order.
    pub const ALL: [Self; 46] = [
        Self::TwoWeaponCombat,
        Self::ArmorUse,
        Self::ShieldUse,
        Self::CombatManeuvers,
        Self::EdgedWeapons,
        Self::BluntWeapons,
        Self::TwoHandedWeapons,
        Self::RangedWeapons,
        Self::ThrownWeapons,
        Self::PolearmWeapons,
        Self::Brawling,
        Self::Ambush,
        Self::MultiOpponentCombat,
        Self::PhysicalFitness,
        Self::Dodging,
        Self::ArcaneSymbols,
        Self::MagicItemUse,
        Self::SpellAiming,
        Self::HarnessPower,
        Self::ElementalManaControl,
        Self::MentalManaControl,
        Self::SpiritManaControl,
        Self::ElementalLoreAir,
        Self::ElementalLoreEarth,
        Self::ElementalLoreFire,
        Self::ElementalLoreWater,
        Self::SpiritualLoreBlessings,
        Self::SpiritualLoreReligion,
        Self::SpiritualLoreSummoning,
        Self::SorcerousLoreDemonology,
        Self::SorcerousLoreNecromancy,
        Self::MentalLoreDivination,
        Self::MentalLoreManipulation,
        Self::MentalLoreTelepathy,
        Self::MentalLoreTransference,
        Self::MentalLoreTransformation,
        Self::Survival,
        Self::DisarmingTraps,
        Self::PickingLocks,
        Self::StalkingAndHiding,
        Self::Perception,
        Self::Climbing,
        Self::Swimming,
        Self::FirstAid,
        Self::Trading,
        Self::Pickpocketing,
    ];

    /// The display name as the wire prints it (`SKILL_NAME_MAP`'s keys).
    ///
    /// **Hyphenation and spacing are load-bearing.** `Two-Handed Weapons` is
    /// hyphenated and `Two Weapon Combat` is not; the lores use ` - ` as a
    /// separator. These are matched literally, so each is a fact about the wire
    /// that `tests/character_skills.rs` checks against the capture.
    #[must_use]
    pub const fn display_name(self) -> &'static str {
        match self {
            Self::TwoWeaponCombat => "Two Weapon Combat",
            Self::ArmorUse => "Armor Use",
            Self::ShieldUse => "Shield Use",
            Self::CombatManeuvers => "Combat Maneuvers",
            Self::EdgedWeapons => "Edged Weapons",
            Self::BluntWeapons => "Blunt Weapons",
            Self::TwoHandedWeapons => "Two-Handed Weapons",
            Self::RangedWeapons => "Ranged Weapons",
            Self::ThrownWeapons => "Thrown Weapons",
            Self::PolearmWeapons => "Polearm Weapons",
            Self::Brawling => "Brawling",
            Self::Ambush => "Ambush",
            Self::MultiOpponentCombat => "Multi Opponent Combat",
            Self::PhysicalFitness => "Physical Fitness",
            Self::Dodging => "Dodging",
            Self::ArcaneSymbols => "Arcane Symbols",
            Self::MagicItemUse => "Magic Item Use",
            Self::SpellAiming => "Spell Aiming",
            Self::HarnessPower => "Harness Power",
            Self::ElementalManaControl => "Elemental Mana Control",
            Self::MentalManaControl => "Mental Mana Control",
            Self::SpiritManaControl => "Spirit Mana Control",
            Self::ElementalLoreAir => "Elemental Lore - Air",
            Self::ElementalLoreEarth => "Elemental Lore - Earth",
            Self::ElementalLoreFire => "Elemental Lore - Fire",
            Self::ElementalLoreWater => "Elemental Lore - Water",
            Self::SpiritualLoreBlessings => "Spiritual Lore - Blessings",
            Self::SpiritualLoreReligion => "Spiritual Lore - Religion",
            Self::SpiritualLoreSummoning => "Spiritual Lore - Summoning",
            Self::SorcerousLoreDemonology => "Sorcerous Lore - Demonology",
            Self::SorcerousLoreNecromancy => "Sorcerous Lore - Necromancy",
            Self::MentalLoreDivination => "Mental Lore - Divination",
            Self::MentalLoreManipulation => "Mental Lore - Manipulation",
            Self::MentalLoreTelepathy => "Mental Lore - Telepathy",
            Self::MentalLoreTransference => "Mental Lore - Transference",
            Self::MentalLoreTransformation => "Mental Lore - Transformation",
            Self::Survival => "Survival",
            Self::DisarmingTraps => "Disarming Traps",
            Self::PickingLocks => "Picking Locks",
            Self::StalkingAndHiding => "Stalking and Hiding",
            Self::Perception => "Perception",
            Self::Climbing => "Climbing",
            Self::Swimming => "Swimming",
            Self::FirstAid => "First Aid",
            Self::Trading => "Trading",
            Self::Pickpocketing => "Pickpocketing",
        }
    }

    /// The Lich key spelling, e.g. `two_weapon_combat`.
    ///
    /// This is `SKILL_NAME_MAP`'s *value*, and it is also what
    /// `Infomon._key` (`lib/gemstone/infomon.rb:132`) normalises the display
    /// name into: `downcase`, then `tr(' -', '_')`, then collapse runs. So
    /// `Elemental Lore - Air` and `elemental_lore_air` are the same key in
    /// Lich, which is why its writer can push a display name and its reader a
    /// symbol without them disagreeing.
    #[must_use]
    pub fn key(self) -> String {
        let name = self.display_name().to_ascii_lowercase();
        let mut key = String::with_capacity(name.len());
        let mut last_underscore = false;
        for ch in name.chars() {
            if ch == ' ' || ch == '-' {
                if !last_underscore {
                    key.push('_');
                    last_underscore = true;
                }
            } else {
                key.push(ch);
                last_underscore = false;
            }
        }
        key
    }

    /// Look a skill up by the display name the wire printed.
    #[must_use]
    pub fn parse(name: &str) -> Option<Self> {
        let name = name.trim();
        Self::ALL.into_iter().find(|k| k.display_name() == name)
    }
}

impl std::fmt::Display for SkillKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.display_name())
    }
}

/// One row of the `skills` table, classified.
///
/// A skill row and a spell-circle row are different shapes, so they are
/// different variants rather than one struct with an `Option`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SkillLine {
    /// A skill: a known name and **two** numbers.
    Skill {
        /// Which of the 46.
        kind: SkillKind,
        /// Bonus, the FIRST column on the wire.
        bonus: u16,
        /// Ranks, the SECOND column on the wire.
        ranks: u16,
    },
    /// A spell circle: a name and **one** number, the ranks.
    ///
    /// The name is a `String` because the circles are an open set in practice
    /// -- `Minor Spiritual`, `Ranger`, and every profession's own list -- and
    /// C21 reserves typed fields for closed vocabularies. `spells.rs` will add
    /// the typed circle enum; this variant carries what the row said.
    SpellCircle {
        /// The circle's printed name, e.g. `Minor Spiritual`.
        name: String,
        /// Ranks in that circle.
        ranks: u16,
    },
}

/// The separator between a row's name and its numbers.
///
/// The wire pads the name with dots to a fixed width and then a pipe:
/// `Two Weapon Combat..................|     312     212`.
const ROW_SEPARATOR: char = '|';

impl SkillLine {
    /// Classify one reassembled line of a `skills` table.
    ///
    /// Returns `None` for anything that is not a row -- the header, the blank
    /// lines, `Spell Lists`, the `Training Points:` footer, and any prose a
    /// player might type that happens to contain a pipe.
    ///
    /// # The discriminator is the field count
    ///
    /// Two numbers after the pipe is a skill; one is a spell circle. This is
    /// `parser.rb:23` vs `:24`, but decided by counting rather than by which
    /// regex was tried first -- see the module docs for why that matters.
    #[must_use]
    pub fn classify(line: &str) -> Option<Self> {
        let (name, numbers) = line.split_once(ROW_SEPARATOR)?;
        let name = name.trim_end_matches('.').trim();
        if name.is_empty() {
            return None;
        }

        let mut fields = numbers.split_whitespace();
        let first: u16 = fields.next()?.parse().ok()?;
        let second = fields.next();
        // A third number means this is not a row shape we know. Refusing is
        // Rule 2.2: better to report nothing than to guess which two of three
        // columns were meant.
        if fields.next().is_some() {
            return None;
        }

        match second {
            Some(second) => {
                let ranks = second.parse().ok()?;
                let kind = SkillKind::parse(name)?;
                Some(Self::Skill {
                    kind,
                    bonus: first,
                    ranks,
                })
            }
            // One number, and the name is NOT one of the 46. A single number
            // beside a known skill name would be a shape this parser has not
            // seen, so it is refused rather than read as a circle.
            None if SkillKind::parse(name).is_none() => Some(Self::SpellCircle {
                name: name.to_owned(),
                ranks: first,
            }),
            None => None,
        }
    }

    /// Classify a row and decide whether its numbers arrived bolded.
    ///
    /// Bold is the enhancement signal (`plan/15` §2c).
    ///
    /// # Both cells are checked, and the match trims
    ///
    /// MEASURED: the wire bolds each number separately and the fragments keep
    /// their column padding -- `["  312", "212"]` for one row. So a comparison
    /// without `trim` matches the ranks cell and misses the bonus cell, which
    /// is a bug that hides itself: the row still reports `enhanced`, from one
    /// of the two numbers, and looks correct.
    ///
    /// Found by mutation. Removing `.trim()` left the suite green because the
    /// only assertion keyed on ranks, whose fragment happens to be unpadded.
    /// Checking **both** cells is what makes the rule real: an enhancive
    /// inflates the bonus, and the bonus is the padded one.
    #[must_use]
    pub fn classify_with_bold(line: &str, bold: &[&str]) -> Option<(Self, bool)> {
        let parsed = Self::classify(line)?;
        let shown: [String; 2] = match &parsed {
            Self::Skill { ranks, bonus, .. } => [ranks.to_string(), bonus.to_string()],
            Self::SpellCircle { ranks, .. } => [ranks.to_string(), ranks.to_string()],
        };
        let bolded = bold
            .iter()
            .any(|fragment| shown.iter().any(|s| fragment.trim() == s));
        Some((parsed, bolded))
    }
}

/// Every skill and circle a character has been told about.
///
/// # `BTreeMap`, not 46 named fields
///
/// C21 asks for typed named fields, and [`SkillKind`] is that type -- the
/// vocabulary is closed and checked at compile time. The *storage* is a map
/// keyed by that enum, which keeps the 46 accessors from being 46 hand-written
/// struct fields plus 46 lines of `merge`, without giving up any typing: there
/// is no way to write a key that is not one of the 46.
///
/// Ordered, per `Vitals`' reason: a `HashMap`'s iteration order varies run to
/// run, and criterion 7 requires a replay to be deterministic.
#[derive(Debug, Clone, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct SkillSet {
    skills: BTreeMap<SkillKind, Skill>,
    circles: BTreeMap<String, u16>,
}

impl SkillSet {
    /// What is known about one skill.
    ///
    /// `None` means the table has never been read. A skill the character has
    /// not trained reports `Some` with `ranks: Some(0)` -- see
    /// [`Skill::is_untrained`].
    #[must_use]
    pub fn get(&self, kind: SkillKind) -> Option<&Skill> {
        self.skills.get(&kind)
    }

    /// Ranks in one skill, with every modifier the table included: Lich's
    /// `Skills.<name>` (`attributes/skills.rb:42-46`).
    ///
    /// **`None` means no `skills` table has been read**; after one, every skill
    /// answers, an untrained one `Some(0)`. Lich cannot tell those apart: its
    /// accessor is `.to_i`, so "never synced" reads 0.
    #[must_use]
    pub fn ranks(&self, kind: SkillKind) -> Option<u16> {
        self.skills.get(&kind)?.ranks
    }

    /// Ranks in a spell circle, by its printed name.
    #[must_use]
    pub fn circle(&self, name: &str) -> Option<u16> {
        self.circles.get(name).copied()
    }

    /// Every circle known, ordered by name.
    pub fn circles(&self) -> impl Iterator<Item = (&str, u16)> {
        self.circles.iter().map(|(n, r)| (n.as_str(), *r))
    }

    /// How many of the 46 have been observed.
    #[must_use]
    pub fn known_count(&self) -> usize {
        self.skills.len()
    }

    /// Forget everything, so a fresh table is not merged onto a stale one.
    ///
    /// **Called before applying a table, by the author's decision**, and the
    /// capture supports it: all 46 rows arrive every time, so nothing is lost,
    /// and a skill that *dropped* to zero is only ever recorded this way.
    ///
    /// This is the same argument `enhancive.rb:368` makes in a comment --
    /// *"Resets all enhancive values to 0/empty // Critical because game output
    /// only shows non-zero values"* -- reaching the opposite conclusion from the
    /// opposite premise. There, output omits zeros, so a reset is needed to
    /// avoid keeping stale non-zeros. Here, output includes zeros, so a reset is
    /// free. Both end at "clear first".
    pub fn clear(&mut self) {
        self.skills.clear();
        self.circles.clear();
    }

    /// Replace everything with one complete table's rows, from [`read_table`].
    ///
    /// Clears first, per [`Self::clear`], then records every skill the rows
    /// did not name as **untrained**: a complete `skills` table omits exactly
    /// those (the module docs cite the wiki), and `skills full` names all 46,
    /// so for it the fill does nothing.
    pub fn replace(&mut self, rows: &[(SkillLine, bool)]) {
        self.clear();
        for (row, bolded) in rows {
            self.apply(row, *bolded);
        }
        for kind in SkillKind::ALL {
            self.skills.entry(kind).or_insert(Skill {
                ranks: Some(0),
                bonus: Some(0),
                enhanced: false,
            });
        }
    }

    /// Record one classified row.
    pub fn apply(&mut self, line: &SkillLine, bolded: bool) {
        match line {
            SkillLine::Skill { kind, bonus, ranks } => {
                self.skills.insert(
                    *kind,
                    Skill {
                        ranks: Some(*ranks),
                        bonus: Some(*bonus),
                        enhanced: bolded,
                    },
                );
            }
            SkillLine::SpellCircle { name, ranks } => {
                self.circles.insert(name.clone(), *ranks);
            }
        }
    }
}

/// The line that opens a `skills` or `skills full` table.
///
/// ```text
/// Ashryn (at level 100), your current skill bonuses and ranks (including all modifiers) are:
/// ```
///
/// Lich's `SkillStart` (`infomon/parser.rb:22`): one word, then
/// `(at level N)`, then the phrase. The name must be one word, so a player
/// who *says* the line (`Bob says, "Ashryn (at level ...`) opens nothing.
/// Leading whitespace is allowed: Lich's pattern requires one, and the
/// committed capture's header was cut from the front of a longer line
/// (`crates/cena-protocol/tests/FIXTURES.md`), so which is true is UNVERIFIED.
#[must_use]
pub fn is_table_header(line: &str) -> bool {
    let Some((before, _)) = line
        .trim_start()
        .split_once("), your current skill bonuses and ranks")
    else {
        return false;
    };
    let Some((name, level)) = before.split_once(" (at level ") else {
        return false;
    };
    let word = |s: &str| !s.is_empty() && s.chars().all(char::is_alphanumeric);
    word(name) && !level.is_empty() && level.bytes().all(|b| b.is_ascii_digit())
}

/// The line that closes a table: `Training Points: 3673 Phy 0 Mnt ...`.
///
/// Lich's `SkillEnd` (`infomon/parser.rb:25`), which is where it commits the
/// rows it held. Both forms print it (`Verb_SKILLS.txt:48`, and the capture).
#[must_use]
pub fn is_table_end(line: &str) -> bool {
    let Some(rest) = line.strip_prefix("Training Points: ") else {
        return false;
    };
    let number = |s: Option<&str>| s.is_some_and(|s| s.bytes().all(|b| b.is_ascii_digit()));
    let mut words = rest.split_whitespace();
    number(words.next())
        && words.next() == Some("Phy")
        && number(words.next())
        && words.next() == Some("Mnt")
}

/// The last complete `skills` table in a chunk: every row between a header
/// and its `Training Points:` footer, each with whether it arrived bolded.
///
/// `None` when the chunk holds no header, or a header with no footer after it
/// -- an incomplete table cannot say which skills it left out. `Some` of no
/// rows is a real answer: a table that lists nothing trained.
#[must_use]
pub fn read_table(chunk: &Chunk) -> Option<Vec<(SkillLine, bool)>> {
    let mut open: Option<Vec<(SkillLine, bool)>> = None;
    let mut complete = None;
    for line in chunk.lines() {
        let text = line.text();
        if is_table_header(&text) {
            open = Some(Vec::new());
        } else if let Some(rows) = open.as_mut() {
            if is_table_end(&text) {
                complete = open.take();
            } else {
                rows.extend(SkillLine::classify_with_bold(&text, &line.bold_refs()));
            }
        }
    }
    complete
}

impl super::Character {
    /// Fold a chunk's `skills` table, and mark the group taught.
    pub(super) fn consume_skills(&mut self, chunk: &Chunk) {
        if let Some(rows) = read_table(chunk) {
            self.skills.replace(&rows);
            self.taught.insert(super::snapshot::Group::Skills);
        }
    }
}
