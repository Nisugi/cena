//! One log window's state: what it shows, and what it asks to read.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use cena_session::player_log::archive::Usage;
use cena_session::player_log::reader::{Entry, Found, Streams};
use cena_session::player_log::writer;

use super::read::{Ask, Days, Inbox, Reply};
use super::view::{Preset, class};

/// The window's tabs.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) enum Tab {
    /// A day, whole, newest at the bottom.
    #[default]
    Recent,
    /// Lines holding a text, newest first.
    Search,
    /// Days written to a file.
    Export,
}

/// One character's log window.
#[derive(Debug)]
pub(crate) struct Logs {
    /// Whose log, as the table named the character.
    pub(crate) character: String,
    /// Whether it is showing.
    pub(crate) open: bool,
    pub(super) tab: Tab,
    pub(super) days: Days,
    pub(super) usage: Option<Usage>,
    /// The day *Recent* shows, once one is chosen.
    pub(super) day: Option<String>,
    pub(super) lines: Vec<Entry>,
    pub(super) query: String,
    pub(super) regex: bool,
    pub(super) found: Option<Found>,
    /// The classes the window has seen, each ticked or not.
    pub(super) ticked: BTreeMap<String, bool>,
    pub(super) dedup: bool,
    pub(super) from: String,
    pub(super) to: String,
    /// What the window last had to say: an error, where an export went.
    pub(super) said: Option<String>,
    /// Reads asked and not yet answered.
    pub(super) waiting: usize,
    /// Asks made before the window was first drawn: its days.
    pub(super) asked: Vec<Ask>,
    pub(super) inbox: Inbox,
}

impl Logs {
    /// A window for `character`, open, asking for its days.
    pub(crate) fn new(character: &str) -> Self {
        Self {
            character: character.to_owned(),
            open: true,
            tab: Tab::Recent,
            days: Vec::new(),
            usage: None,
            day: None,
            lines: Vec::new(),
            query: String::new(),
            regex: false,
            found: None,
            ticked: BTreeMap::new(),
            dedup: false,
            from: String::new(),
            to: String::new(),
            said: None,
            waiting: 0,
            asked: vec![Ask::Days],
            inbox: Inbox::default(),
        }
    }

    /// Where its reads answer.
    pub(crate) fn inbox(&self) -> Inbox {
        Arc::clone(&self.inbox)
    }

    /// What it wants read, taken: each is counted as waiting.
    pub(crate) fn take_asks(&mut self) -> Vec<Ask> {
        let asked = std::mem::take(&mut self.asked);
        self.waiting += asked.len();
        asked
    }

    /// Take what the reads answered.
    pub(crate) fn take_replies(&mut self) {
        let replies = std::mem::take(&mut *crate::sessions::lock(&self.inbox));
        for reply in replies {
            self.waiting = self.waiting.saturating_sub(1);
            self.answer(reply);
        }
    }

    pub(super) fn answer(&mut self, reply: Reply) {
        match reply {
            Reply::Days(Ok((days, usage))) => {
                // Opened on the newest day, and kept on the one chosen.
                let newest = days.first().map(|(day, _)| day.clone());
                if self.day.is_none()
                    && let Some(day) = newest.clone()
                {
                    self.asked.push(Ask::Day(day.clone()));
                    self.day = Some(day);
                }
                if self.from.is_empty() {
                    self.from = newest.clone().unwrap_or_default();
                    self.to = newest.unwrap_or_default();
                }
                self.days = days;
                self.usage = Some(usage);
            }
            Reply::Day(day, Ok(lines)) => {
                if self.day.as_deref() == Some(day.as_str()) {
                    self.learn(&lines);
                    self.lines = lines;
                }
            }
            Reply::Found(Ok(found)) => {
                self.learn(&found.entries);
                self.found = Some(found);
            }
            Reply::Exported(Ok(done)) => {
                self.said = Some(format!(
                    "{} lines from {} days written to {}",
                    done.lines,
                    done.days,
                    done.path.display()
                ));
            }
            Reply::Days(Err(why))
            | Reply::Day(_, Err(why))
            | Reply::Found(Err(why))
            | Reply::Exported(Err(why)) => self.said = Some(why),
        }
    }

    /// Tick each class these lines hold that the window had not seen.
    pub(super) fn learn(&mut self, lines: &[Entry]) {
        for entry in lines {
            self.ticked
                .entry(class(&entry.stream).to_owned())
                .or_insert(true);
        }
    }

    /// Tick what `preset` ticks, and nothing else.
    pub(crate) fn preset(&mut self, preset: Preset) {
        for (class, on) in &mut self.ticked {
            *on = preset.ticks(class);
        }
    }

    /// The tags an export writes: the classes ticked, when any is not.
    pub(super) fn export_streams(&self) -> Streams {
        if self.ticked.values().all(|on| *on) {
            return Streams::all();
        }
        Streams::classes(
            self.ticked
                .iter()
                .filter(|(_, on)| **on)
                .map(|(class, _)| class.clone()),
        )
    }

    /// The folder this character's log is in.
    pub(crate) fn folder(&self) -> PathBuf {
        writer::dir(&writer::root(), &self.character)
    }
}

/// Open `folder` in the system's file browser. What it cannot is said.
pub(crate) fn open_folder(folder: &Path) -> Result<(), String> {
    let program = if cfg!(target_os = "windows") {
        "explorer"
    } else if cfg!(target_os = "macos") {
        "open"
    } else {
        "xdg-open"
    };
    std::fs::create_dir_all(folder).map_err(|e| e.to_string())?;
    std::process::Command::new(program)
        .arg(folder)
        .spawn()
        .map(drop)
        .map_err(|e| format!("{program} could not open {}: {e}", folder.display()))
}
