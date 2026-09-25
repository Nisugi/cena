//! Which items a `;foreach` comes to: read off the model, filtered, and
//! ordered as foreach.lic's `ItemMatcher` orders them
//! (`reference/scripts/scripts/foreach.lic:175-678`).
//!
//! Pure. What a target holds is what the game listed for it -- the
//! `<container>` and `<inv>` feed a `look in` brings, which is where Lich's
//! `GameObj.containers` comes from too (`reference/lich-5/lib/common/xmlparser.rb:516-521`,
//! `:1262-1268`) -- or the room's own list of what is on the ground.

use std::collections::{BTreeSet, HashSet};

use cena_session::{ChunkLine, GameState, LinkKind, RoomItem};

use super::foreach::{Attr, Filter, Options, Sort};

/// Where a group of items was found.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Place {
    /// Inside a container: its `exist` id, and its name as the game said it.
    Container {
        /// The container's `exist` id, which `container` in a command means.
        id: String,
        /// Its name, as the look named it: `dwarf skin backpack`.
        name: String,
    },
    /// On the ground.
    Ground,
}

impl Place {
    /// The header a listing gives it (`:2133`).
    #[must_use]
    pub fn label(&self) -> &str {
        match self {
            Self::Container { name, .. } => name,
            Self::Ground => "On the ground",
        }
    }
}

/// One item that may be picked, with what a filter reads.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Candidate {
    /// `exist=`: what `item` in a command means.
    pub id: String,
    /// `noun=`.
    pub noun: String,
    /// The link's text: `tumbler of black cherry whiskey`.
    pub name: String,
    /// With the words either side, as `GameObj#full_name` joins them
    /// (`reference/lich-5/lib/common/gameobj.rb:227`): `a tumbler of black
    /// cherry whiskey`.
    pub full_name: String,
    /// Its `gameobj` types.
    pub types: BTreeSet<String>,
    /// The shops that buy it.
    pub sellable: BTreeSet<String>,
}

impl Candidate {
    /// An item the game listed, classified.
    #[must_use]
    pub fn of(item: &RoomItem) -> Candidate {
        let classified = cena_session::gameobj::classify(&item.noun, &item.text);
        let full_name = [
            item.before.as_deref(),
            Some(item.text.as_str()),
            item.after.as_deref(),
        ]
        .into_iter()
        .flatten()
        .filter(|part| !part.is_empty())
        .collect::<Vec<_>>()
        .join(" ");
        Candidate {
            id: item.id.clone(),
            noun: item.noun.clone(),
            name: item.text.clone(),
            full_name,
            types: classified.types,
            sellable: classified.sellable,
        }
    }
}

/// The items found in one place, in order.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Group {
    /// Where.
    pub place: Place,
    /// What, as listed and then as picked.
    pub items: Vec<Candidate>,
}

impl Filter {
    /// Whether an item counts.
    #[must_use]
    pub fn matches(&self, item: &Candidate) -> bool {
        match self {
            Self::All => true,
            Self::Unclassified(Attr::Sellable) => item.sellable.is_empty(),
            Self::Unclassified(_) => item.types.is_empty(),
            Self::Pattern { attr, pattern } => match attr {
                Attr::Type => item.types.iter().any(|t| pattern.is_match(t)),
                Attr::Sellable => item.sellable.iter().any(|s| pattern.is_match(s)),
                Attr::Name => pattern.is_match(&item.name),
                Attr::FullName => pattern.is_match(&item.full_name),
                Attr::Noun => pattern.is_match(&item.noun),
            },
        }
    }
}

/// Keep what the filter takes, each item once across every group (`add_item`,
/// `:229-242`).
#[must_use]
pub fn filtered(filter: &Filter, groups: Vec<Group>) -> Vec<Group> {
    let mut seen = HashSet::new();
    groups
        .into_iter()
        .map(|mut group| {
            group
                .items
                .retain(|item| filter.matches(item) && seen.insert(item.id.clone()));
            group
        })
        .collect()
}

/// `finalize` (`:609-678`): within each group `unique`, then the sort, then
/// `reversed`; then `after` and `first` counted across the groups in order.
///
/// **`unique` before `reversed`, as Lich has it**: of two items with one
/// name, the one listed first is kept. `VellumFE` reverses first and keeps
/// the last (`src/core/foreach.rs:333-346`).
#[must_use]
pub fn select(options: &Options, groups: Vec<Group>) -> Vec<Group> {
    let mut names = HashSet::new();
    let mut skip = options.after.unwrap_or(0);
    let mut first = options.first;
    let mut out = Vec::new();
    for mut group in groups {
        if options.unique {
            group
                .items
                .retain(|item| names.insert(item.full_name.clone()));
        }
        match options.sort {
            Sort::Listed => {}
            Sort::Name => group.items.sort_by_key(|item| sort_name(&item.full_name)),
            Sort::Noun => group.items.sort_by_key(|item| {
                format!("{}{}", item.noun.to_lowercase(), sort_name(&item.full_name))
            }),
        }
        if options.reversed {
            group.items.reverse();
        }
        if skip >= group.items.len() {
            skip -= group.items.len();
            continue;
        }
        group.items.drain(..skip);
        skip = 0;
        if let Some(left) = first {
            if left < group.items.len() {
                group.items.truncate(left);
            }
            first = Some(left.saturating_sub(group.items.len()));
        }
        if !group.items.is_empty() {
            out.push(group);
        }
    }
    out
}

