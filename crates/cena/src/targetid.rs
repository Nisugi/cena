//! `.targetid`: each creature's tag after its name, and the tag taken for
//! its creature when typed after a tag verb (`cena_model::targetid`; the
//! author, 2026-09-30: *"Let's incorporate it into hydra as a configurable
//! option please"*).
//!
//! The switch only. The tagging is the session's, for every viewer
//! ([`SessionHandle::tag_creatures`]); this is the word on Hydra's command
//! line that flips it, and the character's settings file keeps it, in a
//! `targetid` section, through the settings menu's own writer
//! ([`crate::general`]), as `.sorter` is kept. Off until asked.

use std::sync::Arc;

use cena_session::command::claimant::Claimed;
use cena_session::{Notice, NoticeKind, SessionHandle};

use crate::commands::Commands;
use crate::general::{GENERAL, Kept};
use crate::sorter::{Command, parse_switch};

/// The name of the section it is kept in: `enabled`, as the sorter's is.
pub(crate) const SECTION: &str = "targetid";

/// The words `.targetid` knows.
const USAGE: &str = "targetid [on|off|status]";

/// Do `parsed` to `handle`'s session, save it in `kept` when there is
/// somewhere to keep it, and say what became of it.
fn answer(parsed: Result<Command, String>, handle: &SessionHandle, kept: Option<&Kept>) -> Notice {
    let now = handle.tags_creatures();
    let state = |on: bool| if on { "on" } else { "off" };
    let on = match parsed {
        Ok(Command::Toggle) => !now,
        Ok(Command::Set(on)) => on,
        Ok(Command::Status) => now,
        Err(rest) => {
            return Notice::line(
                NoticeKind::Error,
                format!(
                    "Targetid: `{rest}` is not a word it knows. Usage: {USAGE} (currently {}).",
                    state(now)
                ),
            );
        }
    };
    handle.tag_creatures(on);
    let saved = match (parsed, kept) {
        (Ok(Command::Status), _) | (_, None) => Ok(String::new()),
        (_, Some(kept)) => kept.change(GENERAL, SECTION, Some(state(on))),
    };
    let said = if on {
        "Creature tags on: a tag after each creature's name, and kill <tag> reaches it."
    } else {
        "Creature tags off."
    };
    match saved {
        Ok(_) => Notice::line(NoticeKind::Info, said),
        Err(why) => Notice::line(NoticeKind::Warn, format!("{said} For this session: {why}")),
    }
}

/// Register `.targetid` on `handle`'s command line, saved in `kept`, the
/// character's settings file, which gave the session its switch at start.
pub(crate) fn open(handle: &SessionHandle, commands: &Commands, kept: Option<Kept>) {
    let told = handle.clone();
    commands.targetid(Arc::new(move |line: &str| {
        let parsed = parse_switch(line, "targetid")?;
        told.say(answer(parsed, &told, kept.as_ref()).answering());
        Some(Claimed::Done)
    }));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn targetid_takes_on_off_and_status() {
        assert_eq!(
            parse_switch("targetid", "targetid"),
            Some(Ok(Command::Toggle))
        );
        assert_eq!(
            parse_switch("TARGETID on", "targetid"),
            Some(Ok(Command::Set(true)))
        );
        assert_eq!(
            parse_switch("targetid status", "targetid"),
            Some(Ok(Command::Status))
        );
        assert!(matches!(
            parse_switch("targetid maybe", "targetid"),
            Some(Err(_))
        ));
        assert_eq!(parse_switch("sorter", "targetid"), None);
    }
}
