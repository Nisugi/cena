//! `.targetid`: each creature's tag after its name, and the tag taken for
//! its creature when typed (`cena_model::targetid`; the author, 2026-09-30:
//! *"Let's incorporate it into hydra as a configurable option please"*). The
//! tag's look is the player's: three slots, each the creature's unique
//! character, a random one, a separator, or nothing, and its health before
//! or after (*"we need options"*).
//!
//! The switch only. The tagging is the session's, for every viewer
//! ([`SessionHandle::tag_creatures`]); this is the word on Hydra's command
//! line that sets it, and the character's settings file keeps it, in a
//! `targetid` section, through the settings menu's own writer
//! ([`crate::general`]), as `.sorter` is kept. Off until asked.

use std::sync::Arc;

use cena_session::command::claimant::Claimed;
use cena_session::targetid::{Health, Slot, Style};
use cena_session::{Notice, NoticeKind, SessionHandle};
use cena_ui::settings::{Row, RowKind, Value};

use crate::commands::Commands;
use crate::general::{GENERAL, Kept};
use crate::sorter::{Command, parse_switch};

/// The `targetid` section of a character's settings file.
#[derive(Clone, Debug, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub(crate) struct Saved {
    /// Whether tags are shown; off when unset.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) enabled: Option<bool>,
    /// The three slots, as [`Slot::word`] writes each; the default's when
    /// unset.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) slots: Option<[String; 3]>,
    /// Where the health goes, as [`Health::word`] writes it; off when unset.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) health: Option<String>,
}

impl Saved {
    /// The look it keeps, whether shown or not: a slot or a health the file
    /// holds that is not one is the default's.
    pub(crate) fn style(&self) -> Style {
        let default = Style::default();
        let mut slots = default.slots;
        if let Some(words) = &self.slots {
            for (slot, word) in slots.iter_mut().zip(words) {
                *slot = Slot::of(word).unwrap_or(*slot);
            }
        }
        Style {
            slots,
            health: self
                .health
                .as_deref()
                .and_then(Health::of)
                .unwrap_or(default.health),
        }
    }

    /// How tags look while shown; `None` while off.
    pub(crate) fn tags(&self) -> Option<Style> {
        self.enabled.unwrap_or(false).then(|| self.style())
    }
}

/// The name of [`Saved`]'s section.
pub(crate) const SECTION: &str = "targetid";

/// The keys its slots are changed by on the General page, in order.
pub(crate) const SLOT_KEYS: [&str; 3] = ["targetid_slot1", "targetid_slot2", "targetid_slot3"];

/// The key its health is changed by on the General page.
pub(crate) const HEALTH_KEY: &str = "targetid_health";

/// Whether `key` is one of the General page's tag rows.
pub(crate) fn owns(key: &str) -> bool {
    key == SECTION || key == HEALTH_KEY || SLOT_KEYS.contains(&key)
}

/// The General page's rows for the tags, as `saved` keeps them: on or off,
/// each slot, the health.
pub(crate) fn rows(saved: &Saved) -> Vec<Row> {
    let style = saved.style();
    let mut rows = vec![Row {
        key: SECTION.to_owned(),
        label: "Creature tags".to_owned(),
        help: "A tag after each creature's name, and tk <tag> reaching it, as targetid does: \
               .targetid."
            .to_owned(),
        kind: RowKind::Toggle,
        value: Value::On(saved.enabled.unwrap_or(false)),
        here: saved.enabled.is_some(),
        from: None,
    }];
    for (at, key) in SLOT_KEYS.iter().enumerate() {
        rows.push(Row {
            key: (*key).to_owned(),
            label: format!("Creature tag, character {}", at + 1),
            help: "unique (no other creature in the room has it), random, none, or a mark \
                   such as - or /: .targetid slot."
                .to_owned(),
            kind: RowKind::Text,
            value: Value::Text(style.slots[at].word()),
            here: saved.slots.is_some(),
            from: None,
        });
    }
    rows.push(Row {
        key: HEALTH_KEY.to_owned(),
        label: "Creature tag health".to_owned(),
        help: "Its health as a percent, three digits, before the tag or after it: \
               .targetid health."
            .to_owned(),
        kind: RowKind::Choice(
            [Health::Off, Health::Front, Health::Back]
                .iter()
                .map(|health| (health.word().to_owned(), health.word().to_owned()))
                .collect(),
        ),
        value: Value::Text(style.health.word().to_owned()),
        here: saved.health.is_some(),
        from: None,
    });
    rows
}

/// `key`, one of the tag rows, set to `to` in `saved`, or back to its
/// default; what was done, in words.
///
/// # Errors
///
/// Why nothing was changed.
pub(crate) fn change(saved: &mut Saved, key: &str, to: Option<&str>) -> Result<String, String> {
    if key == SECTION {
        saved.enabled = match to.map(str::trim) {
            None => None,
            Some("on") => Some(true),
            Some("off") => Some(false),
            Some(other) => return Err(format!("`{other}` is not on or off.")),
        };
        return Ok(format!(
            "Creature tags {}.",
            if saved.enabled.unwrap_or(false) {
                "on"
            } else {
                "off"
            }
        ));
    }
    if key == HEALTH_KEY {
        saved.health = to
            .map(|to| {
                Health::of(to)
                    .map(|health| health.word().to_owned())
                    .ok_or_else(|| format!("A tag's health is off, front or back, not `{to}`."))
            })
            .transpose()?;
        return Ok(format!(
            "Creature tag health {}.",
            saved.style().health.word()
        ));
    }
    let at = SLOT_KEYS
        .iter()
        .position(|slot| *slot == key)
        .ok_or_else(|| format!("There is no setting {key}."))?;
    let mut words = saved
        .slots
        .clone()
        .unwrap_or_else(|| Style::default().slots.map(Slot::word));
    words[at] = match to {
        Some(to) => Slot::of(to).map(Slot::word).ok_or_else(|| {
            format!("`{to}` is not unique, random, none, or a mark that is not a letter or digit.")
        })?,
        None => Style::default().slots[at].word(),
    };
    saved.slots = Some(words);
    Ok(format!(
        "Creature tag character {} is {}.",
        at + 1,
        saved.style().slots[at].word()
    ))
}

