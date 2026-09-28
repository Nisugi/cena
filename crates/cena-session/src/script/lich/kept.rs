//! What a Lich started late is handed first (`plan/51` §7, step 4): the
//! connection's login, then the latest word on each piece of state since.
//!
//! A Lich knows only what it has read (`plan/51` §4, item 6). Started with
//! the character it reads the login; started later, it missed it, and all
//! since. So the session keeps the login as it came and, after it, for each
//! piece of state Lich keeps, the latest chunk to say it: the room and its
//! parts, the hands, the vitals and the other bars, the indicators, the
//! prepared spell, the effects, the timers, the injuries. Handed those in the
//! order they came, then the live stream, a Lich knows where the character
//! is, what it holds and how it is. They are recorded bytes resent, never
//! made-up tags (`plan/46` §10). The list is the one Lich pushes to a
//! frontend attached late, which it builds from its model instead
//! (`reference/lich-5/lib/gemstone/detachable_client_init.rb`: vitals, spell,
//! hands, indicators, stance, mind, encumbrance, injuries, compass), with the
//! room's parts added, since a frontend redraws the room and Lich must know
//! it.
//!
//! # Whole lines
//!
//! A chunk is where a read ended, not where a line did. Resent out of their
//! stream, chunks could splice half a line onto another, so what is kept is a
//! run of chunks to a line's end: those since the parser last held nothing.
//! The run still open is handed last, and the live stream finishes it.
//!
//! # The text in them
//!
//! It is resent too: Lich reads those lines again, as its scripts do, and the
//! session leaves them out of what Lich shows, the character's text having
//! shown them once (`LichText::expect_replay`).

use std::collections::{BTreeMap, HashMap};

use cena_protocol::Frame;

/// The most of a connection's login kept. A login is a few hundred lines;
/// one past this is not handed to a Lich started late, which is told.
pub(crate) const LOGIN_BYTES: usize = 4 * 1024 * 1024;

/// The most one run of chunks to a line's end may hold and be kept. The
/// parser drops a line longer than it can hold, so a run this long is not
/// one Lich would read either.
const RUN_BYTES: usize = 1024 * 1024;

/// A piece of state Lich keeps, as a frame says it.
#[derive(Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub(crate) enum Key {
    /// `<nav>`: which room.
    Room,
    /// `<compass>`: its exits.
    Compass,
    /// `<left>`.
    Left,
    /// `<right>`.
    Right,
    /// `<spell>`: the prepared spell.
    Spell,
    /// `<roundTime>`.
    RoundTime,
    /// `<castTime>`.
    CastTime,
    /// `<streamWindow>`, by id: `room`'s carries the room's title.
    Window(String),
    /// `<component>`, by id: the room's description, objects, players and
    /// exits, and the like.
    Component(String),
    /// `<indicator>`, by id: standing, stunned, hidden, and the like.
    Indicator(String),
    /// A progress bar, label, image or widget, by its dialog and id: the
    /// vitals, stance, mind, encumbrance and injuries among them.
    InDialog(&'static str, Option<String>, String),
    /// An effect, by its category, which is its dialog, and id.
    Effect(String, String),
    /// A dialog emptied: what was said in it before is gone.
    Cleared(String),
    /// A stream begun again, as `inv` is.
    Stream(String),
}

