//! `<crtrStatus>`: what the wire says a creature is doing right now.
//!
//! A stateless classifier over one frame, the shape `plan/12` §3a asks for.
//! Ports the two flag tables in `lib/common/creature/creature_base.rb:104-147`.
//!
//! # Two vocabularies, because they answer different questions
//!
//! Lich keeps them in separate hashes and reads them through different
//! predicates, and the distinction is real:
//!
//! - **[`Status`]** is transient -- `stunned`, `webbed`, `prone`. It changes
//!   several times per fight and is the answer to *what is it doing*.
//! - **[`Classification`]** is a standing fact -- `hostile`, `dead`,
//!   `MiniBoss`. It is the answer to *what is it*.
//!
//! A caller picking a target wants both and wants them apart: a `stunned`
//! creature is worth attacking, a `dead` one is not, and no single flag set
//! makes that distinction legible.
//!
//! # One flag is renamed on the way in
//!
//! The wire says `immobile` and Lich stores `immobilized`
//! (`creature_base.rb:113`), because its message parser uses the longer word
//! and the two detections have to reconcile. That rename is preserved: the
//! wire spelling is what [`Status::parse`] accepts and the canonical spelling
//! is what [`Status::as_str`] prints, so both ends stay readable.
//!
//! MEASURED in `crates/cena-protocol/tests/fixtures/creature_status.xml`:
//! `immobile` appears on the wire, `immobilized` never does.
//!
//! # The frame hands up raw attributes deliberately
//!
//! `Frame::CreatureStatus` keeps `attrs` unparsed *"so the layer above owns
//! the flag-name mapping"* (`frame.rs:201`, Vellum's own principle). This is
//! that layer.
//!
//! # Real hit points, and when they started arriving
//!
//! `health=` and `maxhealth=` are **absolute hit points**, not a percentage
//! and not a flag: `health="30" maxhealth="600"` is a creature nearly dead.
//! This is the game's own answer to a question that used to require
//! inference -- a combat tracker accumulating damage against a bestiary
//! estimate, which is what [`CreatureInstance`](crate::CreatureInstance)
//! still does when the wire says nothing.
//!
//! **The feature is new.** MEASURED over the author's September logs: health
//! appears in **2 files of 127**, both 2026-09-19 or later, and in **zero**
//! older ones -- including one session carrying 11,414 `<crtrStatus>` tags
//! and no health at all. The same creature proves it is not a per-creature
//! property: the jeweler Etaenia (`exist="-285052"`) appears **236 times
//! without health and 7 times with**.
//!
//! Until this was typed, both attributes fell into [`CreatureStatus::unknown`]
//! -- which is Rule 2.2 working exactly as intended. The map surfaced a
//! protocol change; nobody read it for a day.
//!
//! ## Hostile creatures always carry it
//!
//! > **AUTHOR, 2026-09-20:** *"we assume the server sends hp data for all
//! > hostile creatures."*
//!
//! VERIFIED in the two health-era files: **331 of 331** rows carrying
//! `hostile=` also carry `health=`, with none missing it.
//!
//! ## Health goes NEGATIVE, so it is signed
//!
//! A dead creature reads `health="-10" maxhealth="360" dead="1"`. An unsigned
//! field cannot parse that: it fails, and [`CreatureStatus::health`] silently
//! becomes `None` -- so a creature with a `dead` flag would report *no health
//! information at all*, which is the opposite of what the row says.
//!
//! Found by driving a real log rather than by review: an early `u32` version
//! of this code reported 327 of 331 hostile rows carrying health where raw
//! `grep` counted 331. The four it lost were the dead ones.
//!
//! ## `maxhealth="0"` means no HP model, NOT "not hostile"
//!
//! All 42 `maxhealth="0"` rows are non-hostile, so the author's reading that
//! a zero marks a non-combatant holds in that direction. It does **not**
//! invert: the jeweler carries `health="240" maxhealth="240"` and is not
//! hostile. So a zero max is "this NPC has no hit points to speak of", and
//! [`CreatureStatus::health_percent`] returns `None` for it rather than
//! dividing by zero.
//!
//! # A flag that is present but `0` is OFF
//!
//! The wire writes `stunned="1"` to set and may write `stunned="0"` to clear.
//! Both are a statement, which is why [`CreatureStatus::from_attrs`] records
//! `false` rather than absence for the second -- and why
//! [`CreatureStatus::stated`] exists: a caller can tell "the game said it is
//! not stunned" from "the game did not mention stunned".

use std::collections::BTreeMap;

/// A transient combat state.
///
/// `CRTR_STATUS_FLAGS` (`creature_base.rb:112-126`). A closed vocabulary, so
/// typed variants per C21.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Status {
    /// Cannot move. **The wire spells this `immobile`.**
    Immobilized,
    /// Held in a web.
    Webbed,
    /// Asleep.
    Sleeping,
    /// Disoriented.
    Disoriented,
    /// Stunned.
    Stunned,
    /// Rooted in place.
    Rooted,
    /// Calmed, and will not attack.
    Calm,
    /// Kneeling.
    Kneeling,
    /// Prone.
    Prone,
    /// Sitting.
    Sitting,
    /// Flying.
    Flying,
    /// Hovering.
    Hovering,
    /// Hidden.
    Hidden,
}

