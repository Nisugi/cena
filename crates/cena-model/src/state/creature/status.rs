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