/// A full name for sorting: lower case, without a leading `a`, `an`, `some`
/// or `the` (`:639-642`).
fn sort_name(full_name: &str) -> String {
    let mut name = full_name;
    for article in ["a ", "an ", "some ", "the "] {
        if let Some(rest) = name.strip_prefix(article) {
            name = rest.trim_start();
            break;
        }
    }
    name.to_lowercase()
}

/// What a `look in` answered (`INV_PATTERN`, `:190`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Looked {
    /// `In the <backpack> you see ...`, or the sorted view's `In the
    /// <backpack>:`: the container's `exist` id and its name.
    Container {
        /// The container's `exist` id.
        id: String,
        /// Its name, as the answer's link says it.
        name: String,
    },
    /// `There is nothing in the ...`.
    Empty,
    /// `That is closed.`
    Closed,
    /// `I could not find what you were referring to.`
    NotFound,
    /// None of those: the game said something else.
    Unread,
}

/// Read a `look in` answer: the first line that is one of its shapes.
#[must_use]
pub fn looked(lines: &[ChunkLine]) -> Looked {
    for line in lines {
        let text = line.text();
        let text = text.trim();
        if ["in", "on", "under", "behind"]
            .iter()
            .any(|p| text.contains(&format!("There is nothing {p} the")))
        {
            return Looked::Empty;
        }
        if text.contains("That is closed.") {
            return Looked::Closed;
        }
        if text.contains("I could not find what you were referring to.") {
            return Looked::NotFound;
        }
        if let Some(found) = container_of(line, text) {
            return found;
        }
    }
    Looked::Unread
}

/// The container a line names as the one looked in, when it is either of
/// `INV_PATTERN`'s two shapes: the link just before `you see`, after
/// `Peering into`, `In`, `On`, `Under` or `Behind`; or the whole line
/// `In the <link>:`.
fn container_of(line: &ChunkLine, text: &str) -> Option<Looked> {
    let opens = ["Peering into ", "In ", "On ", "Under ", "Behind "]
        .iter()
        .any(|word| text.starts_with(word));
    if !opens {
        return None;
    }
    let exist = |run: &cena_session::Run| {
        run.link
            .iter()
            .chain(run.inner_link.iter())
            .find_map(|link| match &link.kind {
                LinkKind::Exist { id, .. } => Some((id.clone(), link.text.clone())),
                _ => None,
            })
    };
    let runs = &line.runs.runs;
    for pair in runs.windows(2) {
        let (Some((id, name)), next) = (exist(&pair[0]), &pair[1]) else {
            continue;
        };
        let after = next.text.as_str();
        if next.link.is_none() && (after.starts_with(" you see") || after.starts_with(", you see"))
        {
            return Some(Looked::Container { id, name });
        }
    }
    let sorted_view =
        (text.starts_with("In the ") || text.starts_with("On the ")) && text.ends_with(':');
    if sorted_view {
        let (id, name) = runs.iter().find_map(exist)?;
        return Some(Looked::Container { id, name });
    }
    None
}

/// What the game listed for container `id`, when it has: the `<inv>` feed
/// keyed by the window, which is the container's own id except for `stow`,
/// whose `target` is (`crates/cena-model/src/state/inventory.rs`).
#[must_use]
pub fn contents<'a>(state: &'a GameState, id: &str) -> Option<&'a [RoomItem]> {
    state
        .inventory
        .container(id)
        .or_else(|| {
            state
                .inventory
                .containers()
                .find(|(_, container)| container.target.as_deref() == Some(id))
                .map(|(_, container)| container)
        })
        .map(|container| container.items.as_slice())
}

/// What `;foreach` with no commands says: each place, then each item as
/// `#id full name (types)`, then the count (`item_detail`, `:2152-2160`;
/// `:2132-2149`).
#[must_use]
pub fn listing(groups: &[Group]) -> Vec<String> {
    let mut lines = Vec::new();
    let mut total = 0;
    for group in groups {
        lines.push(format!("[{}]:", group.place.label()));
        for item in &group.items {
            let types = if item.types.is_empty() {
                "no type".to_owned()
            } else {
                item.types.iter().cloned().collect::<Vec<_>>().join(",")
            };
            lines.push(format!("#{} {} ({types})", item.id, item.full_name));
        }
        total += group.items.len();
    }
    lines.push(format!("Total items: {total}"));
    lines
}
