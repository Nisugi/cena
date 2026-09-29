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
    /// Where its value came from, when there is more than one place it
    /// can: a hunt setting's level -- `built in`, `global`, `the profile`,
    /// `the character's file` (`plan/50` §7 step 6). Shown in place of
    /// *default*.
    pub from: Option<String>,
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
    /// One of these, each its value and what a player calls it: which way
    /// a bar fills.
    Choice(Vec<(String, String)>),
    /// A colour, its value written `#rrggbb`.
    Color,
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

/// Bytes as a person reads them: `980 KB`, `56.8 MB`, `1.2 GB`. What the
/// *Player log* page, `;history` and the log window say a log takes.
#[must_use]
pub fn size(bytes: u64) -> String {
    const KB: u64 = 1024;
    const MB: u64 = KB * 1024;
    const GB: u64 = MB * 1024;
    // Tenths by integer arithmetic, so no float cast is needed.
    let tenths = |unit: u64| {
        let t = (bytes.saturating_mul(10) + unit / 2) / unit;
        format!("{}.{}", t / 10, t % 10)
    };
    if bytes >= GB {
        format!("{} GB", tenths(GB))
    } else if bytes >= MB {
        format!("{} MB", tenths(MB))
    } else {
        format!("{} KB", bytes.div_ceil(KB))
    }
}

#[cfg(test)]
mod tests {
    use super::size;

    #[test]
    fn sizes_read_as_a_person_reads_them() {
        assert_eq!(size(0), "0 KB");
        assert_eq!(size(1), "1 KB");
        assert_eq!(size(980 * 1024), "980 KB");
        assert_eq!(size(66_400 * 1024), "64.8 MB");
        assert_eq!(size(3 * 1024 * 1024 * 1024 / 2), "1.5 GB");
    }
}
