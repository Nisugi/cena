//! Experience, injuries, stance and encumbrance: **M2 step 3**, `plan/18` §2b.
//!
//! Split out of `state.rs` under Rule 4.1 (`plan/05:352-353`).
//!
//! # All four are wire-driven, and that moved the milestone boundary
//!
//! `plan/18` §1 records the finding: Lich scrapes experience and injuries out of
//! prose with regexes *and* reads the dialog (`common/xmlparser.rb:725-735`), so
//! reading the directory names would have put both in the deferred,
//! text-scraped half. Reading the wire puts them here, with no regex at all.
//!
//! MEASURED over 24 files across 6 characters, `<dialogData id=>` occurrences:
//! `combat` 5,869, `minivitals` 5,235, `expr` 2,439, `Buffs` 2,426,
//! `mapViewMain` 2,229, `injuries` 1,892, `Active Spells` 1,660, `encum` 941,
//! `Debuffs` 840, `Cooldowns` 840, `stance` 377, `quick` 367. The effect dialogs
//! are already handled by `plan/17`; `combat` and `quick*` are UI affordances,
//! not character state (`plan/18` §2b defers them).
//!
//! # The shapes, verbatim from `GSIV-Zoleta`
//!
//! ```text
//! expr:   <label id='yourLvl' value='Level 100'/>
//!         <progressBar id='mindState' value='0' text='clear as a bell'/>
//!         <progressBar id='nextLvlPB' value='100' text='11999265 experience'/>
//! stance: <progressBar id='pbarStance' value='100' text='defensive (100%)'/>
//! encum:  <progressBar id='encumlevel' value='0' text='None'/>
//!         <label id='encumblurb' value='You are not encumbered enough to notice.'/>
//! ```
//!
//! # Injuries are `<image>`, not `<progressBar>` -- and `plan/18` guessed wrong
//!
//! §2b said *"per-body-part injury and scar, 16 parts, named on the wire"* and
//! assumed the shape that carries everything else. It does not. MEASURED: the
//! `injuries` dialog carries only a `health2` bar, and the body parts arrive as
//!
//! ```text
//! <image id="leftArm" name="Injury1" height="0" width="0"/>
//! <image id="neck" name="neck" height="0" width="0"/>
//! ```
//!
//! **`name` equal to `id` means unhurt**; `Injury1`..`Injury3` and
//! `Scar1`..`Scar3` are the severities. VERIFIED against Lich, which reads it
//! identically (`lib/common/xmlparser.rb:809-822`) and supplies two facts the
//! wire alone does not give:
//!
//! * **a scar CLEARS the wound.** They are separate tracks, and a scar is what a
//!   healed wound leaves behind, so reporting both would double-count.
//! * **`nsys`** -- the nervous system -- is one of the parts, not a separate
//!   mechanism.

pub mod vocabulary;

use std::collections::BTreeMap;

/// What the `expr` dialog says about advancement.
///
/// Every field is `Option` because the dialog is **partial**: a burst may carry
/// the level and not the mind state. `None` is "not yet told", which `plan/12`
/// §5.2 makes a first-class value rather than a default.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Experience {
    /// `<label id='yourLvl' value='Level 100'>`, **verbatim**.
    ///
    /// The whole label, not a parsed number. The wire says `Level 100`; turning
    /// that into `100` is a guess about a format the game may change, and every
    /// consumer that wants to display it wants the string back.
    pub level: Option<String>,
    /// `<progressBar id='mindState' text='clear as a bell'>`.
    pub mind_state: Option<String>,
    /// `mindState`'s `value=`, 0-100.
    pub mind_percent: Option<u32>,
    /// `<progressBar id='nextLvlPB' text='11999265 experience'>`.
    pub next_level: Option<String>,
    /// `nextLvlPB`'s `value=`, 0-100.
    pub next_level_percent: Option<u32>,
}

/// How badly one body part is hurt.
///
/// Wound and scar are **separate tracks**, per Lich: a scar is what a healed
/// wound leaves, so a part can be scarred and unwounded at once. Rank 0 is
/// healthy.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Injury {
    /// `Injury1`..`Injury3` -> 1..3. Zero when unhurt.
    pub wound: u8,
    /// `Scar1`..`Scar3` -> 1..3. Zero when unscarred.
    pub scar: u8,
}