impl Key {
    /// The piece of state `frame` says, if it is one Lich keeps.
    pub(crate) fn of(frame: &Frame) -> Option<Self> {
        Some(match frame {
            Frame::RoomId { .. } => Self::Room,
            Frame::Compass { .. } => Self::Compass,
            Frame::LeftHand { .. } => Self::Left,
            Frame::RightHand { .. } => Self::Right,
            Frame::Spell { .. } => Self::Spell,
            Frame::RoundTime { .. } => Self::RoundTime,
            Frame::CastTime { .. } => Self::CastTime,
            Frame::StreamWindow { id, .. } => Self::Window(id.clone()),
            Frame::Component { id, .. } => Self::Component(id.clone()),
            Frame::StatusIndicator { id, .. } => Self::Indicator(id.clone()),
            Frame::ProgressBar(bar) => Self::InDialog("bar", bar.dialog.clone(), bar.id.clone()),
            Frame::Label { id, dialog, .. } => Self::InDialog("label", dialog.clone(), id.clone()),
            Frame::InjuryImage { id, dialog, .. } => {
                Self::InDialog("image", dialog.clone(), id.clone())
            }
            Frame::DialogWidgets(widgets) => {
                Self::InDialog("widget", widgets.dialog.clone(), widgets.id.clone())
            }
            Frame::ActiveEffect(effect) => Self::Effect(effect.category.clone(), effect.id.clone()),
            Frame::ClearDialogData { id } => Self::Cleared(id.clone()),
            Frame::ClearStream { id } => Self::Stream(id.clone()),
            _ => return None,
        })
    }

    /// The dialog it is said in, if any, which emptying empties of it.
    fn dialog(&self) -> Option<&str> {
        match self {
            Self::InDialog(_, dialog, _) => dialog.as_deref(),
            Self::Effect(category, _) => Some(category),
            _ => None,
        }
    }
}

/// A connection's login, and the latest word on each piece of state since.
#[derive(Debug, Default)]
pub(crate) struct Kept {
    /// The login, as it came: from the connection's first chunk to the one
    /// that made it ready, and on to a line's end.
    login: Vec<Vec<u8>>,
    login_bytes: usize,
    /// More came than [`LOGIN_BYTES`]: the login is not kept whole.
    login_cut: bool,
    /// The login is over.
    logged_in: bool,
    /// The chunks since the parser last held nothing, and what they said.
    run: Vec<Vec<u8>>,
    run_bytes: usize,
    run_keys: Vec<Key>,
    /// Each run still the latest to say something, by when it came, with
    /// how many things it is still the latest for.
    runs: BTreeMap<u64, (Vec<Vec<u8>>, usize)>,
    /// Which run is the latest to say each thing.
    latest: HashMap<Key, u64>,
    next: u64,
}

impl Kept {
    /// A new connection: its login is kept from its first chunk, and what
    /// the last one said is let go, since the login says it again.
    pub(crate) fn connected(&mut self) {
        *self = Self::default();
    }

    /// Keep `chunk`, which said `keys`, as it came. `ready`: the connection
    /// was ready once it was read. `whole`: the parser then held no part of
    /// a line.
    pub(crate) fn keep(&mut self, chunk: &[u8], keys: Vec<Key>, ready: bool, whole: bool) {
        if !self.logged_in {
            if self.login_bytes + chunk.len() <= LOGIN_BYTES {
                self.login.push(chunk.to_vec());
                self.login_bytes += chunk.len();
            } else {
                self.login_cut = true;
            }
            self.logged_in = ready && whole;
            return;
        }
        self.run_bytes += chunk.len();
        if self.run_bytes <= RUN_BYTES {
            self.run.push(chunk.to_vec());
        }
        self.run_keys.extend(keys);
        if whole {
            self.close_run();
        }
    }

    /// The run is at a line's end: kept if it is the latest to say anything.
    fn close_run(&mut self) {
        let run = std::mem::take(&mut self.run);
        let mut keys = std::mem::take(&mut self.run_keys);
        let bytes = std::mem::take(&mut self.run_bytes);
        if keys.is_empty() || bytes > RUN_BYTES {
            return;
        }
        keys.sort();
        keys.dedup();
        let at = self.next;
        self.next += 1;
        for key in &keys {
            if let Key::Cleared(dialog) = key {
                let gone: Vec<Key> = self
                    .latest
                    .keys()
                    .filter(|said| said.dialog() == Some(dialog))
                    .cloned()
                    .collect();
                for said in gone {
                    self.forget(&said);
                }
            }
        }
        self.runs.insert(at, (run, keys.len()));
        for key in keys {
            if let Some(before) = self.latest.insert(key, at) {
                self.release(before);
            }
        }
    }

