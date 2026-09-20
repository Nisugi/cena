//! What persists between sessions, and how it goes stale.
//!
//! M3 step 8, the model half. The I/O half is `cena-platform`'s
//! `character_store`; this file is the **shape** on disk and nothing else, so
//! `cena-model` stays free of the filesystem.
//!
//! # The store is the primary record, not a cache
//!
//! > **AUTHOR, 2026-09-19:** *"persisted, like infomon, it's multi part. you
//! > see it parsing the commands there but let's say you go train when you
//! > complete that training session it basically sends the command so you do
//! > it manually to populate then it should stay updated, but that's also why
//! > there is a way to reset and refresh it."*
//!
//! That is the design constraint. Most of what is here can only be taught by a
//! command a person runs by hand -- `info`, `skills`, the five PSM lists,
//! `inventory enhancive totals`. A model rebuilt each session would show a
//! character with no stats until someone typed `info`, so the file on disk is
//! the record and the wire merely updates it.
//!
//! It is also why [`Group`] exists. "Reset and refresh" needs to know **which**
//! group is stale, and a single whole-file timestamp cannot answer that: a
//! character who ran `skills` this morning and `info` last month has one fresh
//! group and one ancient one.
//!
//! # Why serde and not a database
//!
//! `research/04-inherited-decisions.md:1878` (C21). Lich keeps this in `SQLite`
//! -- a per-character table of `key/value/updated_at` (`infomon.rb:86`) -- and
//! the reason is that Ruby has no cheap typed serialization, not that the
//! access pattern wants a database. Every read is "give me this character's
//! stats"; there are no queries, no joins, and one writer.
//!
//! # Staleness is a version AND a timestamp, per group
//!
//! Lich's `db_refresh_needed?` (`cli.rb:73-80`) compares a stored date against
//! a Lich version, on the reasoning that **a parser change invalidates stored
//! values**. That is right and is kept: a snapshot written by an older
//! [`SCHEMA_VERSION`] is not trusted, because the classifier that produced it
//! may have read a column differently.
//!
//! The author asked for *"per-group timestamp plus a schema version"*, which
//! is both halves: the version answers "can this file be believed at all", and
//! the per-group stamp answers "which command should be re-run".

use std::collections::BTreeMap;
use std::time::SystemTime;

use serde::{Deserialize, Serialize};

use super::enhancive::EnhanciveTotals;
use super::psm::PsmSet;
use super::skills::SkillSet;
use super::standing::Standing;
use super::stats::{Identity, Stat, StatKind};

/// The format version of a written snapshot.
///
/// **Bump this whenever a classifier changes what it stores**, not only when a
/// field is added or removed. A snapshot is the output of a parser, so a parser
/// that starts reading a column differently invalidates every file written
/// before it -- which is exactly what Lich's version check exists to catch
/// (`cli.rb:73-80`).
///
/// An older snapshot is not migrated. It is **refused**, which puts the
/// character back to "never synced" and lets the ordinary sync repopulate it.
/// Migration code for a format nobody has shipped is the abstraction Rule -1
/// forbids; when there is a second version and real files to migrate, that is
/// the time to write it.
pub const SCHEMA_VERSION: u32 = 1;

/// Which command taught a group, and therefore which one refreshes it.
///
/// **The grouping is by SOURCE COMMAND, not by subject.** `Stats` and
/// `Identity` are separate groups even though both come from `info`, because
/// Shroud of Deception falsifies the identity while leaving the numbers alone
/// (`blocks.rs`'s `classify_identity`) -- so they can genuinely be fresh and
/// stale at different times.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Group {
    /// The ten statistics, from `info` or `info full`.
    Stats,
    /// Race, profession, gender, age -- also `info`, but separately staleable.
    Identity,
    /// The 46 skills and the spell circles, from `skills`.
    Skills,
    /// The five PSM tables, each from its own `<category> list all all`.
    Psms,
    /// Enhancive totals, from `inventory enhancive totals`.
    Enhancives,
    /// Society, citizenship, warcries and resources.
    ///
    /// **The one group ordinary play keeps current.** The others are only
    /// ever taught by a command someone runs; this one is also taught by
    /// living in the world -- you join a society, you train a PSM, and the
    /// game says so unprompted. See `standing.rs`.
    Standing,
}

impl Group {
    /// All six.
    pub const ALL: [Self; 6] = [
        Self::Stats,
        Self::Identity,
        Self::Skills,
        Self::Psms,
        Self::Enhancives,
        Self::Standing,
    ];