impl Injury {
    /// Whether this part is hurt or scarred at all.
    #[must_use]
    pub const fn is_hurt(self) -> bool {
        self.wound > 0 || self.scar > 0
    }
}

/// What the game says about the character, beyond vitals and the room.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Character {
    /// The `expr` dialog.
    pub experience: Experience,
    /// Injuries by body part id (`head`, `leftArm`, `nsys`, ...).
    ///
    /// **Only hurt parts are present.** A healthy part is removed rather than
    /// stored as zero, so `is_empty()` answers "is this character unhurt"
    /// without a scan, and iterating gives exactly what a display should show.
    ///
    /// `BTreeMap` for criterion 7: a replay iterating a `HashMap` would be
    /// non-deterministic.
    pub injuries: BTreeMap<String, Injury>,
    /// `<progressBar id='pbarStance' text='defensive (100%)'>`.
    pub stance: Option<String>,
    /// `pbarStance`'s `value=`: percent of stance contributing to defense.
    pub stance_percent: Option<u32>,
    /// `<progressBar id='encumlevel' text='None'>`.
    pub encumbrance: Option<String>,
    /// `encumlevel`'s `value=`, 0-100.
    pub encumbrance_percent: Option<u32>,
    /// `<label id='encumblurb' value='You are not encumbered enough to notice.'>`.
    pub encumbrance_detail: Option<String>,
}

impl Character {
    /// Fold a `<progressBar>` that belongs to one of step 3's dialogs.
    ///
    /// Returns whether it was consumed, so the caller can fall through to the
    /// vitals path for everything else. The enclosing dialog is what separates
    /// these from vitals: the same `<progressBar>` shape carries all of them.
    pub(super) fn apply_bar(&mut self, dialog: &str, id: &str, text: &str, percent: u32) -> bool {
        let text = (!text.is_empty()).then(|| text.to_owned());
        match (dialog, id) {
            ("expr", "mindState") => {
                self.experience.mind_state = text;
                self.experience.mind_percent = Some(percent);
            }
            ("expr", "nextLvlPB") => {
                self.experience.next_level = text;
                self.experience.next_level_percent = Some(percent);
            }
            // **By id, not by dialog**, and that is measured rather than lax.
            // `pbarStance` arrives in BOTH `stance` and `combat`, 130 times each
            // over 15 files, with identical values every time -- they are one
            // fact shown in two windows. Requiring `dialog == "stance"` rejected
            // the `combat` copy, which then fell through to `vitals` and showed
            // up as a gauge called `pbarStance` sitting beside health.
            //
            // The id is specific enough to be safe: unlike `health` or
            // `mindState`, nothing else on the wire is named `pbarStance`.
            (_, "pbarStance") => {
                self.stance = text;
                self.stance_percent = Some(percent);
            }
            ("encum", "encumlevel") => {
                self.encumbrance = text;
                self.encumbrance_percent = Some(percent);
            }
            _ => return false,
        }
        true
    }

    /// Fold a `<label>` that belongs to one of step 3's dialogs.
    pub(super) fn apply_label(&mut self, dialog: &str, id: &str, value: &str) {
        match (dialog, id) {
            ("expr", "yourLvl") => self.experience.level = Some(value.to_owned()),
            ("encum", "encumblurb") => self.encumbrance_detail = Some(value.to_owned()),
            _ => {}
        }
    }

    /// Fold an `<image>` from the `injuries` dialog.
    ///
    /// `name` equal to `id` means the part is whole; `Injury<n>` and `Scar<n>`
    /// are the severities. **A scar clears the wound**, per Lich
    /// (`xmlparser.rb:813-816`): a scar is what a healed wound leaves behind, so
    /// reporting both would double-count one injury.
    pub(super) fn apply_injury_image(&mut self, part: &str, name: &str) {
        let rank = |prefix: &str| -> Option<u8> { name.strip_prefix(prefix)?.parse().ok() };
        let injury = if let Some(wound) = rank("Injury") {
            Injury { wound, scar: 0 }
        } else if let Some(scar) = rank("Scar") {
            // The wound is gone: this is the mark it left.
            Injury { wound: 0, scar }
        } else {
            // `name == id`, or anything else the game sends: whole.
            Injury::default()
        };

        if injury.is_hurt() {
            self.injuries.insert(part.to_owned(), injury);
        } else {
            self.injuries.remove(part);
        }
    }
}
