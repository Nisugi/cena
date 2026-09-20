//! What kind of thing is this? Type and sellable classification.
//!
//! Ports `GameObj#type` and `#sellable` (`lib/common/gameobj.rb:233-286`) over
//! `data/gameobj-data.tsv`, which is already transcribed, compile-tested
//! (`tests/gameobj_patterns.rs`) and committed. This is the consumer that
//! table was prepared for.
//!
//! # The matching rule, and the two arms that never fire
//!
//! `matching_data_keys` (`gameobj.rb:1504-1512`):
//!
//! ```ruby
//! matches = (@name =~ entry[:name] || @noun =~ entry[:noun] || obj_full_name =~ entry[:full_name])
//! excluded = entry[:exclude] && @name =~ entry[:exclude]
//! matches && !excluded
//! ```
//!
//! So: **match on name OR noun, veto on exclude**, and an object may match
//! several categories at once. MEASURED against the transcribed data, two of
//! the four fields are dead:
//!
//! | Field | Rows | Read by `matching_data_keys`? |
//! |---|---|---|
//! | `name` | 62 | yes |
//! | `noun` | 26 | yes |
//! | `exclude` | 23 | yes, as a veto |
//! | `suffix` | 2 | **no** |
//! | `full_name` | **0** | yes, but nothing to match |
//!
//! `suffix` carries `^(s)$` twice -- plural matching for the furrier and skin
//! categories -- and `matching_data_keys` never looks at it. `full_name` is
//! read but the data declares none, so the arm cannot fire. Both are recorded
//! in `inventory/10` rather than ported: implementing a matcher for data that
//! does not exist would be inventing behaviour, not porting it.
//!
//! # What is NOT ported: the memo cache
//!
//! `gameobj.rb:260` memoizes on `"#{noun}|#{name}|#{full_name}"` because every
//! call re-scans ~100 regexes and `Inventory::Item` exposes `type` as public
//! API. That is a real cost in Ruby. Here the scan is over compiled patterns
//! held in one table, and a cache keyed by object identity would be
//! per-session state in a crate that holds none -- so a caller that needs it
//! memoizes at its own layer, where the lifetime is obvious.
//!
//! # Degrading rather than crashing
//!
//! Lich's `load_data` nils the table on a missing file and every caller guards
//! for it, because *"a broken gameobj-data.xml yields nil instead of crashing
//! every #type caller"*. The equivalent here is that the table is
//! `include_str!`-ed at compile time, so it cannot be missing -- and a pattern
//! that fails to compile is skipped rather than panicking, with
//! [`ObjectTypes::skipped`] reporting how many. Rule 2.2's floor: nothing is
//! dropped without saying so.

use std::collections::BTreeSet;
use std::sync::OnceLock;

use regex::Regex;

/// The transcribed classification table.
const GAMEOBJ_TSV: &str = include_str!("../../data/gameobj-data.tsv");

/// Which classification a row belongs to.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Classification {
    /// What kind of thing it is: `herb`, `gem`, `aggressive npc`.
    Type,
    /// Who will buy it: `furrier`, `gemshop`.
    Sellable,
}

impl Classification {
    /// The spelling the TSV's first column uses.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Type => "type",
            Self::Sellable => "sellable",
        }
    }
}

/// One category's patterns.
#[derive(Debug)]
struct Category {
    kind: Classification,
    name: String,
    /// Matched against the object's name.
    on_name: Vec<Regex>,
    /// Matched against the object's noun.
    on_noun: Vec<Regex>,
    /// Vetoes a match, tested against the **name** only (`gameobj.rb:1509`).
    exclude: Vec<Regex>,
}

impl Category {
    /// Does this object fall in this category?
    fn matches(&self, noun: &str, name: &str) -> bool {
        let hit = self.on_name.iter().any(|re| re.is_match(name))
            || self.on_noun.iter().any(|re| re.is_match(noun));
        // **The veto reads the NAME even when the match came from the noun.**
        // Faithful to `gameobj.rb:1509`, which tests `@name =~ entry[:exclude]`
        // unconditionally. Worth stating because it looks like an oversight and
        // the data relies on it: `alchemy equipment` matches the noun
        // `mortar` and excludes the name `small blue clay mortar`.
        hit && !self.exclude.iter().any(|re| re.is_match(name))
    }
}

/// The classification table, compiled once.
///
/// Also counts the rows skipped because their pattern did not compile, so the
/// number is reportable rather than silent.
struct Table {
    categories: Vec<Category>,
    skipped: usize,
}

