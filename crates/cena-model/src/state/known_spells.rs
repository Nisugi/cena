//! The spells this character knows: the `Spells` stream, typed.
//!
//! # What the wire sends
//!
//! Once per login, after `<clearStream id="Spells"/>`, a list in the only
//! paired stream form on the wire (`stream_routing.rs`: 353 occurrences, all
//! `Spells`). MEASURED 2026-09-21 on a live log
//! (`GSIV-Nisugi/2026/09/xml/2026-09-06_13-23-16.xml:66`):
//!
//! ```text
//! <stream id="Spells"><a exist="-10966483" coord="2524,1885" noun="">Minor Spiritual</a></stream>
//! <stream id="Spells"> </stream>
//! <stream id="Spells">Minor Spiritual:</stream>
//! <stream id="Spells">  <a exist="-10966483" coord="2524,1865" noun="101">Spirit Warding I</a></stream>
//! ```
//!
//! Three kinds of row: a **circle link** (`noun=""`, a menu entry), a **circle
//! header** (`Name:`, no link), and a **spell** -- a link whose `noun` is the
//! spell NUMBER. That last fact is why this is worth typing: the number is on
//! the wire, so nothing here looks a name up in a table to find it.
//!
//! # This is what the game lists, not what Lich derives
//!
//! Lich answers `Spell.known?` from circle RANKS in `skills` -- 30 ranks of
//! Minor Spiritual means 101 through 130. The stream is the game's own list,
//! and the two can differ: the same capture lists 215, 506 and 515 for a
//! ranger with no Major Spiritual or Major Elemental training to speak of.
//! So this records what the `Spells` window shows and claims nothing about
//! WHY a spell is on it. Whether a listed spell is castable right now is a
//! different question (mana, cooldowns, `Effects`).
//!
//! # Parse first, and a classifier plus a consumer (`plan/12` section 3a)
//!
//! [`classify`] is stateless over one line's runs. The circle a spell belongs
//! to needs memory across lines -- the header comes first -- so
//! [`KnownSpells`] is the stateful consumer above it. Neither touches markup:
//! the parser already put `exist`, `noun` and `coord` on the link.
//!
//! # Not persisted, and cleared on reconnect
//!
//! The login burst re-sends the list every time, so a stored copy could only
//! ever be staler than the wire. `None` from [`KnownSpells::knows`] means
//! "the game has not sent the list", which a reconnect makes true again.

use std::collections::BTreeMap;

use cena_protocol::frame::LinkKind;
use cena_protocol::runs::Runs;

/// The stream id the list arrives on. Capitalised on the wire, unlike every
/// pushed stream.
pub const STREAM: &str = "Spells";

/// One row of the `Spells` stream.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Row {
    /// `Minor Spiritual:` -- the spells after it belong to this circle.
    Circle(String),
    /// A spell, by the number the wire carries in the link's `noun`.
    Spell { number: u32, name: String },
}

/// What one completed line of the `Spells` stream says, if anything.
///
/// A circle LINK (`noun=""`) is `None`: it is a menu entry for the window,
/// and the header row that follows says the same thing in the place that
/// matters for grouping.
#[must_use]
pub fn classify(line: &Runs) -> Option<Row> {
    for run in &line.runs {
        if let Some(link) = &run.link
            && let LinkKind::Exist { noun, .. } = &link.kind
        {
            let number = noun.parse().ok()?;
            return Some(Row::Spell {
                number,
                name: link.text.clone(),
            });
        }
    }
    let text = line.plain();
    let circle = text.trim().strip_suffix(':')?;
    (!circle.is_empty()).then(|| Row::Circle(circle.to_owned()))
}

/// One spell the game lists for this character.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct KnownSpell {
    pub name: String,
    /// The circle header it was listed under. `None` if a spell arrived
    /// before any header, which the wire has not been seen to do.
    pub circle: Option<String>,
}

/// The character's spell list, as the `Spells` window shows it.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct KnownSpells {
    /// By spell number, so iteration is in the game's own order.
    spells: BTreeMap<u32, KnownSpell>,
    /// The header most recently read.
    circle: Option<String>,
    /// Whether the game has sent the list at all.
    stated: bool,
}

impl KnownSpells {
    /// `<clearStream id="Spells"/>`: the list is about to be sent whole.
    ///
    /// **This is what makes the list STATED**, not the first spell row: a
    /// character who knows no spells gets the clear and nothing after it, and
    /// "knows none" must not read as "not told yet".
    pub fn begin(&mut self) {
        self.spells.clear();
        self.circle = None;
        self.stated = true;
    }

    /// One completed line of the stream.
    pub fn read_line(&mut self, line: &Runs) {
        match classify(line) {
            Some(Row::Circle(circle)) => self.circle = Some(circle),
            Some(Row::Spell { number, name }) => {
                let circle = self.circle.clone();
                self.spells.insert(number, KnownSpell { name, circle });
            }
            None => {}
        }
    }

    /// Forget the list: a reconnect, after which the login burst re-sends it.
    pub fn clear(&mut self) {
        *self = Self::default();
    }

    /// Whether the game lists this spell. `None`: the list has not been sent.
    #[must_use]
    pub fn knows(&self, number: u32) -> Option<bool> {
        self.stated.then(|| self.spells.contains_key(&number))
    }

    /// One listed spell.
    #[must_use]
    pub fn get(&self, number: u32) -> Option<&KnownSpell> {
        self.spells.get(&number)
    }

    /// Every listed spell, in number order.
    pub fn iter(&self) -> impl Iterator<Item = (u32, &KnownSpell)> {
        self.spells.iter().map(|(n, s)| (*n, s))
    }

    /// How many are listed.
    #[must_use]
    pub fn len(&self) -> usize {
        self.spells.len()
    }

    /// Whether none are listed -- which, unless [`Self::is_stated`], only
    /// means none have been heard of.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.spells.is_empty()
    }

    /// Whether the game has sent the list this connection.
    #[must_use]
    pub const fn is_stated(&self) -> bool {
        self.stated
    }
}
