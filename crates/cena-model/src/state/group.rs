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
/// Ported from `group.rb:418-529`'s patterns, minus the two that need no
/// pattern here: `GROUP_EMPTIED` is `<indicator id='IconJOINED' visible='n'/>`,
/// which arrives as a `Frame::StatusIndicator` -- `GameState::apply` stores it
/// AND empties the group on it ([`Group::emptied`]), as Lich's `consume` does
/// (`group.rb:603-605`) -- and the `EXIST` scan is the links themselves.
///
/// > **CORRECTED 2026-09-23.** This said the indicator was "already stored",
/// > as though storing it were the whole of Lich's handling. It is half: Lich
/// > also CLEARS the members on it, and on `NO_GROUP` (`:617-619`), and
/// > replaces them wholesale on the `group` command's `MEMBER` line (`:644-645`,
/// > `Group.refresh`). None of the three was ported, so nothing but a
/// > `disband` or a reconnect ever emptied the list -- a character who was
/// > dropped from a group by someone else kept reporting its members.
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
    /// `You are not currently in a group.` -- `NO_GROUP` (`group.rb:512`),
    /// the `group` command's answer when there is nothing to list. Lich
    /// clears on it exactly as on a disband (`:617-619`).
    NotInGroup,
    /// `You are leading <X>, <Y>.` or `You are grouped with <X>, <Y>.` --
    /// `MEMBER` (`group.rb:521`), the `group` command's roster.
    ///
    /// **A complete list, so it REPLACES the members** (`Group.refresh`,
    /// `:67-69`), rather than adding to them: anyone not named has left.
    Listed {
        /// `You are leading`: the leader is you, so it is not a link.
        leading: bool,
        /// Everyone the line named, in order.
        members: Vec<Member>,
    },
}

/// `HOLD_*_FIRST` (`group.rb:468-479`): you take someone's hand, as the prose
/// either side of their name. Reserved, neutral, friendly, warm.
///
/// **Not ported: the eight `HOLD_*_SECOND` and `_THIRD`.** Someone taking
/// *your* hand also makes them the leader (`group.rb:648-651`), and someone
/// taking a third person's hand is `LeaderAdded` by another route; both want
/// their own captures before they are written.
const YOU_HOLD: [(&str, &str); 4] = [
    ("You grab ", " hand."),
    ("You reach out and hold ", " hand."),
    ("You gently take hold of ", " hand."),
    ("You clasp ", " hand tenderly."),
];

/// Read a line as a group event, or `None` if it is not one.
///
/// The text is matched with the links **removed**, so a pattern here is the
/// prose the game wraps around them: `" joins your group."` rather than
/// `"<a exist=...>Name</a> joins your group."`. That is what makes these
/// patterns short, and it is only possible because the parser kept the links.
#[must_use]
pub fn classify(line: &ChunkLine) -> Option<GroupEvent> {
    classify_text(line, &line.text())
}

/// [`classify`], given the line's text already rendered (the chunk renders
/// each line once and shares it).
pub(crate) fn classify_text(line: &ChunkLine, text: &str) -> Option<GroupEvent> {
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
    let trimmed = text.trim();

    // No participants needed.
    if trimmed.starts_with("You disband your group") {
        return Some(GroupEvent::Disbanded);
    }
    // `^You are not currently in a group` (`group.rb:512`), anchored as Lich
    // anchors it (after `consume`'s `line.strip`, `group.rb:587`): a player
    // SAYING it puts their own name first.
    if trimmed.starts_with("You are not currently in a group") {
        return Some(GroupEvent::NotInGroup);
    }
    // `^You are (?:leading|grouped with) (.*)` (`group.rb:521`). Before the
    // `first` guard, because the members ARE the event, however many.
    for (opener, leading) in [("You are leading ", true), ("You are grouped with ", false)] {
        if trimmed.starts_with(opener) {
            return Some(GroupEvent::Listed { leading, members });
        }
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
    // Taking someone's hand adds them, as `You add` does: Lich answers both
    // with `Group.push` (`group.rb:642`, `:646`). One line in four demeanors.
    if members.len() == 1
        && YOU_HOLD
            .iter()
            .any(|(start, end)| trimmed.starts_with(start) && trimmed.ends_with(end))
    {
        // The link's text is possessive here -- `Dicate's` -- and a member's
        // name is not.
        let mut held = first.clone();
        if let Some(name) = held.text.strip_suffix("'s") {
            held.text = name.to_owned();
        }
        return Some(GroupEvent::Added(held));
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
            GroupEvent::Joined(m) | GroupEvent::Added(m) | GroupEvent::AlreadyMember(m) => {
                self.push(m);
            }
            // **Only if the leader is in OUR group** (`group.rb:636-638`,
            // `Group.push(added) if Group.include?(leader)`). The line is
            // broadcast to everyone in the room, so `<X> adds <Y> to his
            // group` is as often news about someone else's group as ours --
            // and pushing unconditionally put strangers on our roster.
            GroupEvent::LeaderAdded { leader, member } => {
                if self.contains(&leader.id) {
                    self.push(member);
                }
            }
            // The same guard, on removal (`group.rb:639-641`): a leader of
            // some other group removing someone says nothing about ours.
            GroupEvent::LeaderRemoved { leader, member } => {
                if self.contains(&leader.id) {
                    self.members.retain(|held| held.id != member.id);
                }
            }
            GroupEvent::Left(m) | GroupEvent::Removed(m) => {
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
            GroupEvent::Disbanded | GroupEvent::NotInGroup => {
                self.emptied();
            }
            // `Group.refresh(*people)` (`group.rb:644-645`): the whole list, as
            // named. `You are leading` makes the leader you, which this model
            // spells `None` (see `LeaderIsYou`); `grouped with` names the
            // leader first (`:610-614`, `Group.leader = people.first`).
            GroupEvent::Listed { leading, members } => {
                self.members.clear();
                for member in members {
                    self.push(member);
                }
                self.leader = if *leading {
                    None
                } else {
                    members.first().cloned()
                };
            }
        }
        before != *self
    }

    /// The group is gone: no members, and no leader but you.
    ///
    /// `GROUP_EMPTIED` (`<indicator id='IconJOINED' visible='n'/>`,
    /// `group.rb:603-605`) and `NO_GROUP`/`DISBAND` (`:617-619`) all do the
    /// same two things in Lich: `Group.leader = :self` and clear the members.
    /// Returns whether anything changed.
    pub fn emptied(&mut self) -> bool {
        let changed = !self.members.is_empty() || self.leader.is_some();
        self.members.clear();
        self.leader = None;
        changed
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
