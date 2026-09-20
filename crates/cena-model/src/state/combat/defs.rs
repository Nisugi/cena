//! The definition tables: three TSVs, compiled once, matched in Lich's order.
//!
//! See the parent module for what the files are and how they were cut. This
//! file owns loading them, substituting the hand-ported patterns, and the one
//! operation every family needs: *the first def in this family that matches
//! this text, with its captures*.
//!
//! # Nothing is dropped without saying so
//!
//! A row whose pattern will not compile under this crate's `regex` is kept
//! with `regex: None` and listed in [`Defs::compile_failures`]; a flagged
//! `markup=1` row with no hand-port likewise. Both are counted, and
//! `tests/combat_defs.rs` asserts the counts are what the port expects --
//! zero and zero -- so a Lich update that adds a lookbehind or a new
//! tag-reading pattern goes red rather than silently shrinking the grammar
//! (Rule 2.2).

use std::collections::BTreeMap;
use std::sync::OnceLock;

use regex::{Captures, Regex};

const ATTACKS_TSV: &str = include_str!("../../../data/combat_attacks.tsv");
const RESULTS_TSV: &str = include_str!("../../../data/combat_results.tsv");
const EFFECTS_TSV: &str = include_str!("../../../data/combat_effects.tsv");

/// A pattern re-expressed by hand, replacing the data's row at `(family, order)`.
#[derive(Debug, Clone, Copy)]
pub struct HandPort {
    /// The family whose row this replaces.
    pub family: &'static str,
    /// The row's position in that family's lookup, 1-based -- so first-match
    /// order stays exactly Lich's.
    pub order: u32,
    /// The plain-text pattern.
    pub pattern: &'static str,
    /// A line-level veto: when it matches, the row does not.
    ///
    /// The shape `crit/match_index.rs` built for the one lookahead in the crit
    /// tables, reused for the one lookbehind here. Rust's `regex` has neither,
    /// by design (linear time), and a positive match plus a separate veto says
    /// the same thing.
    pub veto: Option<&'static str>,
}

/// The nine patterns the extractor could not make plain, and their ports.
///
/// Eight read an `exist` id out of the tag (`markup=1` in the data); the
/// ninth is the single lookbehind in 954 rows (`statuses.rb:118`,
/// `inventory/11` §2b). The id each tag-reader needs comes from the line's
/// links (`target.rs`), which is the point: the pattern finds the sentence,
/// the runs say who.
///
/// | Lich | Here |
/// |---|---|
/// | `(?<target>.+?</a>) .+?!` (energy, sprite) | `(?<target>[^!]+)!` -- capture to the bang; the link inside it is the target. The original's `</a>` was the boundary; overcapturing the text is harmless because the id is the link's, not the text's. |
/// | `Your <a exist="(?<id>\d+)" ...>(?<name>[^<]+)</a>` | `^\*\* Your ` / `^Your ` -- the weapon is the first link whose run follows `Your `; see `flare.rs` |
/// | the five UCS `<a exist="([0-9]+)"` forms | the sentence without the tag; the creature is the line's first link |
/// | `(?<target>.+?)(?<! dazed and)(?<! paralyzed and) is (?:knocked\|driven) to ... knees!` | the same without the lookbehinds, plus a veto on `(?:dazed\|paralyzed) and is ...` -- a line where the "target" would have ended in the excluded words is refused whole |
///
/// The orders are read from the data, not remembered: a first draft wrote
/// 59 and 131 for the two flares from memory, and the loader's own test said
/// 39 and 90.
pub const HAND_PORTED: &[HandPort] = &[
    HandPort {
        family: "flare",
        order: 39,
        pattern: r"\*\* A beam of .+? energy emits from the tip of your .+? and collides with (?<target>[^!]+)! \*\*",
        veto: None,
    },
    HandPort {
        family: "flare",
        order: 90,
        pattern: r"\*\* The .+? sprite on your shoulder sends forth a cylindrical, .+? blast of magic at (?<target>[^!]+)! \*\*",
        veto: None,
    },
    HandPort {
        family: "flare_weapon_link",
        order: 1,
        pattern: r"^(?:\*\* )?Your ",
        veto: None,
    },
    HandPort {
        family: "ucs",
        order: 1,
        pattern: r"^You have (?<tier>decent|good|excellent) positioning against ",
        veto: None,
    },
    HandPort {
        family: "ucs",
        order: 2,
        pattern: r" has (?<tier>decent|good|excellent) positioning against you\.",
        veto: None,
    },
    HandPort {
        family: "ucs",
        order: 4,
        pattern: r"^ *A crimson mist suddenly surrounds ",
        veto: None,
    },
    HandPort {
        family: "ucs",
        order: 5,
        pattern: r"The crimson mist surrounding .+held in the corporeal plane",
        veto: None,
    },
    HandPort {
        family: "ucs",
        order: 6,
        pattern: r"^ *The crimson mist surrounding .+returns to an ethereal state",
        veto: None,
    },
    HandPort {
        family: "status",
        order: KNEELING_ORDER,
        pattern: r"(?<target>.+?) is (?:knocked|driven) to (?:his|her|its) knees!",
        veto: Some(r"(?:dazed|paralyzed) and is (?:knocked|driven) to (?:his|her|its) knees!"),
    },
];

