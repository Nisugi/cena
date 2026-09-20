//! Context menus: turning a `<menu>` of bare coordinates into labelled
//! commands.
//!
//! # A menu carries no labels
//!
//! Right-click menus are **coordinate lookups**. The client asks with
//! `_menu #<exist id>`; the game answers with `<menu>` holding `<mi coord=>`
//! items and no text whatsoever:
//!
//! ```text
//! <menu id="1" ><mi coord="2524,1543"/><mi coord="2524,1546"/></menu>
//! ```
//!
//! Each coordinate is a key into a command dictionary that maps it to a
//! label and a command template. Without the dictionary a menu is a list of
//! numbers, which is why this table is data in the crate rather than
//! something a frontend is expected to find.
//!
//! # The dictionary does NOT arrive on the wire
//!
//! MEASURED over the author's six most recent logins: `cmdlist=0`, `cli=0`,
//! while `menu=2` and `mi=60`. It is **static data a client ships**, which is
//! how the author came to have it and how `VellumFE` uses it
//! (`src/cmdlist.rs:29`, loaded from disk). `plan/15` records the wiki's
//! claim to the contrary and why the corpus overrules it.
//!
//! # Which copy of the dictionary
//!
//! From **the live Wrayth client's own settings**,
//! `%APPDATA%/Wrayth/GS4/cmdlist1.xml`: **1,106** entries across 43
//! categories, each coordinate unique, `timestamp="1788300900.1.1.1"`
//! (2026-09-05).
//!
//! **Not** the copy `VellumFE` ships (`defaults/globals/cmdlist1.xml`), which
//! is `timestamp="1069343154.1.1.1"` -- **2003**, 592 entries. That file was
//! used first and the corpus caught it: 8 of the 60 coordinates in a real
//! menu were missing from it, every one above `2524,2500`. They are UCS
//! attacks (`jab`, `grapple`), focused multistrike, and `symbol of sleep` --
//! a decade and a half of commands the old file predates.
//!
//! The wire offers no fallback: MEASURED over all **425** `<mi>` in the
//! author's September logs, the only attributes are `coord` (425), `noun`
//! (10) and `menu_cat` (8). **Zero** carry a label or a command. So a
//! coordinate the dictionary lacks cannot be resolved at all, which is why
//! the freshest available copy is the one to ship.
//!
//! # Menu depth: three levels, and only one branch reaches it
//!
//! The category string encodes the path, with two separators:
//!
//! | Depth | Shape | Example | Entries |
//! |---|---|---|---|
//! | 1 | bare | `6` | 634 |
//! | 2 | `_` | `6_focused multistrike` | 453 |
//! | 3 | `-` | `5_roleplay-swear` | **19** |
//!
//! `5_roleplay-swear` is the **only** three-level category in all 1,106
//! entries -- `menu -> 5 -> roleplay -> swear`. See
//! [`MenuCommand::category_path`].
//!
//! # One malformed row, left as the source has it
//!
//! `2524,2014` reads `menu="7" command="7" menu_cat="drag"`: the label and
//! command are both `"7"` and the category is `"drag"`, which is plainly the
//! label. The fields are scrambled in Simutronics' own file, and it is
//! **unchanged in the 2026 copy**, so this is not staleness.
//!
//! It is kept verbatim. Correcting source data on inference is how a silent
//! divergence from the game starts, and the cost is one menu entry that
//! displays as `7`.
//!
//! # Why this lives in the model
//!
//! > **AUTHOR, 2026-09-20:** *"Cena will be building menus, yes menus are
//! > frontend facing, cena will support multiple frontends and will keep the
//! > layers separate like vellum does, so we still need menu building code in
//! > core somewhere that will feed any frontend."*
//!
//! So the resolution -- coordinate to label to command -- happens once, here,
//! and every frontend renders the same answer. What a frontend still owns is
//! presentation: ordering within a category, icons, and how a radial or a
//! list is drawn.

