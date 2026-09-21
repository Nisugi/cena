//! Who is grouped with you.
//!
//! # The port is half the size, and §3a is why
//!
//! Lich's patterns match **raw XML** (`gemstone/group.rb:418-470`):
//!
//! ```text
//! JOIN = %r{^<a exist="(?<id>[\d-]+)" noun="(?<noun>[A-Za-z]+)">(?<name>\w+?)</a> joins your group.}
//! ```
//!
//! It has to, because `GameObj` does not hand it the link -- so every pattern
//! re-tokenizes the markup to recover `exist=` and `noun=`, and each one
//! repeats that 60-character fragment once per participant.
//!
//! Cena's parser already did that. [`ChunkLine::links`] yields
//! [`LinkKind::Exist`] with `id` and `noun` typed, so a pattern here only has
//! to say **which event** a line is; the participants are the links, in order.
//!
//! That is not a rewrite of Lich's design -- it is Lich's design with one half
//! deleted. It already separates the two: `people = exist(line)`
//! (`group.rb:608`) scans the links independently and the handlers use THAT,
//! never the regexes' captures. Which is also why `SWAP_LEADER`'s duplicate
//! capture-group names (`group.rb:442`, three pairs of them) are harmless:
//! Ruby keeps only the last of each, and nothing reads them.
//!
//! # The members are ids, not names
//!
//! A group holds `exist` ids. Two characters can display the same name -- the
//! possessive pronoun in `adds you to <a>his</a> group` is itself a link, with
//! the leader's id -- so the id is the identity and the name is decoration.

use cena_protocol::frame::LinkKind;

use crate::state::chunks::ChunkLine;

/// One participant in a group event: the link the game sent.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Member {
    /// `exist=`, verbatim. The identity.
    pub id: String,
    /// `noun=`. What a command targets them by.
    pub noun: String,
    /// The link's display text, which may be a pronoun (`his`, `her`).
    pub text: String,
}

/// What one line said about the group.
///
/// Ported from `group.rb:418-470`'s patterns, minus the two that need no
/// pattern here: `GROUP_EMPTIED` is `<indicator id='IconJOINED' visible='n'/>`,
/// which arrives as a `Frame::StatusIndicator` and is already stored by
/// `GameState::apply`, and the `EXIST` scan is the links themselves.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum GroupEvent {
    /// `<X> joins your group.`
    Joined(Member),
    /// `<X> leaves your group.`
    Left(Member),
    /// `You add <X> to your group.`
    Added(Member),
    /// `You remove <X> from the group.`
    Removed(Member),
    /// `But <X> is already a member of your group!`
    ///
    /// Lich treats this as a `push` (`group.rb:642`), and so does a consumer
    /// here: the game is stating membership, which is worth believing even
    /// though the command failed.
    AlreadyMember(Member),
    /// `<X> designates you as the new leader of the group.`
    LeaderIsYou(Member),
    /// `<X> designates <Y> as the new leader of the group.`
    LeaderChanged {
        /// Who gave it away.
        from: Member,
        /// Who holds it now.
        to: Member,
    },
    /// `You designate <X> as the new leader of the group.`
    GaveLeadership(Member),
    /// `<X> adds you to <X's> group.`
    AddedToGroup(Member),
    /// `You join <X>.`
    JoinedGroup(Member),
    /// `<X> adds <Y> to <X's> group.`
    LeaderAdded {
        /// The leader.
        leader: Member,
        /// Who was added.
        member: Member,
    },
    /// `<X> removes <Y> from the group.`
    LeaderRemoved {
        /// The leader.
        leader: Member,
        /// Who was removed.
        member: Member,
    },
    /// `You disband your group.`
    Disbanded,
}

