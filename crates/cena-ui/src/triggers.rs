//! The trigger editor's view of the one triggers file (`plan/54`), as a
//! frontend draws it.
//!
//! The file is read and written by the binary, through the writer `;trigger`
//! uses (`cena_behavior::triggers::edit`), so the editor and the command
//! cannot disagree. What crosses to the window is this: the list, the
//! switches, and the changes it may ask for. As the settings menu's pages
//! do ([`crate::settings`]).

/// Every trigger in the file, and its switches.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Book {
    /// Each trigger, sorted by category then name, as the file is.
    pub triggers: Vec<Entry>,
    /// Every category a trigger has or a switch names, each on or off.
    pub categories: Vec<(String, bool)>,
    /// Each kind of response, on or off everywhere.
    pub kinds: Vec<(String, bool)>,
    /// Where the file is, as the player would look for it.
    pub file: String,
    /// Why nothing can be changed: the file is there and does not read.
    pub problem: Option<String>,
}

/// One trigger in the list.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Entry {
    /// Its name: unique, and what a change addresses.
    pub name: String,
    /// Its category; empty when it has none.
    pub category: String,
    /// On for everyone (before any one character's copy).
    pub enabled: bool,
    /// Why it cannot be used, when it cannot.
    pub refused: Option<String>,
    /// What it watches and does, in a line.
    pub summary: String,
    /// Its send, waiting for the player's approval.
    pub held: Option<String>,
    /// Where it came from, when an import brought it.
    pub origin: Option<String>,
}

/// What a switch turns on or off.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Switch {
    /// One trigger, by name.
    Trigger(String),
    /// Every trigger in a category.
    Category(String),
    /// One kind of response, everywhere.
    Every(String),
}

/// A change the editor asks of the file.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Change {
    /// Switch something on or off.
    Switch(Switch, bool),
    /// Let a trigger from elsewhere send its line.
    Approve(String),
    /// Take a trigger out of the file.
    Remove(String),
}