impl Status {
    /// Every transient status, in `creature_base.rb`'s order.
    pub const ALL: [Self; 13] = [
        Self::Immobilized,
        Self::Webbed,
        Self::Sleeping,
        Self::Disoriented,
        Self::Stunned,
        Self::Rooted,
        Self::Calm,
        Self::Kneeling,
        Self::Prone,
        Self::Sitting,
        Self::Flying,
        Self::Hovering,
        Self::Hidden,
    ];

    /// The attribute name the wire uses.
    ///
    /// Differs from [`Self::as_str`] for exactly one status: the wire says
    /// `immobile`.
    #[must_use]
    pub const fn wire_name(self) -> &'static str {
        match self {
            Self::Immobilized => "immobile",
            Self::Webbed => "webbed",
            Self::Sleeping => "sleeping",
            Self::Disoriented => "disoriented",
            Self::Stunned => "stunned",
            Self::Rooted => "rooted",
            Self::Calm => "calmed",
            Self::Kneeling => "kneeling",
            Self::Prone => "prone",
            Self::Sitting => "sitting",
            Self::Flying => "flying",
            Self::Hovering => "hovering",
            Self::Hidden => "hidden",
        }
    }

    /// The canonical name, which Lich's message parser also produces.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Immobilized => "immobilized",
            Self::Calm => "calm",
            other => other.wire_name(),
        }
    }

    /// Parse a wire attribute name.
    #[must_use]
    pub fn parse(wire_name: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|s| s.wire_name() == wire_name)
    }

    /// **Does this status stop the creature acting?**
    ///
    /// Lich calls this `muckled?` (`creature.rb:658`). The one question a
    /// hunting consumer asks of a status set, and the reason the vocabulary is
    /// typed rather than a bag of strings.
    ///
    /// `Prone`, `Kneeling`, `Sitting`, `Flying`, `Hovering` and `Hidden` are
    /// **not** muckles: a prone creature still attacks.
    #[must_use]
    pub const fn is_muckle(self) -> bool {
        matches!(
            self,
            Self::Immobilized
                | Self::Webbed
                | Self::Sleeping
                | Self::Stunned
                | Self::Rooted
                | Self::Calm
                | Self::Disoriented
        )
    }
}

/// A standing fact about a creature, rather than a transient state.
///
/// `CRTR_CLASSIFICATION_FLAGS` (`creature_base.rb:135-147`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Classification {
    /// Will attack. Attackable.
    Hostile,
    /// Disengaged from combat.
    Disengaged,
    /// Dead.
    Dead,
    /// Under Sympathy.
    Sympathetic,
    /// An ascended creature.
    Ascended,
    /// Far below the character's level.
    Inferior,
    /// An Ascension boss.
    AscensionBoss,
    /// A mini-boss.
    MiniBoss,
    /// Challenging for the character's level.
    Challenging,
    /// Riding a mount.
    Rider,
    /// Being ridden.
    Mount,
}

impl Classification {
    /// Every classification, in `creature_base.rb`'s order.
    pub const ALL: [Self; 11] = [
        Self::Hostile,
        Self::Disengaged,
        Self::Dead,
        Self::Sympathetic,
        Self::Ascended,
        Self::Inferior,
        Self::AscensionBoss,
        Self::MiniBoss,
        Self::Challenging,
        Self::Rider,
        Self::Mount,
    ];

    /// The attribute name the wire uses.
    ///
    /// **Two are camel-cased on the wire** -- `AscensionBoss` and `MiniBoss` --
    /// where every other flag is lowercase. Ported verbatim because the wire
    /// is the wire; a normalising `to_lowercase` here would silently stop
    /// matching them.
    #[must_use]
    pub const fn wire_name(self) -> &'static str {
        match self {
            Self::Hostile => "hostile",
            Self::Disengaged => "disengaged",
            Self::Dead => "dead",
            Self::Sympathetic => "sympathetic",
            Self::Ascended => "ascended",
            Self::Inferior => "inferior",
            Self::AscensionBoss => "AscensionBoss",
            Self::MiniBoss => "MiniBoss",
            Self::Challenging => "challenging",
            Self::Rider => "rider",
            Self::Mount => "mount",
        }
    }

    /// Parse a wire attribute name. Case-sensitive, per [`Self::wire_name`].
    #[must_use]
    pub fn parse(wire_name: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|c| c.wire_name() == wire_name)
    }
}

