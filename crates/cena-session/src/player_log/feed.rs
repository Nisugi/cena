//! One connection's feed into the player log: what is captured, and what a
//! line is tagged once its chunk is understood.
//!
//! [`Tap`] records a line the moment it is handed one. That is right for a
//! notice and wrong for game text, for a reason that is the author's goal for
//! this whole feature:
//!
//! > **AUTHOR, 2026-09-21:** *"One of my biggest goals for this program is to be
//! > able to separate a combat feed out from the main feed ... with enough
//! > definitions we can have a classifier that classifies a blob as combat if
//! > it contains one of our defs."*
//!
//! # A line's class is not known when the line ends
//!
//! The blob is the model's prompt-to-prompt chunk, and the combat tracker
//! classifies it when the prompt closes it. So a line cannot be tagged as it
//! completes. The feed **holds a chunk's lines and writes them at the prompt**,
//! when the actor can say whether the chunk produced combat facts.
//!
//! Everything is held, not only the main window's lines, so the file stays in
//! the order things happened. Each line keeps the stamp of the moment it
//! arrived; only the write waits.
//!
//! # The tag: `source/class`
//!
//! A line from a combat chunk is tagged `main/combat`. Still one bracket group,
//! so the line format and `parse_line` are unchanged; a viewer splits on `/`.
//! **The class never replaces the source**: `main` is where the game sent it,
//! `combat` is what our definitions made of it, and a reader auditing the
//! definitions needs to tell those apart.
//!
//! A chunk the definitions miss stays `main`. It is still in the log, in
//! order -- coverage can grow without old logs having lied.
//!
//! # Capture is configurable because a feed costs disk
//!
//! > **AUTHOR, 2026-09-21:** *"they may not want all the bloat that comes with
//! > the inventory feed, ect. eats a lot of space."*
//!
//! [`Capture`] decides what is WRITTEN; filtering what is SHOWN is the
//! reader's. Every feed is available. The readouts -- streams the game clears
//! and rewrites whole -- default to off, and when on are written **only when
//! they change**: Lichborne measured 1,173 `spells` and 2,574 `inv` lines in
//! one 11-minute window, nearly all of them the same table again.

use std::collections::{BTreeMap, BTreeSet};

use cena_platform::line_time;

use super::tap::{self, Tap};
use crate::lifecycle::Generation;

/// The class given to every main-window line of a chunk that produced combat
/// facts.
pub const COMBAT: &str = "combat";

/// A chunk is written unclassified once it holds this many lines.
///
/// The login burst and a long `look` in a crowded room run far past anything a
/// combat chunk does, and a connection that never prompts must not hold its
/// log in memory. The model's own chunk caps at 200 lines for the same reason.
pub const MAX_HELD: usize = 512;

/// Which feeds are written to disk.
///
/// A set of what is switched OFF, so that a stream nobody has heard of is
/// captured: a reader can filter noise and cannot fill a hole.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Capture {
    off: BTreeSet<String>,
}

impl Default for Capture {
    /// Text feeds on; the readouts ([`tap::READOUTS`]) off.
    fn default() -> Self {
        Self {
            off: tap::READOUTS.iter().map(|s| (*s).to_owned()).collect(),
        }
    }
}

impl Capture {
    /// Every feed, readouts included.
    #[must_use]
    pub const fn everything() -> Self {
        Self {
            off: BTreeSet::new(),
        }
    }

    /// Switch one feed on or off, by the tag it is logged under (`main`,
    /// `cmd`, `inv`, `thoughts`...).
    #[must_use]
    pub fn set(mut self, feed: &str, on: bool) -> Self {
        if on {
            self.off.remove(feed);
        } else {
            self.off.insert(feed.to_owned());
        }
        self
    }

    /// Whether a feed is written.
    #[must_use]
    pub fn wants(&self, feed: &str) -> bool {
        !self.off.contains(feed)
    }
}

/// The `player_log` section of a character's settings file
/// (`settings_store`): which feeds to write, by tag.
///
/// ```json
/// "player_log": { "feeds": { "inv": true, "thoughts": false } }
/// ```
///
/// **Overrides, not the whole list.** A feed not named keeps its default, so a
/// file written today does not switch off a feed that is added tomorrow.
#[derive(Clone, Debug, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct LogSettings {
    /// Feed tag to on/off, overriding the default for each feed named.
    #[serde(default)]
    pub feeds: BTreeMap<String, bool>,
}