    /// The command that refreshes this group.
    ///
    /// Here rather than in the session layer because it is a fact about the
    /// game, and because "reset and refresh" needs it: a caller asking to
    /// refresh a stale group should not have to know the command by heart.
    ///
    /// [`Self::Psms`] answers with one of five; the caller sends all five, and
    /// `PsmSet` is already keyed per category so a partial refresh is
    /// meaningful.
    #[must_use]
    pub const fn refresh_command(self) -> &'static str {
        match self {
            Self::Stats | Self::Identity => "info full",
            Self::Skills => "skills full",
            Self::Psms => "cman list all all",
            Self::Enhancives => "inventory enhancive totals",
            // One of four, like `Psms`: `citizenship`, `warcry` and
            // `resource` each teach a different part. `society` is the one a
            // caller refreshing a single group most likely means.
            Self::Standing => "society",
        }
    }
}

/// Everything about one character that survives a restart.
///
/// **Not `GameState`.** What is here is what a command taught and what no
/// reconnect re-sends: the room, the hands, the vitals and the prompt are all
/// re-sent by the login burst (`reconnect.rs` measures which), so persisting
/// them would store a belief about a session that has ended.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct CharacterSnapshot {
    /// The format this file was written in. See [`SCHEMA_VERSION`].
    pub schema_version: u32,
    /// The instance, from `<settingsInfo instance=>`.
    ///
    /// **Part of the identity, not decoration.** Lich keys its table
    /// `game_name` (`infomon.rb:86`) because two characters of the same name on
    /// different instances are different characters.
    pub instance: String,
    /// The character's name, from `<playerID>`.
    pub character: String,
    /// The ten statistics.
    pub stats: BTreeMap<StatKind, Stat>,
    /// Race, profession, gender, age.
    pub identity: Identity,
    /// The 46 skills and the spell circles.
    pub skills: SkillSet,
    /// The five PSM tables.
    pub psms: PsmSet,
    /// Enhancive totals.
    pub enhancives: EnhanciveTotals,
    /// Society, citizenship, warcries and resources.
    pub standing: Standing,
    /// When each group was last taught.
    ///
    /// Absent means never. `BTreeMap` rather than five `Option` fields so
    /// iterating the stale ones is a loop rather than five hand-written arms.
    pub updated_at: BTreeMap<Group, SystemTime>,
}

impl CharacterSnapshot {
    /// A snapshot for a character nothing has taught yet.
    #[must_use]
    pub fn new(instance: &str, character: &str) -> Self {
        Self {
            schema_version: SCHEMA_VERSION,
            instance: instance.to_owned(),
            character: character.to_owned(),
            ..Self::default()
        }
    }

    /// When this group was last taught, if ever.
    #[must_use]
    pub fn group_updated_at(&self, group: Group) -> Option<SystemTime> {
        self.updated_at.get(&group).copied()
    }

    /// Record that a group was just taught.
    pub fn touch(&mut self, group: Group, at: SystemTime) {
        self.updated_at.insert(group, at);
    }

    /// Has this group ever been taught?
    #[must_use]
    pub fn is_known(&self, group: Group) -> bool {
        self.updated_at.contains_key(&group)
    }

    /// Groups that need a command run, given how old is too old.
    ///
    /// **A group with no timestamp is always stale**, which is what makes the
    /// first login sync everything. `max_age` covers the other case: a group
    /// taught six months ago describes a character who has trained since.
    ///
    /// Returns them in [`Group::ALL`] order, so a caller issuing the commands
    /// sends them in a stable sequence and a replay is deterministic.
    ///
    /// # A clock that went backwards does not make data fresh
    ///
    /// `duration_since` fails when `now` precedes the stamp -- a corrected
    /// system clock, or a file copied from another machine. That is treated as
    /// **stale**, not as fresh: the alternative is trusting a timestamp we have
    /// just proved is wrong.
    #[must_use]
    pub fn stale_groups(&self, now: SystemTime, max_age: std::time::Duration) -> Vec<Group> {
        Group::ALL
            .into_iter()
            .filter(|group| {
                self.group_updated_at(*group)
                    .and_then(|at| now.duration_since(at).ok())
                    .is_none_or(|age| age > max_age)
            })
            .collect()
    }

    /// Can this snapshot be believed?
    ///
    /// Only when it was written by **this exact** schema version. Not `>=`: a
    /// file from a newer version was written by a classifier this build does
    /// not have, and reading its fields as if they meant what they mean here is
    /// the same error as reading an older one.
    #[must_use]
    pub const fn is_current(&self) -> bool {
        self.schema_version == SCHEMA_VERSION
    }

    /// Does this snapshot describe the character who just logged in?
    ///
    /// **Both halves, and the instance is the one that gets forgotten.** Two
    /// characters of the same name on Prime and Platinum are different people
    /// with different skills; loading one into the other would look like a
    /// character who had silently lost training.
    ///
    /// Case-insensitive on the name because the wire is not consistent about
    /// it -- `<playerID>` and the `Name:` line have differed -- and a store
    /// that missed on case would silently start the character over.
    #[must_use]
    pub fn describes(&self, instance: &str, character: &str) -> bool {
        self.instance.eq_ignore_ascii_case(instance)
            && self.character.eq_ignore_ascii_case(character)
    }
}