/// The `status` row carrying `statuses.rb:118`'s lookbehinds, by position.
///
/// Written by the loader's own report (`every_pattern_compiles` names the
/// row) rather than by hand.
const KNEELING_ORDER: u32 = 32;

/// What part a pattern plays within its family.
///
/// Most families have none. Statuses are `add`/`remove` pairs; brackets
/// (assaults, sequences) are `start`/`end` pairs; the `attack_class` rows are
/// name lists under three roles.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Role {
    /// No role: the family is flat.
    None,
    /// A status onset line.
    Add,
    /// A status expiry line.
    Remove,
    /// A bracket's opening line.
    Start,
    /// A bracket's closing line.
    End,
    /// `attack_class`: the world did it (frigid wind).
    Environmental,
    /// `attack_class`: our own gear did it (thorn recoil).
    SelfInflicted,
    /// `attack_class`: a creature maneuver aimed at the whole room.
    RoomTargeted,
}

impl Role {
    fn parse(text: &str) -> Self {
        match text {
            "add" => Self::Add,
            "remove" => Self::Remove,
            "start" => Self::Start,
            "end" => Self::End,
            "environmental" => Self::Environmental,
            "self_inflicted" => Self::SelfInflicted,
            "room_targeted" => Self::RoomTargeted,
            _ => Self::None,
        }
    }
}

/// One definition row.
#[derive(Debug)]
pub struct Def {
    /// The family: `attack`, `flare`, `status`, ...
    pub family: String,
    /// The def's name within its family: an attack name, a status, a
    /// resolution type, a spell number.
    pub name: String,
    /// Its role, where the family has roles.
    pub role: Role,
    /// Position in the family's first-match-wins lookup, 1-based.
    pub order: u32,
    /// Family-specific facts: `damaging`, `aoe`, `spawns`, `spell_name`,
    /// `group`, `tier`.
    pub extra: BTreeMap<String, String>,
    /// The plain-text pattern, after the extractor's stripping or the
    /// hand-port's substitution.
    pub pattern: String,
    /// Compiled, or `None` when the pattern could not be (see
    /// [`Defs::compile_failures`]) or the row is a name list with no pattern.
    pub regex: Option<Regex>,
    /// A hand-port's veto: when it matches the line, this def does not.
    pub veto: Option<Regex>,
}

impl Def {
    /// A family-specific fact, as text.
    #[must_use]
    pub fn extra(&self, key: &str) -> Option<&str> {
        self.extra.get(key).map(String::as_str)
    }

    /// A family-specific flag: `1` is true, anything else false.
    #[must_use]
    pub fn flag(&self, key: &str) -> bool {
        self.extra(key) == Some("1")
    }

