//! `.targetid`: each creature's tag after its name, and the tag taken for
//! its creature when typed after a tag verb (`cena_model::targetid`; the
//! author, 2026-09-30: *"Let's incorporate it into hydra as a configurable
//! option please"*), its length the player's to choose, 1 to 6, 3 unless
//! chosen (*"3 can be default but a setting to change it"*).
//!
//! The switch only. The tagging is the session's, for every viewer
//! ([`SessionHandle::tag_creatures`]); this is the word on Hydra's command
//! line that sets it, and the character's settings file keeps it, in a
//! `targetid` section, through the settings menu's own writer
//! ([`crate::general`]), as `.sorter` is kept. Off until asked.

use std::sync::Arc;

use cena_session::command::claimant::Claimed;
use cena_session::targetid::{DEFAULT_LENGTH, LONGEST};
use cena_session::{Notice, NoticeKind, SessionHandle};

use crate::commands::Commands;
use crate::general::{GENERAL, Kept};
use crate::sorter::{Command, parse_switch};

/// The `targetid` section of a character's settings file.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub(crate) struct Saved {
    /// Whether tags are shown; off when unset.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) enabled: Option<bool>,
    /// How many characters a tag is; [`DEFAULT_LENGTH`] when unset.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) length: Option<u8>,
}

impl Saved {
    /// How long each tag is while shown; `None` while off.
    pub(crate) fn tags(self) -> Option<usize> {
        self.enabled.unwrap_or(false).then(|| {
            self.length
                .map_or(DEFAULT_LENGTH, usize::from)
                .clamp(1, LONGEST)
        })
    }
}

/// The name of [`Saved`]'s section.
pub(crate) const SECTION: &str = "targetid";

/// The key its length is changed by on the General page.
pub(crate) const LENGTH_KEY: &str = "targetid_length";

/// The words `.targetid` knows.
const USAGE: &str = "targetid [on|off|status|length 1-6]";

/// One `.targetid`, parsed.
#[derive(Debug, PartialEq, Eq)]
enum Asked {
    /// On, off, flip or say.
    Switch(Command),
    /// A tag this many characters long.
    Length(usize),
}

/// `line`, without its symbol, as `.targetid`: `None` when it is not one,
/// an error holding what was not understood.
fn parse(line: &str) -> Option<Result<Asked, String>> {
    let words: Vec<&str> = line.split_whitespace().collect();
    match words.as_slice() {
        [word, length, rest @ ..]
            if word.eq_ignore_ascii_case("targetid") && length.eq_ignore_ascii_case("length") =>
        {
            Some(
                match rest {
                    [n] => n
                        .parse::<usize>()
                        .ok()
                        .filter(|n| (1..=LONGEST).contains(n)),
                    _ => None,
                }
                .map(Asked::Length)
                .ok_or_else(|| format!("length {}", rest.join(" "))),
            )
        }
        _ => parse_switch(line, "targetid").map(|parsed| parsed.map(Asked::Switch)),
    }
}

/// Do `parsed` to `handle`'s session, save it in `kept` when there is
/// somewhere to keep it, and say what became of it.
fn answer(parsed: Result<Asked, String>, handle: &SessionHandle, kept: Option<&Kept>) -> Notice {
    let now = handle.tags_creatures();
    let state = |on: bool| if on { "on" } else { "off" };
    let (key, value, tags) = match parsed {
        Ok(Asked::Switch(Command::Status)) => {
            return Notice::line(NoticeKind::Info, said(now));
        }
        Ok(Asked::Switch(command)) => {
            let on = match command {
                Command::Set(on) => on,
                _ => now.is_none(),
            };
            let length = now.unwrap_or(DEFAULT_LENGTH);
            (SECTION, state(on).to_owned(), on.then_some(length))
        }
        Ok(Asked::Length(length)) => (LENGTH_KEY, length.to_string(), now.map(|_| length)),
        Err(rest) => {
            return Notice::line(
                NoticeKind::Error,
                format!("Targetid: `{rest}` is not a word it knows. Usage: {USAGE}."),
            );
        }
    };
    let Some(kept) = kept else {
        handle.tag_creatures(tags);
        return Notice::line(NoticeKind::Info, said(tags));
    };
    match kept.change(GENERAL, key, Some(&value)) {
        Ok(_) => {
            // The file, whole, is what the session takes: its length too.
            kept.take(handle);
            Notice::line(NoticeKind::Info, said(handle.tags_creatures()))
        }
        Err(why) => {
            handle.tag_creatures(tags);
            Notice::line(
                NoticeKind::Warn,
                format!("{} For this session: {why}", said(tags)),
            )
        }
    }
}

/// What the tags are now, in words.
fn said(tags: Option<usize>) -> String {
    match tags {
        Some(length) => format!(
            "Creature tags on, {length} character{} long.",
            if length == 1 { "" } else { "s" }
        ),
        None => "Creature tags off.".to_owned(),
    }
}

/// Register `.targetid` on `handle`'s command line, saved in `kept`, the
/// character's settings file, which gave the session its switch at start.
pub(crate) fn open(handle: &SessionHandle, commands: &Commands, kept: Option<Kept>) {
    let told = handle.clone();
    commands.targetid(Arc::new(move |line: &str| {
        let parsed = parse(line)?;
        told.say(answer(parsed, &told, kept.as_ref()).answering());
        Some(Claimed::Done)
    }));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn targetid_takes_on_off_status_and_a_length() {
        let switch = |command| Some(Ok(Asked::Switch(command)));
        assert_eq!(parse("targetid"), switch(Command::Toggle));
        assert_eq!(parse("TARGETID on"), switch(Command::Set(true)));
        assert_eq!(parse("targetid status"), switch(Command::Status));
        assert_eq!(parse("targetid length 4"), Some(Ok(Asked::Length(4))));
        assert!(matches!(parse("targetid length 7"), Some(Err(_))));
        assert!(matches!(parse("targetid length"), Some(Err(_))));
        assert!(matches!(parse("targetid maybe"), Some(Err(_))));
        assert_eq!(parse("sorter"), None);
    }

    #[test]
    fn a_saved_length_applies_only_while_on() {
        let saved = |enabled, length| Saved { enabled, length };
        assert_eq!(saved(None, Some(4)).tags(), None, "off until asked");
        assert_eq!(saved(Some(true), None).tags(), Some(DEFAULT_LENGTH));
        assert_eq!(saved(Some(true), Some(5)).tags(), Some(5));
        assert_eq!(saved(Some(true), Some(9)).tags(), Some(LONGEST));
    }
}