/// The name of [`LogSettings`]' section.
pub const SECTION: &str = "player_log";

impl Capture {
    /// This capture with a settings section's choices laid over it.
    #[must_use]
    pub fn with(self, settings: &LogSettings) -> Self {
        settings
            .feeds
            .iter()
            .fold(self, |capture, (feed, on)| capture.set(feed, *on))
    }
}

/// A line waiting for its chunk to close.
#[derive(Debug)]
struct Held {
    at: String,
    feed: String,
    text: String,
}

/// One connection's feed. Owned by the actor; see the module doc.
#[derive(Debug)]
pub struct Feed {
    tap: Tap,
    generation: Generation,
    held: Vec<Held>,
    /// What each readout last wrote, so an unchanged rewrite is not written.
    written: BTreeMap<String, Vec<String>>,
}

impl Feed {
    /// A feed for connection `generation`, writing through `tap`, with nothing
    /// held and nothing yet written.
    #[must_use]
    pub const fn new(tap: Tap, generation: Generation) -> Self {
        Self {
            tap,
            generation,
            held: Vec::new(),
            written: BTreeMap::new(),
        }
    }

    /// The game has named the character, so their settings can be read.
    ///
    /// # Errors
    ///
    /// [`Tap::choose`]'s.
    pub fn choose(&self, instance: &str, character: &str) -> Result<(), String> {
        self.tap.choose(instance, character)
    }

    /// One completed line of game text, from the stream the wire named.
    pub fn line(&mut self, stream: &str, text: &str) {
        if !text.trim().is_empty() {
            self.hold(tap::tag(stream), text);
        }
    }

    /// One command, as it went to the game.
    pub fn command(&mut self, message: &[u8]) {
        let text = String::from_utf8_lossy(message);
        let text = text.trim_end_matches(['\r', '\n']);
        if !text.is_empty() {
            self.hold(tap::COMMANDS, text);
        }
    }

    fn hold(&mut self, feed: &str, text: &str) {
        self.held.push(Held {
            at: line_time(),
            feed: feed.to_owned(),
            text: text.to_owned(),
        });
        if self.held.len() >= MAX_HELD {
            self.close(false);
        }
    }

    /// The prompt closed the chunk: write what was held.
    ///
    /// `combat` is whether the chunk produced combat facts. Only the main
    /// window's lines take the class -- a thought that arrived mid-swing is
    /// someone else's words, which is the same gate the chunk itself applies.
    pub fn close(&mut self, combat: bool) {
        // **Capture is judged HERE, not when the line arrived.** The settings
        // file is named by what `<app>` says, so it cannot be loaded until the
        // game has spoken -- and the first lines of a login come before that.
        // By the first prompt it has long since loaded.
        let capture = self.tap.capture();
        let held: Vec<Held> = std::mem::take(&mut self.held)
            .into_iter()
            .filter(|line| capture.wants(&line.feed))
            .collect();
        let unchanged = self.unchanged_readouts(&held);
        for line in held {
            if unchanged.contains(&line.feed) {
                continue;
            }
            let tag = if combat && line.feed == tap::MAIN {
                format!("{}/{COMBAT}", line.feed)
            } else {
                line.feed
            };
            self.tap.record_at(self.generation, line.at, tag, line.text);
        }
    }

    /// The readouts in this chunk that say exactly what they said last time.
    ///
    /// A readout arrives whole -- `clearStream`, push, every line, pop -- with
    /// no prompt inside it, so a chunk's lines for one readout ARE its
    /// snapshot. Remembers the new snapshot for the ones that did change.
    fn unchanged_readouts(&mut self, held: &[Held]) -> BTreeSet<String> {
        let mut snapshots: BTreeMap<&str, Vec<String>> = BTreeMap::new();
        for line in held {
            if tap::READOUTS.contains(&line.feed.as_str()) {
                snapshots
                    .entry(&line.feed)
                    .or_default()
                    .push(line.text.clone());
            }
        }
        let mut unchanged = BTreeSet::new();
        for (feed, snapshot) in snapshots {
            if self.written.get(feed) == Some(&snapshot) {
                unchanged.insert(feed.to_owned());
            } else {
                self.written.insert(feed.to_owned(), snapshot);
            }
        }
        unchanged
    }
}

impl Drop for Feed {
    /// A connection that ends mid-chunk still logs what it saw: the last lines
    /// before a drop are the ones a reader goes looking for.
    fn drop(&mut self) {
        self.close(false);
    }
}