use std::collections::BTreeMap;
use std::sync::OnceLock;

use cena_protocol::{CmdListEntry, Menu, MenuItem};

const MENU_COMMANDS_TSV: &str = include_str!("../../data/menu_commands.tsv");

/// One dictionary entry: what a coordinate means.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MenuCommand {
    /// `coord=`, e.g. `"2524,1543"`.
    pub coord: String,
    /// The display template, e.g. `"attack @"`.
    pub label: String,
    /// The command template, e.g. `"attack #"`.
    pub command: String,
    /// The dictionary's category, e.g. `"6"` or `"5_roleplay"`.
    ///
    /// **The wire may override this**: an `<mi>` carries its own optional
    /// `menu_cat`, and when present it wins. See [`ResolvedItem::category`].
    pub category: String,
}

/// Split a category into its menu path.
///
/// `"5_roleplay-swear"` becomes `["5", "roleplay", "swear"]`; a bare `"6"`
/// becomes `["6"]`. The separators are the dictionary's own: `_` for the
/// second level and `-` for the third.
///
/// Parsed here so a frontend building a nested menu does not each reimplement
/// it -- the same reason the whole resolution lives in the model.
#[must_use]
pub fn category_path(category: &str) -> Vec<&str> {
    category
        .split('_')
        .flat_map(|part| part.split('-'))
        .filter(|part| !part.is_empty())
        .collect()
}

impl MenuCommand {
    /// This entry's menu path. See [`category_path`].
    #[must_use]
    pub fn category_path(&self) -> Vec<&str> {
        category_path(&self.category)
    }
}

/// One menu item, resolved against the dictionary.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolvedItem {
    /// The coordinate the wire sent.
    pub coord: String,
    /// The label with `@` replaced by the noun, e.g. `"attack kobold"`.
    ///
    /// `None` when the coordinate is not in the dictionary -- a menu entry
    /// Simutronics added since the file was cut. The item is still carried,
    /// because dropping it would silently shorten the player's menu.
    pub label: Option<String>,
    /// The command to send, ready but for `%`. `None` for an unknown
    /// coordinate.
    ///
    /// See [`MenuCommands::command_for`] for the substitution rules and for
    /// what `%` is.
    pub command: Option<String>,
    /// Which category this item belongs under.
    ///
    /// The wire's `menu_cat` when the item carried one, else the
    /// dictionary's, else `None` for an unknown coordinate with no override.
    pub category: Option<String>,
    /// Whether the command still contains a `%` for the caller to fill.
    pub needs_secondary: bool,
}

/// Dictionary rows the SERVER taught us, layered over the shipped table.
///
/// # Why an overlay and not an edit
///
/// The shipped table is `include_str!`'d into the binary, so a running
/// process cannot write to it -- the path does not exist on a user's machine.
/// Learned rows therefore have to live somewhere else whatever one's
/// preference, and this is that place.
///
/// It is also a mutable thing, which the shipped table deliberately is not:
/// `plan/05` Rule 5.2 forbids mutable process globals, so the static holding
/// the baseline is immutable and this rides beside it, owned by whoever holds
/// the session rather than by the process.
///
/// # The merge is additive
///
/// A `<cmdlist>` push is a DELTA, not a full dump:
///
/// > **AUTHOR, 2026-09-20:** *"makes sense cause it's a lot of commands to
/// > push."*
///
/// The dictionary is 1,106 rows and the one captured push carried 2, which is
/// consistent with that but does not prove it alone -- only one push has ever
/// been observed. So rows are ADDED and never removed, which is the safe
/// direction under either reading: if it is a delta, adding is correct; if it
/// were ever a full dump, the cost is a stale row for a coordinate the server
/// no longer sends, and a coordinate the server does not send never appears
/// in a `<menu>` to be resolved.
///
/// A row that repeats a coordinate REPLACES it. The server is the authority
/// on what a coordinate means, including when it changes meaning, and both
/// captured rows were already byte-identical to the shipped table -- so
/// replacement is what keeps a genuine change from being ignored.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct LearnedCommands {
    entries: BTreeMap<String, MenuCommand>,
    version: Option<String>,
}