/// The words `.targetid` knows.
const USAGE: &str = "targetid [on|off|status], targetid slot <1-3> <unique|random|none|a mark like - or />, \
                     targetid health <off|front|back>";

/// One `.targetid`, parsed.
#[derive(Debug, PartialEq, Eq)]
enum Asked {
    /// On, off, flip or say.
    Switch(Command),
    /// A slot, 0 to 2, set to what the word says.
    Slot(usize, Slot),
    /// Where the health goes.
    Health(Health),
}

/// `line`, without its symbol, as `.targetid`: `None` when it is not one,
/// an error holding what was not understood.
fn parse(line: &str) -> Option<Result<Asked, String>> {
    let words: Vec<&str> = line.split_whitespace().collect();
    let [word, what, rest @ ..] = words.as_slice() else {
        return parse_switch(line, "targetid").map(|parsed| parsed.map(Asked::Switch));
    };
    if !word.eq_ignore_ascii_case("targetid") {
        return None;
    }
    let asked = match (what.to_ascii_lowercase().as_str(), rest) {
        ("slot", [n, slot]) => n
            .parse::<usize>()
            .ok()
            .filter(|n| (1..=3).contains(n))
            .zip(Slot::of(slot))
            .map(|(n, slot)| Asked::Slot(n - 1, slot)),
        ("health", [health]) => Health::of(health).map(Asked::Health),
        ("slot" | "health", _) => None,
        _ => return parse_switch(line, "targetid").map(|parsed| parsed.map(Asked::Switch)),
    };
    Some(asked.ok_or_else(|| format!("{what} {}", rest.join(" "))))
}

/// Do `parsed` to `handle`'s session, save it in `kept` when there is
/// somewhere to keep it, and say what became of it.
fn answer(parsed: Result<Asked, String>, handle: &SessionHandle, kept: Option<&Kept>) -> Notice {
    let now = handle.tags_creatures();
    let (key, value, tags) = match parsed {
        Ok(Asked::Switch(Command::Status)) => {
            return Notice::line(NoticeKind::Info, said(now));
        }
        Ok(Asked::Switch(command)) => {
            let on = match command {
                Command::Set(on) => on,
                _ => now.is_none(),
            };
            let word = if on { "on" } else { "off" };
            (
                SECTION,
                word.to_owned(),
                on.then(|| now.unwrap_or_default()),
            )
        }
        Ok(Asked::Slot(at, slot)) => {
            let tags = now.map(|mut style| {
                style.slots[at] = slot;
                style
            });
            (SLOT_KEYS[at], slot.word(), tags)
        }
        Ok(Asked::Health(health)) => {
            let tags = now.map(|style| Style { health, ..style });
            (HEALTH_KEY, health.word().to_owned(), tags)
        }
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
            // The file, whole, is what the session takes: the look too.
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

/// What the tags are now, in words, with one as it looks.
fn said(tags: Option<Style>) -> String {
    match tags {
        Some(style) => {
            let example = cena_session::targetid::Mark {
                unique: 'A',
                random: ['K', '7', 'Q'],
            };
            format!(
                "Creature tags on, like (a kobold ({})); tk <tag> and the script's other commands work.",
                cena_session::targetid::tag(example, style, Some(42))
            )
        }
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
    fn targetid_takes_on_off_status_slots_and_health() {
        let switch = |command| Some(Ok(Asked::Switch(command)));
        assert_eq!(parse("targetid"), switch(Command::Toggle));
        assert_eq!(parse("TARGETID on"), switch(Command::Set(true)));
        assert_eq!(parse("targetid status"), switch(Command::Status));
        assert_eq!(
            parse("targetid slot 3 -"),
            Some(Ok(Asked::Slot(2, Slot::Separator('-'))))
        );
        assert_eq!(
            parse("targetid slot 1 unique"),
            Some(Ok(Asked::Slot(0, Slot::Unique)))
        );
        assert_eq!(
            parse("targetid health back"),
            Some(Ok(Asked::Health(Health::Back)))
        );
        assert!(matches!(parse("targetid slot 4 random"), Some(Err(_))));
        assert!(matches!(parse("targetid slot 1 x"), Some(Err(_))));
        assert!(matches!(parse("targetid health up"), Some(Err(_))));
        assert!(matches!(parse("targetid maybe"), Some(Err(_))));
        assert_eq!(parse("sorter"), None);
    }

    #[test]
    fn the_saved_look_applies_only_while_on() {
        let saved = Saved {
            enabled: None,
            slots: Some(["unique".into(), "random".into(), "-".into()]),
            health: Some("back".into()),
        };
        assert_eq!(saved.tags(), None, "off until asked");
        let on = Saved {
            enabled: Some(true),
            ..saved
        };
        assert_eq!(
            on.tags(),
            Some(Style {
                slots: [Slot::Unique, Slot::Random, Slot::Separator('-')],
                health: Health::Back,
            })
        );
        let odd = Saved {
            enabled: Some(true),
            slots: Some(["what".into(), "none".into(), "none".into()]),
            health: None,
        };
        assert_eq!(
            odd.tags().map(|style| style.slots),
            Some([Slot::Unique, Slot::None, Slot::None]),
            "what is not a slot is the default's"
        );
    }
}
