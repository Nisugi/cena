//! The `charprofile` stream: what `profile` shows, typed.
//!
//! # What the wire sends
//!
//! MEASURED 2026-09-21 against a live capture
//! (`fixtures/character_profile.xml`, cut from
//! `GSIV-Nisugi/2026/09/xml/2026-09-06_13-23-16.xml:18954`). A window
//! declaration, `<output class="mono"/>`, then four **bold section headers**
//! with plain lines under each:
//!
//! ```text
//! PERSONAL INFORMATION
//! Name: Nisugi
//! Profession: Ranger   Level: 100
//! Title: Hero
//! Race: Half-Elf   Gender: Male
//! Age: a tender age (36)
//! Date of Birth: 8/28/5090
//! He is taller than average.  He appears to be of a tender age.
//! AFFILIATIONS
//! Full citizen of Kraken's Fall
//! Master of the Guardians of Sunfist
//! ACHIEVEMENTS
//! Strongest foe vanquished: a leopard ogre (level 140)
//! HISTORY
//! Formerly known as Ibbo.
//! ```
//!
//! # What this type holds, and what it deliberately does not
//!
//! Most of `PERSONAL INFORMATION` and all of `AFFILIATIONS` are **already
//! modelled**, and this reader must not become a second home for them:
//!
//! | Profile line | Already read by |
//! |---|---|
//! | `Race:`, `Profession:`, `Gender:` | [`Identity`](super::stats::Identity), from `info` |
//! | `Master of the Guardians of Sunfist` | [`Standing::apply_society`](super::standing::Standing) |
//! | `Full citizen of Kraken's Fall` | `Standing`'s citizenship |
//!
//! Rule 2.2a: those keep one home. What is here is what nothing else has --
//! **`Title:`, the character's own description, the achievements and the
//! history** -- plus the two identity fields the profile states better than
//! `info` does (`Age:` carries a number in brackets; `Date of Birth:` appears
//! nowhere else).
//!
//! # Nothing here is the level
//!
//! `Level: 100` is on this line and is NOT read, for the reason
//! `blocks.rs:155` already gives about `info`: Lich discards it and says
//! *"level captured here, but do not rely on it - use XML"*. The `expr` dialog
//! is the level's home.
//!
//! # It arrives in the MAIN window, not in a stream
//!
//! > **CORRECTED 2026-09-21, by the fixture, before anything shipped.** This
//! > was first built to read a `charprofile` stream buffer, on the strength of
//! > `stream_routing.rs`'s census listing `charprofile` among the six pushed
//! > stream ids. **The capture says otherwise**: `profile` sends
//! > `streamWindow` + `clearStream` + `exposeStream` and then prints the text
//! > into the MAIN window. MEASURED over the two profile runs in
//! > `E:\Gemstone\dev\lich-5\logs\GSIV-Nisugi`: `exposeStream
//! > id="charprofile"` twice, `pushStream id="charprofile"` **zero times**.
//! >
//! > So this is an ordinary chunk reader, beside `blocks::InfoReport`, and the
//! > stream-buffer version read an empty buffer and taught nothing. Every test
//! > here failed until it was fixed -- which is the argument for cutting the
//! > fixture from live traffic before writing the reader, not after.
//! >
//! > `exposeStream` is a **display** instruction: show that window. The
//! > wiki's `ifClosed=''` "Copy" behaviour is the same shape -- the server
//! > sends the line to main as well -- and `plan/15` should record that a
//! > declared-and-exposed window is not evidence of a pushed stream.
//!
//! # A classifier over a chunk, per `plan/12` section 3a
//!
//! The section headers arrive bold, and the parser already records bold depth,
//! so nothing here re-tokenizes markup. Read whole from one chunk because the
//! sections are positional: `Formerly known as` means something different
//! under `HISTORY` than it would anywhere else -- and because the prompt is
//! the only boundary a command's output has.

use super::{snapshot, standing};

/// The window `profile` declares and exposes.
///
/// **Not a stream to read text from** -- see the module docs. Kept because a
/// frontend showing the profile in its own panel needs the id, and because a
/// reader looking for `charprofile` should land on that correction.
pub const WINDOW: &str = "charprofile";

/// A section of the profile, as its bold header names it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Section {
    Personal,
    Affiliations,
    Achievements,
    History,
}

impl Section {
    fn of(line: &str) -> Option<Self> {
        match line.trim() {
            "PERSONAL INFORMATION" => Some(Self::Personal),
            "AFFILIATIONS" => Some(Self::Affiliations),
            "ACHIEVEMENTS" => Some(Self::Achievements),
            "HISTORY" => Some(Self::History),
            _ => None,
        }
    }
}

