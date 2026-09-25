//! Reading the bestiary's seven TSVs into [`Creature`]s.
//!
//! Moved down out of `creature.rs` when the port took everything a template
//! says (2026-09-24): the types stay there, the parsing is here, and
//! `creature.rs` keeps the one `static` that holds the result.
//!
//! # By header name, and every column accounted for
//!
//! The first port read `creatures.tsv` by position and never read three of
//! its columns -- `height`, `speed` and `bcs` were extracted, shipped and
//! lost on this side, with nothing to say so. A [`Table`] looks each column
//! up by its header name and remembers which it was asked for. Afterwards,
//! any column no field took, and any name asked for that the header lacks,
//! is a problem. So is a row whose kind, category or list does not parse, or
//! that names a creature the bestiary does not have. They are collected
//! rather than dropped, and [`super::load_problems`] returns them; a test
//! holds that list empty.

use std::cell::RefCell;
use std::collections::{BTreeMap, BTreeSet};

use super::{
    Ability, Area, Attack, AttackCategory, Creature, Entry, Message, MessageKind, Stat, Treasure,
};

/// One row per creature.
const CREATURES_TSV: &str = include_str!("../../../data/creatures.tsv");
/// Where each creature is found, as room UID ranges.
const AREAS_TSV: &str = include_str!("../../../data/creature_areas.tsv");
/// Every attack, of every category.
const ATTACKS_TSV: &str = include_str!("../../../data/creature_attacks.tsv");
/// Every message, of every kind, with its key.
const MESSAGES_TSV: &str = include_str!("../../../data/creature_messages.tsv");
/// The tips, by section.
const INFO_TSV: &str = include_str!("../../../data/creature_info.tsv");
/// The plain lists: classes, equipment, defenses, notes, treasure.
const LISTS_TSV: &str = include_str!("../../../data/creature_lists.tsv");
/// The declared abilities.
const ABILITIES_TSV: &str = include_str!("../../../data/creature_abilities.tsv");

/// The twelve target-defense columns, in the header's order.
const TARGET_DEFENSES: [&str; 12] = [
    "bar_td", "cle_td", "emp_td", "pal_td", "ran_td", "sor_td", "wiz_td", "mje_td", "mne_td",
    "mjs_td", "mns_td", "mnm_td",
];

/// The loaded bestiary.
pub(super) struct Bestiary {
    /// Keyed by template id.
    pub(super) by_id: BTreeMap<String, Creature>,
    /// Strengths kept verbatim rather than dropped.
    pub(super) unparsed_strengths: usize,
    /// What was not read, and why.
    pub(super) problems: Vec<String>,
}

/// One TSV, read by header name.
struct Table<'a> {
    /// The file, for a problem's text.
    file: &'static str,
    /// Header name to column index.
    columns: BTreeMap<&'a str, usize>,
    /// The data rows, split.
    rows: Vec<Vec<&'a str>>,
    /// Every name a reader asked for.
    asked: RefCell<BTreeSet<String>>,
}

impl<'a> Table<'a> {
    fn new(file: &'static str, tsv: &'a str) -> Self {
        let mut lines = tsv.lines();
        let columns = lines
            .next()
            .unwrap_or_default()
            .split('\t')
            .enumerate()
            .map(|(index, name)| (name.trim(), index))
            .collect();
        let rows = lines
            .filter(|line| !line.is_empty())
            .map(|line| line.split('\t').collect())
            .collect();
        Self {
            file,
            columns,
            rows,
            asked: RefCell::new(BTreeSet::new()),
        }
    }