impl LearnedCommands {
    /// Absorb one `<cmdlist>` push.
    pub fn apply(&mut self, entries: &[CmdListEntry]) {
        let rows: Vec<MenuCommand> = entries
            .iter()
            .map(|entry| MenuCommand {
                coord: entry.coord.clone(),
                label: entry.label.clone(),
                command: entry.command.clone(),
                category: entry.category.clone(),
            })
            .collect();
        self.absorb(&rows);
    }

    /// Absorb rows that are already [`MenuCommand`]s.
    ///
    /// The same merge as [`apply`], for rows that did not arrive as a
    /// `<cmdlist>` -- reading back the supplemental file a previous session
    /// wrote, whose rows came from the wire but not from *this* connection.
    ///
    /// Both paths go through here so the rule cannot drift between them: a
    /// coordinate REPLACES, and an empty coordinate is skipped because it is
    /// the key and a row without one could never be looked up.
    ///
    /// [`apply`]: Self::apply
    pub fn absorb(&mut self, rows: &[MenuCommand]) {
        for row in rows {
            if row.coord.is_empty() {
                continue;
            }
            self.entries.insert(row.coord.clone(), row.clone());
        }
    }

    /// Record the dictionary version from `<cmdtimestamp>`.
    pub fn set_version(&mut self, version: &str) {
        if !version.is_empty() {
            self.version = Some(version.to_owned());
        }
    }

    /// The dictionary version the server last stated.
    ///
    /// `None` until one arrives. The shipped table's own version is not
    /// assumed here: it is a property of the file, and claiming it as the
    /// session's would make a stale cache look current.
    #[must_use]
    pub fn version(&self) -> Option<&str> {
        self.version.as_deref()
    }

    /// One learned row.
    #[must_use]
    pub fn entry(&self, coord: &str) -> Option<&MenuCommand> {
        self.entries.get(coord)
    }

    /// Every learned row, in coordinate order.
    pub fn all(&self) -> impl Iterator<Item = &MenuCommand> {
        self.entries.values()
    }

    /// How many rows have been learned.
    #[must_use]
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Whether nothing has been learned.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Rows that differ from the shipped table, or are absent from it.
    ///
    /// What a supplemental file needs to carry, and what a person folding
    /// updates back into the shipped TSV would want to look at: a row
    /// identical to the baseline is noise.
    pub fn novel(&self) -> impl Iterator<Item = &MenuCommand> {
        let shipped = MenuCommands::get();
        self.entries
            .values()
            .filter(move |row| shipped.entry(&row.coord) != Some(*row))
    }
}

/// The command dictionary, keyed by coordinate.
pub struct MenuCommands {
    by_coord: BTreeMap<String, MenuCommand>,
    version: Option<String>,
}

fn dictionary() -> &'static MenuCommands {
    static DICT: OnceLock<MenuCommands> = OnceLock::new();
    DICT.get_or_init(build)
}

fn build() -> MenuCommands {
    let mut by_coord = BTreeMap::new();
    let mut version = None;
    for line in MENU_COMMANDS_TSV.lines() {
        // The `<cmdtimestamp>` this table was cut at. Carried IN the file
        // rather than in a comment in this source, because it is a property
        // of the data: a release that folds learned rows in updates the file,
        // and a version that lived here would have to be edited in lockstep
        // by whoever remembered to.
        if let Some(stamp) = line.strip_prefix("# cmdtimestamp	") {
            version = Some(stamp.trim().to_owned());
            continue;
        }
        if line.trim().is_empty() || line.starts_with('#') || line == "coord	label	command	category"
        {
            continue;
        }
        let cols: Vec<&str> = line.split('\t').collect();
        let (Some(coord), Some(label), Some(command), Some(category)) =
            (cols.first(), cols.get(1), cols.get(2), cols.get(3))
        else {
            continue;
        };
        by_coord.insert(
            (*coord).to_owned(),
            MenuCommand {
                coord: (*coord).to_owned(),
                label: (*label).to_owned(),
                command: (*command).to_owned(),
                category: (*category).to_owned(),
            },
        );
    }
    MenuCommands { by_coord, version }
}