/// What `profile` says that nothing else does.
///
/// Every field is `Option` or empty-by-default: a profile nobody has run is
/// not a character with no title (`plan/12` section 5.2).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Profile {
    /// `Title: Hero` -- the title in use, which `info` does not state.
    pub title: Option<String>,
    /// `Age: a tender age (36)` -- the number in the brackets.
    ///
    /// Also reaches [`Identity::age`](super::stats::Identity::age), which is
    /// where a caller should read it. Kept here too because this reader is
    /// where the bracketed form is understood.
    pub age: Option<u32>,
    /// `Date of Birth: 8/28/5090`, verbatim. Elanthian, so not a date type.
    pub birthday: Option<String>,
    /// The `He is...` / `He has...` lines, in wire order: height, eyes, skin,
    /// hair, scars, tattoos. Free prose, and the set varies per character, so
    /// these are kept as lines rather than parsed into fields.
    pub description: Vec<String>,
    /// The `AFFILIATIONS` lines, in wire order.
    ///
    /// **Kept whole even though `Standing` reads most of them**, because the
    /// section says things `Standing` has no field for: `Follower of Zelia`,
    /// `Attuned to the Element of Earth`, `Member of House of Paupers`. A
    /// caller wanting the society or citizenship reads `Standing`.
    pub affiliations: Vec<String>,
    /// `ACHIEVEMENTS`, as `(label, value)`: *Strongest foe vanquished*,
    /// *Most difficult lock picked*, *Number of warcamps destroyed*. Labels
    /// vary by profession, so this is a list rather than a struct.
    pub achievements: Vec<(String, String)>,
    /// `HISTORY` lines, e.g. `Formerly known as Ibbo.`
    pub history: Vec<String>,
    /// Whether a profile has been read at all.
    stated: bool,
}

impl Profile {
    /// Whether `profile` has been seen this session.
    #[must_use]
    pub const fn is_stated(&self) -> bool {
        self.stated
    }

    /// Read one chunk's lines.
    ///
    /// Returns whether they were a profile. Recognised by its `PERSONAL
    /// INFORMATION` header rather than assumed, because this runs on every
    /// chunk -- the same rule `blocks::InfoReport::read` follows.
    pub fn read_lines(&mut self, lines: &[String]) -> bool {
        if !lines
            .iter()
            .any(|l| Section::of(l) == Some(Section::Personal))
        {
            return false;
        }
        // Whole-list replacement, like the `Spells` stream: `profile` prints
        // everything every time, so merging would keep an achievement that a
        // later run no longer lists.
        *self = Self {
            stated: true,
            ..Self::default()
        };
        let mut section = None;
        for line in lines {
            if let Some(next) = Section::of(line) {
                section = Some(next);
                continue;
            }
            let text = line.trim();
            if text.is_empty() {
                continue;
            }
            match section {
                Some(Section::Personal) => self.read_personal(text),
                Some(Section::Affiliations) => self.affiliations.push(text.to_owned()),
                Some(Section::Achievements) => {
                    if let Some((label, value)) = text.split_once(": ") {
                        self.achievements
                            .push((label.trim().to_owned(), value.trim().to_owned()));
                    }
                }
                Some(Section::History) => self.history.push(text.to_owned()),
                None => {}
            }
        }
        true
    }

    fn read_personal(&mut self, text: &str) {
        // `Name:`, `Profession:`, `Race:`, `Gender:` are deliberately skipped:
        // `Character` and `Identity` own those (see the module docs).
        if let Some(title) = text.strip_prefix("Title: ") {
            self.title = Some(title.trim().to_owned());
        } else if let Some(age) = text.strip_prefix("Age: ") {
            // `a tender age (36)`: the brackets hold the number, and the words
            // before them are flavour that changes with it.
            self.age = age
                .rsplit_once('(')
                .and_then(|(_, n)| n.trim_end_matches(')').trim().parse().ok());
        } else if let Some(birthday) = text.strip_prefix("Date of Birth: ") {
            self.birthday = Some(birthday.trim().to_owned());
        } else if text.contains(" is ") || text.contains(" has ") {
            // The description lines. Recognised by their shape rather than by
            // a pronoun, because the pronoun is the character's.
            self.description.push(text.to_owned());
        }
    }

    /// One achievement by label, e.g. `"Number of warcamps destroyed"`.
    #[must_use]
    pub fn achievement(&self, label: &str) -> Option<&str> {
        self.achievements
            .iter()
            .find(|(l, _)| l == label)
            .map(|(_, v)| v.as_str())
    }

    /// Forget it: a reconnect, after which `profile` must be run again.
    pub fn clear(&mut self) {
        *self = Self::default();
    }
}

impl super::Character {
    /// Send what the profile said to the fields that own it.
    ///
    /// **Rule 2.2a: one home per fact.** `profile` states the age, the society
    /// and the citizenship, and `Identity` and `Standing` already hold those
    /// from `info` and from single lines. This routes them there instead of
    /// letting `Profile` become a second, quietly disagreeing copy -- and it
    /// reuses `standing`'s own line readers, so `Master of the Guardians of
    /// Sunfist` yields rank 20 here exactly as it does from a chunk.
    pub(crate) fn absorb_profile(&mut self) {
        if let Some(age) = self.profile.age {
            self.identity.age = Some(age);
            self.taught.insert(snapshot::Group::Identity);
        }
        let mut standing_changed = false;
        // `profile`'s own wording, which the report readers do not accept --
        // see `standing::profile_affiliation`.
        for line in self.profile.affiliations.clone() {
            match standing::profile_affiliation(&line) {
                Some(standing::Affiliation::Society(event)) => {
                    standing_changed |= self.standing.apply_society(event);
                }
                Some(standing::Affiliation::Citizenship(town)) => {
                    let town = Some(Some(town));
                    standing_changed |= self.standing.citizenship != town;
                    self.standing.citizenship = town;
                }
                None => {}
            }
        }
        if standing_changed {
            self.taught.insert(snapshot::Group::Standing);
        }
    }
}