    /// A named column of one row, trimmed; empty and missing alike are `None`.
    fn get(&self, row: &[&'a str], name: &str) -> Option<&'a str> {
        self.asked.borrow_mut().insert(name.to_owned());
        let index = *self.columns.get(name)?;
        row.get(index)
            .map(|value| value.trim())
            .filter(|value| !value.is_empty())
    }

    /// A tri-state flag: `true`, `false`, or empty for **unknown**.
    fn flag(&self, row: &[&'a str], name: &str) -> Option<bool> {
        match self.get(row, name) {
            Some("true") => Some(true),
            Some("false") => Some(false),
            _ => None,
        }
    }

    /// A text column, unescaped (see [`unescape`]).
    fn text(&self, row: &[&'a str], name: &str) -> Option<String> {
        self.get(row, name).map(unescape)
    }

    fn stat(&self, row: &[&'a str], name: &str) -> Option<Stat> {
        self.get(row, name).and_then(Stat::parse)
    }

    fn number<T: std::str::FromStr>(&self, row: &[&'a str], name: &str) -> Option<T> {
        self.get(row, name).and_then(|value| value.parse().ok())
    }

    /// Every column never asked for, and every name asked for that the
    /// header does not have.
    fn unread(&self, problems: &mut Vec<String>) {
        let asked = self.asked.borrow();
        for name in self.columns.keys() {
            if !asked.contains(*name) {
                problems.push(format!("{}: column `{name}` is never read", self.file));
            }
        }
        for name in asked.iter() {
            if !self.columns.contains_key(name.as_str()) {
                problems.push(format!("{}: no column `{name}`", self.file));
            }
        }
    }
}

/// Undo the extractor's escaping: `\n`, `\t` and `\\`.
///
/// A TSV cell cannot hold a newline, and the source has them: 26 descriptions
/// and 13 tips run to several paragraphs, and 7 attack and trigger messages
/// are two game lines. The extractor escapes rather than flattening, so the
/// line breaks survive to here (`extract_creatures.rb`, `clean`).
fn unescape(cell: &str) -> String {
    let mut out = String::with_capacity(cell.len());
    let mut chars = cell.chars();
    while let Some(c) = chars.next() {
        if c != '\\' {
            out.push(c);
            continue;
        }
        match chars.next() {
            Some('n') => out.push('\n'),
            Some('t') => out.push('\t'),
            // An escaped backslash, or a lone one closing the cell.
            Some('\\') | None => out.push('\\'),
            Some(other) => {
                out.push('\\');
                out.push(other);
            }
        }
    }
    out
}

pub(super) fn build() -> Bestiary {
    let mut problems = Vec::new();
    let mut by_id = creature_table(&mut problems);

    let tables = [
        areas(&mut by_id),
        attacks(&mut by_id),
        messages(&mut by_id),
        info(&mut by_id),
        lists(&mut by_id),
        abilities(&mut by_id),
    ];
    let mut unparsed_strengths = 0;
    for (table, unplaced, unparsed) in &tables {
        table.unread(&mut problems);
        problems.extend(unplaced.iter().cloned());
        unparsed_strengths += unparsed;
    }

    Bestiary {
        by_id,
        unparsed_strengths,
        problems,
    }
}

/// What one joined table left: the table, the rows it could not place, and
/// how many strengths it kept verbatim.
type Joined = (Table<'static>, Vec<String>, usize);

fn creature_table(problems: &mut Vec<String>) -> BTreeMap<String, Creature> {
    let t = Table::new("creatures.tsv", CREATURES_TSV);
    let mut by_id = BTreeMap::new();
    for row in &t.rows {
        let (Some(id), Some(name)) = (t.get(row, "id"), t.get(row, "name")) else {
            problems.push(format!("creatures.tsv: a row with no id or name: {row:?}"));
            continue;
        };
        let target_defense = TARGET_DEFENSES
            .into_iter()
            .filter_map(|key| t.stat(row, key).map(|stat| (key.to_owned(), stat)))
            .collect();
        by_id.insert(
            id.to_owned(),
            Creature {
                id: id.to_owned(),
                name: name.to_owned(),
                noun: t.text(row, "noun"),
                level: t.number(row, "level"),
                max_hp: t.stat(row, "max_hp"),
                family: t.text(row, "family"),
                kind: t.text(row, "type"),
                size: t.text(row, "size"),
                height: t.number(row, "height"),
                speed: t.stat(row, "speed"),
                undead: t.flag(row, "undead"),
                blood: t.flag(row, "blood"),
                bones: t.flag(row, "bones"),
                limbs: t.flag(row, "limbs"),
                witherable: t.flag(row, "witherable"),
                sympathy: t.flag(row, "sympathy"),
                muggable: t.flag(row, "muggable"),
                sleepable: t.flag(row, "sleepable"),
                bcs: t.flag(row, "bcs"),
                boss: t.flag(row, "boss") == Some(true),
                boss_type: t.text(row, "boss_type"),
                asg: t.text(row, "asg"),
                melee_ds: t.stat(row, "melee"),
                ranged_ds: t.stat(row, "ranged"),
                bolt_ds: t.stat(row, "bolt"),
                udf: t.stat(row, "udf"),
                target_defense,
                treasure: Treasure {
                    skin: t.text(row, "skin"),
                    skins: t.flag(row, "skins"),
                    coins: t.flag(row, "coins"),
                    boxes: t.flag(row, "boxes"),
                    gems: t.flag(row, "gems"),
                    magic_items: t.flag(row, "magic_items"),
                    blunt_required: t.flag(row, "blunt_required"),
                    ..Treasure::default()
                },
                special_other: t.text(row, "special_other"),
                url: t.text(row, "url"),
                picture: t.text(row, "picture"),
                schema_version: t.number(row, "schema_version"),
                ..Creature::default()
            },
        );
    }
    t.unread(problems);
    by_id
}

/// The creature a joined row names, or a problem saying it is missing.
fn owner<'c>(
    by_id: &'c mut BTreeMap<String, Creature>,
    file: &str,
    id: Option<&str>,
    unplaced: &mut Vec<String>,
) -> Option<&'c mut Creature> {
    let found = id.and_then(|id| by_id.get_mut(id));
    if found.is_none() {
        unplaced.push(format!("{file}: a row for an unknown creature {id:?}"));
    }
    found
}

fn areas(by_id: &mut BTreeMap<String, Creature>) -> Joined {
    let t = Table::new("creature_areas.tsv", AREAS_TSV);
    let mut unplaced = Vec::new();
    for row in &t.rows {
        let span = (
            t.text(row, "area"),
            t.number(row, "uid_lo"),
            t.number(row, "uid_hi"),
        );
        let Some(creature) = owner(by_id, t.file, t.get(row, "creature_id"), &mut unplaced) else {
            continue;
        };
        let (Some(name), Some(uid_low), Some(uid_high)) = span else {
            unplaced.push(format!("{}: an incomplete span {row:?}", t.file));
            continue;
        };
        creature.areas.push(Area {
            name,
            uid_low,
            uid_high,
        });
    }
    (t, unplaced, 0)
}

fn attacks(by_id: &mut BTreeMap<String, Creature>) -> Joined {
    let t = Table::new("creature_attacks.tsv", ATTACKS_TSV);
    let mut unplaced = Vec::new();
    let mut unparsed = 0;
    for row in &t.rows {
        let category = t.get(row, "category").and_then(AttackCategory::parse);
        let attack = (category, t.text(row, "name"));
        let attack_strength_raw = t.text(row, "as_raw");
        let casting_strength_raw = t.text(row, "cs_raw");
        let (attack_strength, casting_strength) = (t.stat(row, "as"), t.stat(row, "cs"));
        let (note, kind) = (t.text(row, "note"), t.text(row, "type"));
        let Some(creature) = owner(by_id, t.file, t.get(row, "creature_id"), &mut unplaced) else {
            continue;
        };
        let (Some(category), Some(name)) = attack else {
            unplaced.push(format!(
                "{}: an unknown category or no name {row:?}",
                t.file
            ));
            continue;
        };
        unparsed += usize::from(attack_strength_raw.is_some())
            + usize::from(casting_strength_raw.is_some());
        creature.attacks.push(Attack {
            category,
            name,
            attack_strength,
            attack_strength_raw,
            casting_strength,
            casting_strength_raw,
            note,
            kind,
        });
    }
    (t, unplaced, unparsed)
}

fn messages(by_id: &mut BTreeMap<String, Creature>) -> Joined {
    let t = Table::new("creature_messages.tsv", MESSAGES_TSV);
    let mut unplaced = Vec::new();
    for row in &t.rows {
        let kind = t.get(row, "kind").and_then(MessageKind::parse);
        let (key, text) = (t.text(row, "key"), t.text(row, "text"));
        let Some(creature) = owner(by_id, t.file, t.get(row, "creature_id"), &mut unplaced) else {
            continue;
        };
        let (Some(kind), Some(text)) = (kind, text) else {
            unplaced.push(format!("{}: an unknown kind or no text {row:?}", t.file));
            continue;
        };
        creature
            .messages
            .entry(kind)
            .or_default()
            .push(Message { key, text });
    }
    (t, unplaced, 0)
}

fn info(by_id: &mut BTreeMap<String, Creature>) -> Joined {
    let t = Table::new("creature_info.tsv", INFO_TSV);
    let mut unplaced = Vec::new();
    for row in &t.rows {
        let tip = (t.text(row, "section"), t.text(row, "text"));
        let Some(creature) = owner(by_id, t.file, t.get(row, "creature_id"), &mut unplaced) else {
            continue;
        };
        let (Some(section), Some(text)) = tip else {
            unplaced.push(format!("{}: no section or no text {row:?}", t.file));
            continue;
        };
        creature.info.entry(section).or_default().push(text);
    }
    (t, unplaced, 0)
}

fn lists(by_id: &mut BTreeMap<String, Creature>) -> Joined {
    let t = Table::new("creature_lists.tsv", LISTS_TSV);
    let mut unplaced = Vec::new();
    for row in &t.rows {
        let list = t.get(row, "list");
        let (name, note) = (t.text(row, "name"), t.text(row, "note"));
        let Some(creature) = owner(by_id, t.file, t.get(row, "creature_id"), &mut unplaced) else {
            continue;
        };
        let Some(name) = name else {
            unplaced.push(format!("{}: an entry with no name {row:?}", t.file));
            continue;
        };
        let into = match list {
            Some("otherclass") => &mut creature.other_class,
            Some("equipment") => &mut creature.equipment,
            Some("immunities") => &mut creature.immunities,
            Some("defensive_spells") => &mut creature.defensive_spells,
            Some("special_defenses") => &mut creature.special_defenses,
            Some("special_notes") => &mut creature.special_notes,
            Some("armaments") => &mut creature.treasure.armaments,
            Some("treasure_other") => &mut creature.treasure.other,
            Some("defensive_abilities") => {
                creature.defensive_abilities.push(Entry { name, note });
                continue;
            }
            other => {
                unplaced.push(format!("{}: an unknown list {other:?}", t.file));
                continue;
            }
        };
        if note.is_some() {
            unplaced.push(format!("{}: a note on a plain list {row:?}", t.file));
        }
        into.push(name);
    }
    (t, unplaced, 0)
}

fn abilities(by_id: &mut BTreeMap<String, Creature>) -> Joined {
    let t = Table::new("creature_abilities.tsv", ABILITIES_TSV);
    let mut unplaced = Vec::new();
    for row in &t.rows {
        let named = (t.text(row, "id"), t.text(row, "name"));
        let effects = t
            .get(row, "effects")
            .map(|all| {
                all.split(';')
                    .map(|effect| match effect.split_once('=') {
                        Some((flag, value)) => (flag.to_owned(), Some(value.to_owned())),
                        None => (effect.to_owned(), None),
                    })
                    .collect()
            })
            .unwrap_or_default();
        let ability = (
            t.text(row, "type"),
            t.text(row, "target"),
            t.number(row, "typical_duration_s"),
            t.flag(row, "dispellable"),
            t.text(row, "notes"),
        );
        let Some(creature) = owner(by_id, t.file, t.get(row, "creature_id"), &mut unplaced) else {
            continue;
        };
        let (Some(id), Some(name)) = named else {
            unplaced.push(format!("{}: an ability with no id or name {row:?}", t.file));
            continue;
        };
        let (kind, target, typical_duration_s, dispellable, notes) = ability;
        creature.abilities.push(Ability {
            id,
            name,
            kind,
            target,
            typical_duration_s,
            dispellable,
            effects,
            notes,
        });
    }
    (t, unplaced, 0)
}