    /// This def's captures over `text`, unless its veto refuses the line.
    fn captures<'t>(&self, text: &'t str) -> Option<Captures<'t>> {
        let caps = self.regex.as_ref()?.captures(text)?;
        if self.veto.as_ref().is_some_and(|v| v.is_match(text)) {
            return None;
        }
        Some(caps)
    }
}

/// The loaded tables.
#[derive(Debug, Default)]
pub struct Defs {
    by_family: BTreeMap<String, Vec<Def>>,
    /// `(family, name, pattern, error)` for every row that did not compile.
    failures: Vec<(String, String, String, String)>,
    /// `markup=1` rows with no entry in [`HAND_PORTED`].
    unported: Vec<(String, u32)>,
    /// `parser.rb:170`'s `SELF_IN_PATTERN`, compiled beside the tables so it
    /// shares their one static rather than owning a second.
    addresses_self: Option<Regex>,
    /// `parser.rb:252`'s `SWING_WEAPON_PATTERN`, likewise.
    swing_weapon: Option<Regex>,
}

fn table() -> &'static Defs {
    static DEFS: OnceLock<Defs> = OnceLock::new();
    DEFS.get_or_init(build)
}

/// The tables, loaded once.
#[must_use]
pub fn defs() -> &'static Defs {
    table()
}

/// A TSV column, or `None` when absent or empty.
fn column<'a>(cols: &[&'a str], index: usize) -> Option<&'a str> {
    cols.get(index)
        .map(|c| c.trim_end_matches('\r'))
        .filter(|c| !c.is_empty())
}

fn build() -> Defs {
    let mut out = Defs::default();
    for tsv in [ATTACKS_TSV, RESULTS_TSV, EFFECTS_TSV] {
        for line in tsv.lines().skip(1).filter(|l| !l.is_empty()) {
            let cols: Vec<&str> = line.splitn(7, '\t').collect();
            let (Some(family), Some(name)) = (column(&cols, 0), column(&cols, 1)) else {
                continue;
            };
            let role = column(&cols, 2).map_or(Role::None, Role::parse);
            let order = column(&cols, 3)
                .and_then(|o| o.parse::<u32>().ok())
                .unwrap_or(0);
            let flags = column(&cols, 4).unwrap_or("");
            let extra: BTreeMap<String, String> = column(&cols, 5)
                .unwrap_or("")
                .split(';')
                .filter_map(|kv| kv.split_once('='))
                .map(|(k, v)| (k.to_owned(), v.to_owned()))
                .collect();
            let mut pattern = column(&cols, 6).unwrap_or("").to_owned();

            // A hand-ported row is replaced at the same position. A
            // tag-reading row with no port is a fact, not an error, until the
            // test says otherwise.
            let port = HAND_PORTED
                .iter()
                .find(|p| p.family == family && p.order == order);
            let mut veto = None;
            match port {
                Some(p) => {
                    p.pattern.clone_into(&mut pattern);
                    veto = p.veto.and_then(|v| Regex::new(v).ok());
                }
                None if extra.get("markup").map(String::as_str) == Some("1") => {
                    out.unported.push((family.to_owned(), order));
                    continue;
                }
                None => {}
            }

            let regex = if pattern.is_empty() {
                None
            } else {
                let source = if flags.contains('i') {
                    format!("(?i){pattern}")
                } else {
                    pattern.clone()
                };
                match Regex::new(&source) {
                    Ok(re) => Some(re),
                    Err(e) => {
                        out.failures.push((
                            family.to_owned(),
                            name.to_owned(),
                            pattern.clone(),
                            e.to_string(),
                        ));
                        None
                    }
                }
            };

            out.by_family
                .entry(family.to_owned())
                .or_default()
                .push(Def {
                    family: family.to_owned(),
                    name: name.to_owned(),
                    role,
                    order,
                    extra,
                    pattern,
                    regex,
                    veto,
                });
        }
    }
    for defs in out.by_family.values_mut() {
        defs.sort_by_key(|d| d.order);
    }
    out.addresses_self = Regex::new(r"(?i)\b(?:at|towards?|upon|around|near|on)\s+your?\b").ok();
    // Lich's `(?<weapon>[^<]+?) at (?=<|\S)`: the capture ends at the ` at `
    // that precedes the target. `[^<]` and the lookahead both existed to stop
    // at a link tag; in plain text the target simply follows.
    out.swing_weapon = Regex::new(
        r"^You(?: take aim and)? (?:swing|fire) (?:an? |your |some )?(?<weapon>.+?) at \S",
    )
    .ok();
    out
}