/// One creature's status, as a single `<crtrStatus>` stated it.
///
/// **This is what the frame said, not accumulated state.** A creature's status
/// over time is a stateful consumer's job (`creature_base.rb`'s registry,
/// deliberately not ported); this is the classifier under it.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CreatureStatus {
    /// The `exist=` id this frame is about.
    pub id: String,
    /// Each transient status the frame mentioned, and whether it was set.
    pub statuses: BTreeMap<Status, bool>,
    /// Each classification the frame mentioned, and whether it was set.
    pub classifications: BTreeMap<Classification, bool>,
    /// `health=`: current hit points, absolute.
    ///
    /// **Signed**: a dead creature reads `health="-10"`. `None` when
    /// unstated.
    pub health: Option<i32>,
    /// `maxhealth=`: maximum hit points. `None` when unstated, and `0` when
    /// the creature has no HP model -- see [`Self::health_percent`].
    pub max_health: Option<u32>,
    /// Attributes that are neither, other than `exist`.
    ///
    /// Non-empty means the game added a flag since this table was cut. Kept
    /// and counted rather than ignored (Rule 2.2) -- a silently dropped flag
    /// is how a client stops noticing that creatures can now be, say, `feared`.
    pub unknown: BTreeMap<String, String>,
}

impl CreatureStatus {
    /// Classify one `<crtrStatus>` frame's attributes.
    ///
    /// `id` is the frame's `exist=`; `attrs` its remaining attributes.
    #[must_use]
    pub fn from_attrs<'a, I>(id: &str, attrs: I) -> Self
    where
        I: IntoIterator<Item = (&'a str, &'a str)>,
    {
        let mut out = Self {
            id: id.to_owned(),
            ..Self::default()
        };
        for (name, value) in attrs {
            if name == "exist" {
                continue;
            }
            // Absolute hit points, not flags: parsed as numbers before the
            // `"0"`-means-off rule below, which would read `health="0"` --
            // a dead creature -- as "the health flag is off".
            match name {
                "health" => {
                    out.health = value.trim().parse().ok();
                    continue;
                }
                "maxhealth" => {
                    out.max_health = value.trim().parse().ok();
                    continue;
                }
                _ => {}
            }
            // `stunned="1"` sets, `stunned="0"` clears. Both are statements.
            let on = value != "0";
            if let Some(status) = Status::parse(name) {
                out.statuses.insert(status, on);
            } else if let Some(class) = Classification::parse(name) {
                out.classifications.insert(class, on);
            } else {
                out.unknown.insert(name.to_owned(), value.to_owned());
            }
        }
        out
    }

    /// Current health as a percentage, 0..=100.
    ///
    /// `None` when either value is unstated, **or when `maxhealth` is 0** --
    /// an NPC with no HP model, 42 such rows in the author's logs. Reporting
    /// 0% for a shopkeeper would read as "nearly dead" to anything choosing
    /// a target.
    #[must_use]
    pub fn health_percent(&self) -> Option<u32> {
        let (health, max) = (self.health?, self.max_health?);
        if max == 0 {
            return None;
        }
        // Negative health is a dead creature, and 0% is the honest reading;
        // clamping before the cast keeps the unsigned result meaningful.
        let health = health.max(0);
        // The `.min(100)` is DEFENSIVE, not measured: `health > maxhealth`
        // occurs 0 times in 677 health-bearing rows, so no test pins it and
        // a mutation removing it survives. It stays because a percentage
        // above 100 would be nonsense in a gauge, and `VellumFE` clamps the
        // same way (`src/core/state.rs:837`) -- but it is an assumption
        // about the wire, not a fact from it.
        Some(((health.unsigned_abs() * 100) / max).min(100))
    }

    /// Is this status set?
    ///
    /// `false` both when the frame said it is off and when the frame did not
    /// mention it. Use [`Self::stated`] to tell those apart.
    #[must_use]
    pub fn is(&self, status: Status) -> bool {
        self.statuses.get(&status).copied().unwrap_or(false)
    }

    /// Is this classification set?
    #[must_use]
    pub fn is_classified(&self, class: Classification) -> bool {
        self.classifications.get(&class).copied().unwrap_or(false)
    }

    /// **Did this frame mention the status at all?**
    ///
    /// The three-valued answer: `Some(true)` set, `Some(false)` explicitly
    /// cleared, `None` not mentioned. A `<crtrStatus>` is a delta rather than
    /// a full snapshot, so "not mentioned" carries no information and must not
    /// read as "off" for a consumer accumulating across frames.
    #[must_use]
    pub fn stated(&self, status: Status) -> Option<bool> {
        self.statuses.get(&status).copied()
    }

    /// Did this frame mention the classification at all?
    #[must_use]
    pub fn stated_classification(&self, class: Classification) -> Option<bool> {
        self.classifications.get(&class).copied()
    }

    /// **Is this creature unable to act?**
    ///
    /// Lich's `muckled?` (`creature.rb:658`): any set status that stops it
    /// acting. The question a hunting consumer asks before deciding whether a
    /// room is safe to loot in.
    #[must_use]
    pub fn is_muckled(&self) -> bool {
        self.statuses
            .iter()
            .any(|(status, &on)| on && status.is_muckle())
    }

    /// Is this a worthwhile target: hostile, and not already dead?
    ///
    /// Lich's `valid_target?` (`creature.rb:642`) is `!crtr_flag?(:dead)`, with
    /// hostility checked separately by `targets`. Both are here because a
    /// caller asking "should I attack this" wants one answer.
    #[must_use]
    pub fn is_attackable(&self) -> bool {
        self.is_classified(Classification::Hostile) && !self.is_classified(Classification::Dead)
    }
}