    fn forget(&mut self, key: &Key) {
        if let Some(at) = self.latest.remove(key) {
            self.release(at);
        }
    }

    /// The run at `at` is the latest for one thing fewer.
    fn release(&mut self, at: u64) {
        if let Some((_, still)) = self.runs.get_mut(&at) {
            *still -= 1;
            if *still == 0 {
                self.runs.remove(&at);
            }
        }
    }

    /// What a Lich started now is handed before the live stream, as it came:
    /// the login, if it is kept whole, then each run still the latest to say
    /// something, in the order they came, then the run still open, which the
    /// live stream finishes.
    pub(crate) fn replay(&self) -> Vec<u8> {
        let login = if self.login_cut { &[][..] } else { &self.login };
        let runs = self.runs.values().flat_map(|(run, _)| run);
        login
            .iter()
            .chain(runs)
            .chain(&self.run)
            .flatten()
            .copied()
            .collect()
    }

    /// Whether the login is kept whole, and so handed to a Lich started late.
    pub(crate) fn has_login(&self) -> bool {
        !self.login_cut
    }
}

#[cfg(test)]
mod tests {
    use super::{Kept, Key};

    fn indicator(id: &str) -> Vec<Key> {
        vec![Key::Indicator(id.into())]
    }

    fn logged_in() -> Kept {
        let mut kept = Kept::default();
        kept.keep(b"login\n", Vec::new(), false, true);
        kept.keep(b"<prompt>\n", Vec::new(), true, true);
        kept
    }

    /// The login, then the latest run to say each thing, in the order they
    /// came; what said nothing Lich keeps, and what was said again, not.
    #[test]
    fn the_login_then_the_latest_word_on_each() {
        let mut kept = logged_in();
        kept.keep(b"kneel\n", indicator("IconKNEELING"), true, true);
        kept.keep(b"left\n", vec![Key::Left], true, true);
        kept.keep(b"chatter\n", Vec::new(), true, true);
        kept.keep(b"stand\n", indicator("IconKNEELING"), true, true);
        assert_eq!(kept.replay(), b"login\n<prompt>\nleft\nstand\n");
    }

    /// Chunks that end mid-line are kept with the rest of their line, and
    /// the run still open is handed last.
    #[test]
    fn a_run_is_kept_to_its_lines_end() {
        let mut kept = logged_in();
        kept.keep(b"<left>a gem", vec![Key::Left], true, false);
        kept.keep(b"</left>\n", Vec::new(), true, true);
        kept.keep(b"half a li", Vec::new(), true, false);
        assert_eq!(
            kept.replay(),
            b"login\n<prompt>\n<left>a gem</left>\nhalf a li"
        );
    }

    /// Emptying a dialog lets go of what was said in it before.
    #[test]
    fn an_emptied_dialog_lets_go_of_what_was_in_it() {
        let mut kept = logged_in();
        let effect = |id: &str| Key::Effect("Buffs".into(), id.into());
        kept.keep(b"buff 1\n", vec![effect("1")], true, true);
        kept.keep(b"buff 2\n", vec![effect("2")], true, true);
        kept.keep(
            b"clear, buff 2\n",
            vec![Key::Cleared("Buffs".into()), effect("2")],
            true,
            true,
        );
        assert_eq!(kept.replay(), b"login\n<prompt>\nclear, buff 2\n");
    }

    /// The login runs to the chunk that made the connection ready, and on to
    /// a line's end; a new connection starts it again.
    #[test]
    fn the_login_is_the_connections_until_ready() {
        let mut kept = Kept::default();
        kept.keep(b"login\n", Vec::new(), false, true);
        kept.keep(b"<prompt>\nmore", Vec::new(), true, false);
        kept.keep(b" of it\n", indicator("IconSTANDING"), true, true);
        assert_eq!(kept.replay(), b"login\n<prompt>\nmore of it\n");
        kept.connected();
        assert_eq!(kept.replay(), b"");
    }
}