/// Read a line as a group event, or `None` if it is not one.
///
/// The text is matched with the links **removed**, so a pattern here is the
/// prose the game wraps around them: `" joins your group."` rather than
/// `"<a exist=...>Name</a> joins your group."`. That is what makes these
/// patterns short, and it is only possible because the parser kept the links.
#[must_use]
pub fn classify(line: &ChunkLine) -> Option<GroupEvent> {
    let members: Vec<Member> = line
        .links()
        .filter_map(|link| match &link.kind {
            LinkKind::Exist { id, noun } => Some(Member {
                id: id.clone(),
                noun: noun.clone(),
                text: link.text.clone(),
            }),
            _ => None,
        })
        .collect();
    let text = line.text();
    let trimmed = text.trim();

    // No participants: the only event that needs none.
    if trimmed.starts_with("You disband your group") {
        return Some(GroupEvent::Disbanded);
    }
    let first = members.first()?;

    // One participant, the game speaking about them.
    if trimmed.ends_with(" joins your group.") {
        return Some(GroupEvent::Joined(first.clone()));
    }
    if trimmed.ends_with(" leaves your group.") {
        return Some(GroupEvent::Left(first.clone()));
    }
    if trimmed.starts_with("You add ") && trimmed.ends_with(" to your group.") {
        return Some(GroupEvent::Added(first.clone()));
    }
    if trimmed.starts_with("You remove ") && trimmed.ends_with(" from the group.") {
        return Some(GroupEvent::Removed(first.clone()));
    }
    if trimmed.starts_with("But ") && trimmed.ends_with(" is already a member of your group!") {
        return Some(GroupEvent::AlreadyMember(first.clone()));
    }
    if trimmed.starts_with("You join ") && trimmed.ends_with('.') && members.len() == 1 {
        return Some(GroupEvent::JoinedGroup(first.clone()));
    }

    // Leadership. `designates you` and `designates <Y>` differ by participant
    // count, because "you" is not a link.
    if trimmed.ends_with(" as the new leader of the group.") {
        if trimmed.starts_with("You designate ") {
            return Some(GroupEvent::GaveLeadership(first.clone()));
        }
        return match members.len() {
            1 => Some(GroupEvent::LeaderIsYou(first.clone())),
            _ => Some(GroupEvent::LeaderChanged {
                from: first.clone(),
                to: members[1].clone(),
            }),
        };
    }

    // Adding. **The possessive pronoun is itself a link**, so
    // `<X> adds you to <his> group.` and `<X> adds <Y> to <his> group.` have
    // two and three participants -- the count is what tells them apart, not
    // the prose, which is identical either side of the middle name.
    if trimmed.ends_with(" group.") && trimmed.contains(" adds ") {
        return match members.len() {
            0 | 1 => None,
            2 => Some(GroupEvent::AddedToGroup(first.clone())),
            _ => Some(GroupEvent::LeaderAdded {
                leader: first.clone(),
                member: members[1].clone(),
            }),
        };
    }
    if trimmed.ends_with(" from the group.") && trimmed.contains(" removes ") {
        return Some(GroupEvent::LeaderRemoved {
            leader: first.clone(),
            member: members.get(1)?.clone(),
        });
    }

    None
}

/// Who is grouped with you, by `exist` id.
///
/// **Not a list of names.** Two characters can display the same text, and the
/// possessive pronoun in `adds you to <his> group` is a link carrying the
/// leader's id -- so the id is the identity.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Group {
    members: Vec<Member>,
    leader: Option<Member>,
}

impl Group {
    /// Apply one event.
    ///
    /// Returns whether anything changed.
    pub fn apply(&mut self, event: &GroupEvent) -> bool {
        let before = self.clone();
        match event {
            GroupEvent::Joined(m)
            | GroupEvent::Added(m)
            | GroupEvent::AlreadyMember(m)
            | GroupEvent::LeaderAdded { member: m, .. } => self.push(m),
            GroupEvent::Left(m)
            | GroupEvent::Removed(m)
            | GroupEvent::LeaderRemoved { member: m, .. } => {
                self.members.retain(|held| held.id != m.id);
            }
            // Joining someone's group replaces whatever was held: you are in
            // THEIR group now, and its other members are unknown until the
            // game names them.
            GroupEvent::AddedToGroup(leader) | GroupEvent::JoinedGroup(leader) => {
                self.members.clear();
                self.push(leader);
                self.leader = Some(leader.clone());
            }
            GroupEvent::LeaderChanged { to, .. } => self.leader = Some(to.clone()),
            GroupEvent::GaveLeadership(m) => self.leader = Some(m.clone()),
            // The game named YOU as leader, and "you" is not a link -- so the
            // leader becomes unknown rather than being set to the person who
            // gave it away. A consumer asking "who leads" gets `None`, which
            // is honest; asking "is it me" is a different question this model
            // cannot answer without knowing its own id.
            GroupEvent::LeaderIsYou(_) => self.leader = None,
            GroupEvent::Disbanded => {
                self.members.clear();
                self.leader = None;
            }
        }
        before != *self
    }

    /// Add, without duplicating an id already held.
    fn push(&mut self, member: &Member) {
        if !self.members.iter().any(|held| held.id == member.id) {
            self.members.push(member.clone());
        }
    }

    /// Everyone in the group, in the order the game named them.
    #[must_use]
    pub fn members(&self) -> &[Member] {
        &self.members
    }

    /// The leader, if the game has named one.
    #[must_use]
    pub fn leader(&self) -> Option<&Member> {
        self.leader.as_ref()
    }

    /// Whether this id is in the group.
    #[must_use]
    pub fn contains(&self, id: &str) -> bool {
        self.members.iter().any(|held| held.id == id)
    }

    /// Whether the group is empty.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.members.is_empty()
    }
}
