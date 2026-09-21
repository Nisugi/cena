//! What is in each hand.
//!
//! # The wire says more than a name, and this used to drop it
//!
//! `<left>` and `<right>` carry the same `exist=`/`noun=` an object link
//! does:
//!
//! ```text
//! <left exist="157925365" noun="bow">glowbark long bow</left>
//! <right>Empty</right>
//! ```
//!
//! [`Frame::LeftHand`](cena_protocol::frame::Frame::LeftHand) preserves that
//! as a `link`, and `GameState::apply` matched `{ item, .. }` -- keeping the
//! display text and discarding the id. Rule 2.2a: the model dropping what the
//! parser preserved.
//!
//! It matters because **an id is what a command targets and a name is not**.
//! Two `glowbark long bow`s are different items; `get #157925365` is
//! unambiguous where `get bow` is a guess the game resolves its own way. Item
//! resolution ([`super::resolve`]) asks "am I already holding this?", and with
//! only a name it cannot answer for the case that matters -- two items whose
//! names match.
//!
//! # `Empty` is a name the game sends, not an item
//!
//! MEASURED over the 208 live Lich XML logs: **2,806 `<right>Empty` and 2,048
//! `<left>Empty`.** The tag is always present, so "the hand is empty" arrives
//! as the literal word `Empty` with no `exist=` -- which is why emptiness is
//! [`Hand::Empty`] rather than `Option::None`. `None` is reserved for its
//! §5.2 meaning: the game has not said.

use cena_protocol::frame::{Link, LinkKind};

/// What one hand holds.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub enum Hand {
    /// The game has not said what is in this hand.
    ///
    /// **Not the same as empty** (`plan/12` §5.2). Before the first
    /// `<left>`/`<right>` of a session, and after a reconnect invalidates,
    /// the honest answer is that nobody has said.
    #[default]
    Unknown,
    /// The game said `Empty`.
    Empty,
    /// The game named an item.
    Holding {
        /// `exist=`, when the wire carried one. **What a command targets.**
        ///
        /// `Option` because the wire is not obliged to send it: a hand's
        /// contents can arrive as bare text, and an item without an id is
        /// still an item worth knowing about.
        id: Option<String>,
        /// `noun=`, when the wire carried one.
        noun: Option<String>,
        /// The display text, e.g. `"glowbark long bow"`.
        name: String,
    },
}

impl Hand {
    /// Read one `<left>` / `<right>` frame.
    #[must_use]
    pub fn read(item: &str, link: Option<&Link>) -> Self {
        // `Empty` is the game's word, and it comes with no `exist=`. Testing
        // the link first would be wrong in the other direction: an item
        // genuinely called "Empty" would carry one.
        if item == "Empty" && link.is_none() {
            return Self::Empty;
        }
        let (id, noun) = match link.map(|l| &l.kind) {
            Some(LinkKind::Exist { id, noun }) => (Some(id.clone()), Some(noun.clone())),
            _ => (None, None),
        };
        Self::Holding {
            id,
            noun,
            name: item.to_owned(),
        }
    }

    /// The exist id, when the hand holds something the game identified.
    #[must_use]
    pub fn id(&self) -> Option<&str> {
        match self {
            Self::Holding { id, .. } => id.as_deref(),
            _ => None,
        }
    }

    /// The display name, when the hand holds something.
    #[must_use]
    pub fn name(&self) -> Option<&str> {
        match self {
            Self::Holding { name, .. } => Some(name),
            _ => None,
        }
    }

    /// The noun a command would target it by, when the game sent one.
    #[must_use]
    pub fn noun(&self) -> Option<&str> {
        match self {
            Self::Holding { noun, .. } => noun.as_deref(),
            _ => None,
        }
    }

    /// Whether the game has said this hand is empty.
    ///
    /// **False when nothing has been said.** A caller wanting "not known to
    /// hold anything" wants `!self.is_holding()`.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        matches!(self, Self::Empty)
    }

    /// Whether the game has named something in this hand.
    #[must_use]
    pub fn is_holding(&self) -> bool {
        matches!(self, Self::Holding { .. })
    }

    /// Whether the game has said anything at all about this hand.
    ///
    /// The question a reconnect test asks: `Empty` and `Holding` are both
    /// answers, and only [`Self::Unknown`] is silence. This replaced
    /// `Option::is_some` when hands stopped being `Option<String>`, and the
    /// compiler made every call site say which of the two it meant -- all
    /// six meant this one.
    #[must_use]
    pub fn is_known(&self) -> bool {
        !matches!(self, Self::Unknown)
    }

    /// Whether this hand holds the item with this exist id.
    #[must_use]
    pub fn holds(&self, id: &str) -> bool {
        self.id() == Some(id)
    }
}