impl MenuCommands {
    /// The `<cmdtimestamp>` version the shipped table was cut at.
    ///
    /// `None` only if the file lost its version line, which
    /// `the_shipped_table_states_its_version` fails on.
    ///
    /// This is the **baseline** of the staleness chain: a release that folds
    /// learned rows into this file advances it, and the supplemental file is
    /// then discardable up to that point.
    #[must_use]
    pub fn version(&self) -> Option<&str> {
        self.version.as_deref()
    }

    /// The dictionary.
    #[must_use]
    pub fn get() -> &'static Self {
        dictionary()
    }

    /// One entry by coordinate.
    #[must_use]
    pub fn entry(&self, coord: &str) -> Option<&MenuCommand> {
        self.by_coord.get(coord)
    }

    /// Every coordinate the dictionary knows, in order.
    pub fn coords(&self) -> impl Iterator<Item = &str> {
        self.by_coord.keys().map(String::as_str)
    }

    /// How many entries the dictionary holds.
    #[must_use]
    pub fn len(&self) -> usize {
        self.by_coord.len()
    }

    /// Whether the dictionary is empty. It never is.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.by_coord.is_empty()
    }

    /// The command for one coordinate, with placeholders filled.
    ///
    /// Three placeholders, ported from `VellumFE/src/cmdlist.rs:119-139`:
    ///
    /// * `@` becomes the **noun**, e.g. `"attack @"` -> `"attack kobold"`.
    /// * `#` becomes `#` **followed by the exist id**, e.g. `"attack #"` ->
    ///   `"attack #12345"`. The hash stays: it is the game's syntax for
    ///   addressing an object by id, not a placeholder marker.
    /// * `%` is a **secondary argument the caller supplies** -- a second item
    ///   for `"transfer # %"`, a demeanour for `"demeanor % @"`. Left in
    ///   place when `secondary` is `None`, so a caller can see it is still
    ///   needed rather than sending a malformed command.
    ///
    /// MEASURED across the 592 entries: 336 use `#`, 120 use `@`, 20 use `%`.
    #[must_use]
    pub fn command_for(
        &self,
        coord: &str,
        noun: &str,
        exist: &str,
        secondary: Option<&str>,
    ) -> Option<String> {
        let entry = self.entry(coord)?;
        Some(substitute(&entry.command, noun, exist, secondary))
    }

    /// Resolve one wire item, consulting what the server has taught us first.
    ///
    /// An unknown coordinate resolves to an item with no label and no
    /// command rather than to nothing: the game sent it, so the player has
    /// it, and a silently shortened menu is worse than a visibly unnamed
    /// entry (Rule 2.2).
    #[must_use]
    pub fn resolve_item(
        &self,
        item: &MenuItem,
        exist: &str,
        secondary: Option<&str>,
        learned: Option<&LearnedCommands>,
    ) -> ResolvedItem {
        let coord = item.coord.clone().unwrap_or_default();
        // **The server's row wins.** A `<cmdlist>` push is the game stating
        // what a coordinate means now; the shipped table is a cache of older
        // pushes, and can only be staler.
        let entry = learned
            .and_then(|l| l.entry(&coord))
            .or_else(|| self.entry(&coord));
        // The wire's noun is the object's own; the dictionary's `@` is a
        // slot for it.
        let noun = item.noun.as_deref().unwrap_or_default();
        let command = entry.map(|e| substitute(&e.command, noun, exist, secondary));
        ResolvedItem {
            needs_secondary: command.as_deref().is_some_and(|c| c.contains('%')),
            // `label.replace('@', "")` on an item with no noun leaves
            // `"attack "` -- the template's separating space with nothing
            // after it. MEASURED: most `<mi>` carry no `noun` at all, so
            // trimming is the common path rather than an edge case.
            label: entry.map(|e| e.label.replace('@', noun).trim().to_owned()),
            command,
            // **The wire wins.** An `<mi menu_cat=>` is the game saying where
            // this item belongs in THIS menu, which is newer and more
            // specific than the shipped dictionary's guess.
            category: item
                .menu_cat
                .clone()
                .or_else(|| entry.map(|e| e.category.clone())),
            coord,
        }
    }

    /// Resolve a whole `<menu>`, in wire order.
    ///
    /// **`exist` is the caller's**, not the menu's. `Menu::id` is a request
    /// sequence number -- the corpus shows `id="1"`, `"2"`, `"3"` -- so it
    /// answers *which request is this* and never *which object*. The object
    /// is whatever the player right-clicked, which only the caller knows.
    #[must_use]
    pub fn resolve(
        &self,
        menu: &Menu,
        exist: &str,
        secondary: Option<&str>,
        learned: Option<&LearnedCommands>,
    ) -> Vec<ResolvedItem> {
        menu.items
            .iter()
            .map(|item| self.resolve_item(item, exist, secondary, learned))
            .collect()
    }

    /// Resolve a menu and group it into categories, **in the order the wire
    /// asked for**.
    ///
    /// `<menu cat_list="1 2 3 4 …">` is the game stating how to present the
    /// categories, so it leads. Categories the items use but `cat_list` did
    /// not name follow, in first-seen order, rather than being dropped --
    /// the dictionary and the wire disagree about categories often enough
    /// that its list cannot be treated as exhaustive.
    ///
    /// Items with no category at all -- an unknown coordinate the wire did
    /// not place either -- come last under `None`.
    #[must_use]
    pub fn resolve_grouped(
        &self,
        menu: &Menu,
        exist: &str,
        secondary: Option<&str>,
        learned: Option<&LearnedCommands>,
    ) -> Vec<(Option<String>, Vec<ResolvedItem>)> {
        let mut groups: BTreeMap<Option<String>, Vec<ResolvedItem>> = BTreeMap::new();
        let mut seen: Vec<Option<String>> = Vec::new();
        for item in self.resolve(menu, exist, secondary, learned) {
            let key = item.category.clone();
            if !groups.contains_key(&key) {
                seen.push(key.clone());
            }
            groups.entry(key).or_default().push(item);
        }
        let mut order: Vec<Option<String>> = menu
            .categories
            .iter()
            .map(|c| Some(c.clone()))
            .filter(|k| groups.contains_key(k))
            .collect();
        for key in seen {
            if !order.contains(&key) {
                order.push(key);
            }
        }
        order
            .into_iter()
            .filter_map(|k| groups.remove(&k).map(|v| (k, v)))
            .collect()
    }
}

/// Fill a template's placeholders. See [`MenuCommands::command_for`].
fn substitute(template: &str, noun: &str, exist: &str, secondary: Option<&str>) -> String {
    let filled = template
        .replace('@', noun)
        .replace('#', &format!("#{exist}"));
    let filled = match secondary {
        Some(value) => filled.replace('%', value),
        None => filled,
    };
    // An empty substitution leaves the template's separating space behind:
    // `"sheathe @"` becomes `"sheathe "`, and the two entries where `@` sits
    // mid-command -- `"convert set @ confirm"` (`2524,1834`) and
    // `"tell @ %"` (`2524,2161`) -- become a doubled space. Collapsing runs
    // of whitespace keeps the sent command well formed either way.
    filled.split_whitespace().collect::<Vec<_>>().join(" ")
}
