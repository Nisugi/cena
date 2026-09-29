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
    /// What the form offers: every event a trigger may name.
    pub events: Vec<String>,
    /// Every guard word, with what it takes: for a condition and *only if*.
    pub guard_words: Vec<String>,
    /// The sound files in the sounds folder, by name.
    pub sounds: Vec<String>,
    /// Where the sounds folder is.
    pub sounds_folder: String,
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
    /// Everything the form edits, as the file holds it.
    pub form: Form,
    /// The characters it is switched off for (`for.<name>.enabled = false`).
    pub off_for: Vec<String>,
    /// The characters whose copy changes more than on or off: kept in the
    /// file and `;trigger`, shown here (`plan/54` §1 row 4).
    pub changed_for: Vec<String>,
}

/// A trigger as the editor's form holds it: every field of the file's
/// trigger table that the form edits (`plan/54` step 2). What the form does
/// not edit (one character's copy, where it came from) the file keeps.
#[expect(
    clippy::struct_excessive_bools,
    reason = "each is one of the file's own switches on a trigger, edited as a box"
)]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Form {
    /// Its name, unique in the file.
    pub name: String,
    /// Its group; empty for none.
    pub category: String,
    /// On for everyone.
    pub enabled: bool,
    /// The words it looks for, or its regular expression; empty for none.
    pub text: String,
    /// `text` is a regular expression.
    pub regex: bool,
    /// Case matters. Off unless set (`plan/45` §5d).
    pub case_sensitive: bool,
    /// A literal matches whole words only. On unless cleared (§5d).
    pub whole_word: bool,
    /// The one stream it looks at; empty for all.
    pub stream: String,
    /// What the model must read the line as; empty for none.
    pub event: String,
    /// Guard words that fire it when they all become true; empty for none.
    pub condition: String,
    /// Seconds a condition stays false before it fires again; `None`, the
    /// default.
    pub rearm: Option<u32>,
    /// Guard words that must all hold when it would fire.
    pub only_if: String,
    /// How what it matched looks.
    pub look: Option<Look>,
    /// Hide the line.
    pub squelch: bool,
    /// Put this in place of the match.
    pub substitute: Option<String>,
    /// Show the line on another stream.
    pub redirect: Option<Redirect>,
    /// Set or clear a flag.
    pub flag: Option<Flag>,
    /// Play this: a file in the sounds folder, or a path.
    pub sound: Option<String>,
    /// Say this as an OS notification; empty says the line.
    pub notify: Option<String>,
    /// Show this as a banner; empty shows the line.
    pub alert: Option<String>,
    /// Send this line as if typed.
    pub send: Option<String>,
    /// Seconds before it sounds or sends again; `None`, the default.
    pub cooldown: Option<u32>,
    /// Higher goes first when looks overlap.
    pub priority: i32,
    /// Only these characters; empty for everyone.
    pub characters: Vec<String>,
}

impl Default for Form {
    /// A new trigger: on, for everyone, whole words.
    fn default() -> Self {
        Self {
            name: String::new(),
            category: String::new(),
            enabled: true,
            text: String::new(),
            regex: false,
            case_sensitive: false,
            whole_word: true,
            stream: String::new(),
            event: String::new(),
            condition: String::new(),
            rearm: None,
            only_if: String::new(),
            look: None,
            squelch: false,
            substitute: None,
            redirect: None,
            flag: None,
            sound: None,
            notify: None,
            alert: None,
            send: None,
            cooldown: None,
            priority: 0,
            characters: Vec::new(),
        }
    }
}

/// A look, as the form holds it.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Look {
    /// Text colour, `#rrggbb`; empty for none.
    pub color: String,
    /// Background, `#rrggbb`; empty for none.
    pub background: String,
    /// Bold.
    pub bold: bool,
    /// What it covers: `match`, `line`, or a group's number.
    pub span: String,
}

/// A redirect, as the form holds it.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Redirect {
    /// The stream it goes to.
    pub stream: String,
    /// Shown on both.
    pub copy: bool,
}

/// A flag, as the form holds it.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Flag {
    /// Its name.
    pub name: String,
    /// How long it holds; `None`, until cleared.
    pub seconds: Option<u32>,
    /// Clear it rather than set it.
    pub clear: bool,
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
    /// Save the form: a new trigger when `was` is `None`, else the trigger
    /// named `was`, renamed if the form's name differs. A send the form
    /// changed is approved (`plan/54` §1 row 2).
    Save {
        /// The trigger's name before, or `None` for a new one.
        was: Option<String>,
        /// What it is now.
        form: Box<Form>,
    },
    /// Switch a trigger off, or back on, for one character.
    OffFor {
        /// The trigger.
        name: String,
        /// The character.
        character: String,
        /// Off for them, or back as everyone has it.
        off: bool,
    },
}