impl Defs {
    /// Every def in a family, in first-match order. Empty for an unknown family.
    #[must_use]
    pub fn family(&self, name: &str) -> &[Def] {
        self.by_family.get(name).map_or(&[], Vec::as_slice)
    }

    /// The family names the tables declare.
    pub fn families(&self) -> impl Iterator<Item = &str> {
        self.by_family.keys().map(String::as_str)
    }

    /// Rows whose pattern did not compile: `(family, name, pattern, error)`.
    #[must_use]
    pub fn compile_failures(&self) -> &[(String, String, String, String)] {
        &self.failures
    }

    /// Flagged `markup=1` rows with no hand-port: `(family, order)`.
    #[must_use]
    pub fn unported(&self) -> &[(String, u32)] {
        &self.unported
    }

    /// The first def in `family` whose pattern matches `text`, with captures.
    ///
    /// This is `table.lookup.each { |pattern, ...| pattern.match(line) }` in
    /// every Lich def module: first match wins, in assembly order.
    #[must_use]
    pub fn first_match<'t>(&self, family: &str, text: &'t str) -> Option<(&Def, Captures<'t>)> {
        self.family(family)
            .iter()
            .find_map(|d| d.captures(text).map(|c| (d, c)))
    }

    /// The first def in `family` with the given role that matches `text`.
    #[must_use]
    pub fn first_match_with_role<'t>(
        &self,
        family: &str,
        role: Role,
        text: &'t str,
    ) -> Option<(&Def, Captures<'t>)> {
        self.family(family)
            .iter()
            .filter(|d| d.role == role)
            .find_map(|d| d.captures(text).map(|c| (d, c)))
    }

    /// Does any def in `family` match `text`?
    #[must_use]
    pub fn any_match(&self, family: &str, text: &str) -> bool {
        self.family(family)
            .iter()
            .any(|d| d.captures(text).is_some())
    }

    /// Does a def's own pattern text address us?
    ///
    /// `parser.rb:170`, `SELF_IN_PATTERN = /\b(?:at|towards?|upon|around|near|on)\s+your?\b/i`,
    /// tested against the def's **source**: some inbound defs name us in the
    /// literal rather than a capture (*"springs from the shadows and strikes
    /// at you!"*), and have an attacker capture and no target capture.
    #[must_use]
    pub fn pattern_addresses_self(&self, pattern: &str) -> bool {
        self.addresses_self
            .as_ref()
            .is_some_and(|re| re.is_match(pattern))
    }

    /// The weapon a 2p swing or fire line names in prose.
    ///
    /// `parser.rb:246-256`: swing lines do not capture the weapon in their
    /// def -- `You swing a kelyn-edged slim short sword at ...` -- and the
    /// processor needs it *"to claim pre-flares by weapon"*.
    #[must_use]
    pub fn swing_weapon(&self, text: &str) -> Option<String> {
        self.swing_weapon
            .as_ref()?
            .captures(text)?
            .name("weapon")
            .map(|m| m.as_str().to_owned())
    }

    /// The names in an `attack_class` role: which attacks are environmental,
    /// self-inflicted, or room-targeted (`attacks.rb:601-608`).
    pub fn attack_class(&self, role: Role) -> impl Iterator<Item = &str> {
        self.family("attack_class")
            .iter()
            .filter(move |d| d.role == role)
            .map(|d| d.name.as_str())
    }
}