fn table() -> &'static Table {
    static TABLE: OnceLock<Table> = OnceLock::new();
    TABLE.get_or_init(build_table)
}

/// Parse the TSV into compiled categories.
fn build_table() -> Table {
    let mut categories: Vec<Category> = Vec::new();
    let mut skipped = 0;

    for line in GAMEOBJ_TSV.lines().skip(1).filter(|l| !l.is_empty()) {
        let mut cols = line.splitn(4, '\t');
        let (Some(kind), Some(name), Some(field), Some(pattern)) =
            (cols.next(), cols.next(), cols.next(), cols.next())
        else {
            skipped += 1;
            continue;
        };
        let kind = match kind {
            "type" => Classification::Type,
            "sellable" => Classification::Sellable,
            _ => {
                skipped += 1;
                continue;
            }
        };
        // `suffix` is in the data and `matching_data_keys` never reads it; see
        // the module docs. Skipped deliberately rather than silently, so the
        // count says so.
        if field == "suffix" {
            skipped += 1;
            continue;
        }
        let Ok(regex) = Regex::new(pattern) else {
            skipped += 1;
            continue;
        };

        // Find-or-create, in two statements because the `find` borrow must
        // end before the `push`.
        if !categories.iter().any(|c| c.kind == kind && c.name == name) {
            categories.push(Category {
                kind,
                name: name.to_owned(),
                on_name: Vec::new(),
                on_noun: Vec::new(),
                exclude: Vec::new(),
            });
        }
        let Some(entry) = categories
            .iter_mut()
            .find(|c| c.kind == kind && c.name == name)
        else {
            skipped += 1;
            continue;
        };
        match field {
            "name" => entry.on_name.push(regex),
            "noun" => entry.on_noun.push(regex),
            "exclude" => entry.exclude.push(regex),
            _ => skipped += 1,
        }
    }

    Table {
        categories,
        skipped,
    }
}

/// Classification answers for one object.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ObjectTypes {
    /// Every `type` category matched, ordered.
    ///
    /// **A set, because an object can be several things.** Lich joins them
    /// with commas into one string (`gameobj.rb:263`) and callers then
    /// `split(',')` to ask a question (`type?`); returning the set directly
    /// removes a round trip through a format nobody wants.
    pub types: BTreeSet<String>,
    /// Every `sellable` category matched.
    pub sellable: BTreeSet<String>,
}

impl ObjectTypes {
    /// Is this object of the named type?
    ///
    /// `GameObj#type?` (`gameobj.rb:270-272`).
    #[must_use]
    pub fn is(&self, type_name: &str) -> bool {
        self.types.contains(type_name)
    }

    /// Will the named shop buy this?
    #[must_use]
    pub fn sells_to(&self, shop: &str) -> bool {
        self.sellable.contains(shop)
    }

    /// Nothing matched.
    #[must_use]
    pub fn is_unclassified(&self) -> bool {
        self.types.is_empty() && self.sellable.is_empty()
    }

    /// How many table rows were skipped at load.
    ///
    /// Non-zero means a pattern did not compile under this crate's regex
    /// dialect, or a row was malformed, or it was a `suffix` row that
    /// `matching_data_keys` would not have read anyway. Exposed so a caller
    /// can report it rather than silently classifying against a partial table
    /// (Rule 2.2).
    #[must_use]
    pub fn skipped() -> usize {
        table().skipped
    }
}

/// Classify one object by its noun and name.
///
/// `noun` is the wire's `noun=` attribute; `name` is the display text. Both
/// are matched, because the data uses whichever is more discriminating --
/// `gem` keys on nouns, `aggressive npc` on a 40 KB name alternation.
#[must_use]
pub fn classify(noun: &str, name: &str) -> ObjectTypes {
    let mut out = ObjectTypes::default();
    for category in &table().categories {
        if !category.matches(noun, name) {
            continue;
        }
        match category.kind {
            Classification::Type => out.types.insert(category.name.clone()),
            Classification::Sellable => out.sellable.insert(category.name.clone()),
        };
    }
    out
}

/// Every category name the table declares, for one classification.
///
/// So a caller can discover what it may ask about rather than guessing at
/// string literals.
#[must_use]
pub fn categories(kind: Classification) -> BTreeSet<&'static str> {
    table()
        .categories
        .iter()
        .filter(|c| c.kind == kind)
        .map(|c| c.name.as_str())
        .collect()
}
