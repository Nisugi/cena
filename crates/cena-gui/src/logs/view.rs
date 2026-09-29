//! The reading view: a line's class, the presets, and *Dedup*. None of it
//! changes what is read or written.

use std::collections::BTreeMap;

use cena_session::player_log::reader::Entry;

/// The class of a tag: its last part. `main/combat` is `combat`.
#[must_use]
pub(crate) fn class(tag: &str) -> &str {
    tag.rsplit('/').next().unwrap_or(tag)
}

/// A preset: which classes it ticks.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Preset {
    /// Every class.
    Everything,
    /// Only what our definitions called combat.
    Combat,
    /// What people said.
    Social,
    /// Everything but combat and the room's comings and goings.
    Quiet,
}

impl Preset {
    /// Every preset, in the order the window offers them.
    pub(crate) const ALL: [Preset; 4] = [
        Preset::Everything,
        Preset::Combat,
        Preset::Social,
        Preset::Quiet,
    ];

    /// What the button says.
    pub(crate) const fn label(self) -> &'static str {
        match self {
            Preset::Everything => "Everything",
            Preset::Combat => "Combat",
            Preset::Social => "Social",
            Preset::Quiet => "Quiet",
        }
    }

    /// Whether this preset ticks `class`.
    pub(crate) fn ticks(self, class: &str) -> bool {
        /// Where people talk: the game's streams for it.
        const SOCIAL: &[&str] = &["thoughts", "speech", "whispers", "talk", "ooc"];
        /// The noise a quiet read leaves out, beside combat.
        const NOISE: &[&str] = &[
            "combat",
            "atmospherics",
            "arrivals",
            "logons",
            "death",
            "deaths",
        ];
        match self {
            Preset::Everything => true,
            Preset::Combat => class == "combat",
            Preset::Social => SOCIAL.contains(&class),
            Preset::Quiet => !NOISE.contains(&class),
        }
    }
}

/// One line as the window shows it: the line, and how many times it came in
/// a row when *Dedup* folded it.
pub(crate) type Shown<'a> = (&'a Entry, usize);

/// The lines of `entries` whose class `ticked` ticks (a class it has not
/// heard of is shown), folded when `dedup` says.
pub(crate) fn shown<'a>(
    entries: &'a [Entry],
    ticked: &BTreeMap<String, bool>,
    dedup: bool,
) -> Vec<Shown<'a>> {
    let mut shown: Vec<Shown<'a>> = Vec::new();
    for entry in entries {
        if !ticked.get(class(&entry.stream)).copied().unwrap_or(true) {
            continue;
        }
        if dedup
            && let Some((last, count)) = shown.last_mut()
            && last.stream == entry.stream
            && last.text == entry.text
        {
            *count += 1;
            continue;
        }
        shown.push((entry, 1));
    }
    shown
}
