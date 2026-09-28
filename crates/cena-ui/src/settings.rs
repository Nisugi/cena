//! The settings menu's pages, as a frontend draws them (`plan/50` §7 step 1).
//!
//! The one menu (the author, 2026-09-27: *"one main settings button to get to
//! the main settings menu"*) draws pages it is given; it knows no behavior.
//! The binary builds each page from a behavior's table of keys and its file,
//! and applies a change through the writer that behavior's `;` command
//! uses. So the command and the menu cannot disagree about a value, and a
//! file has one writer. Here so any frontend can draw the same pages.

/// One page of the menu: one file's settings.
#[derive(Clone, Debug, PartialEq)]
pub struct Page {
    /// What a change names it by.
    pub id: String,
    /// What a player calls it.
    pub title: String,
    /// The file it is kept in, as the player would look for it.
    pub file: String,
    /// When what is changed takes effect: *"the next time Heal runs"*.
    pub takes: String,
    /// Why nothing on the page can be changed: its file is there and does not
    /// read, and is never written over.
    pub problem: Option<String>,
    /// Its settings, in the behavior's order.
    pub rows: Vec<Row>,
}

/// One setting on a page.
#[derive(Clone, Debug, PartialEq)]
pub struct Row {
    /// The key a change names.
    pub key: String,
    /// What a player calls it.
    pub label: String,
    /// A line of what it does.
    pub help: String,
    /// How it is edited.
    pub kind: RowKind,
    /// Its value in effect.
    pub value: Value,
    /// Whether this page's file sets it, rather than its default.
    pub here: bool,
}

/// How a setting is edited.
#[derive(Clone, Debug, PartialEq)]
pub enum RowKind {
    /// On or off.
    Toggle,
    /// A whole number, from `min` to `max`.
    Whole {
        /// The least.
        min: u32,
        /// The most.
        max: u32,
    },
    /// A number, from `min` to `max`.
    Number {
        /// The least.
        min: f64,
        /// The most.
        max: f64,
    },
    /// Words.
    Text,
    /// A list of whole numbers.
    Numbers,
    /// A list of words.
    Words,
    /// Names each given a value: shown, and changed with the behavior's own
    /// command, which the row's help names.
    Map,
}

/// A setting's value, as written: numbers are text, so nothing is lost to a
/// conversion.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Value {
    /// Unset, with no default.
    Unset,
    /// On or off.
    On(bool),
    /// A number or words.
    Text(String),
    /// A list.
    List(Vec<String>),
    /// Names each given a value.
    Map(Vec<(String, String)>),
}

/// One setting changed from the menu.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Change {
    /// The character, as the roster names it: `GAME:Name`.
    pub character: String,
    /// The page's [`Page::id`].
    pub page: String,
    /// The row's [`Row::key`].
    pub key: String,
    /// The value, written as it would be typed after `;heal set <key>`;
    /// `None` puts it back to its default.
    pub to: Option<String>,
}
