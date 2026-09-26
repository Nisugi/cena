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
//!
//! # Who leads is three answers, not two
//!
//! Lich's `@@leader` is `nil` until something is said, `:self` when you lead,
//! and a `GameObj` when someone else does (`group.rb:21`, `:266-286`). This
//! model spelled both `nil` and `:self` as `None`, so "am I the leader?" had
//! no answer (`plan/39` §4, gap 2), and every group role is read off it
//! (`plan/39` §8, questions 2 and 8). [`Leader`] keeps the three apart.
//!
//! "You" is prose, not a link, in every line Lich reads (`designates you`,
//! `You are leading`), so the classifier needs no id to find it. The
//! character's own id ([`Character::exist_id`]) is for a line that names you
//! by a link anyway: [`Group::apply`] reads that link as you, never as a
//! member.
//!
//! [`Character::exist_id`]: crate::state::Character::exist_id

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
    /// `You add <X> to your group.`, and `You grab <X's> hand.` in each
    /// demeanor (`HOLD_*_FIRST`), which Lich answers alike (`group.rb:642`,
    /// `:646-647`).
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
    /// `<X> adds you to <X's> group.`, and `<X> grabs your hand.` in each
    /// demeanor (`HOLD_*_SECOND`, `group.rb:481-492`), which Lich answers
    /// alike (`:628-631`, `:648-651`): X leads you now.
    AddedToGroup(Member),
    /// `You join <X>.`
    JoinedGroup(Member),
    /// `<X> adds <Y> to <X's> group.`, and `<X> grabs <Y's> hand.` in each
    /// demeanor (`HOLD_*_THIRD`, `group.rb:494-505`), which Lich answers alike
    /// (`:636-638`, `:652-654`).
    LeaderAdded {
        /// The leader.
        leader: Member,
        /// Who was added.
        member: Member,
    },
    /// `<X> joins <Y's> group.` -- `OTHER_JOINED_GROUP` (`group.rb:509`),
    /// someone joining a group you do not lead. The joiner is named first,
    /// the other way round from `adds`.
    JoinedOther {
        /// Who joined.
        member: Member,
        /// Whose group it is.
        leader: Member,
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
    /// `You have no group to disband.` -- `NO_GROUP_TO_DISBAND`
    /// (`group.rb:516`): you lead no group, which a follower hears too.
    NoGroupToDisband,
    /// `Your group status is currently open.` (or `closed`) -- `STATUS`
    /// (`group.rb:525`).
    Status(GroupStatus),
    /// `<X>'s group status is closed`: the game refusing `group #<id>` or
    /// `join #<id>` (`group.rb:307-309`, `:349-351`). Lich's observer has no
    /// pattern for it; `Group.add` and `Group.join` read it as the answer to
    /// their own command.
    ///
    /// UNVERIFIED that the game sends the name as a link: Lich matches this
    /// one on stripped text. Its sibling `<X> joins <Y's> group.` links the
    /// possessive name (`group.rb:508`), so it is read as a link here, with
    /// or without the `'s` inside it, and a line with no link is not read.
    Refused(Member),
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
const YOU_HOLD: [(&str, &str); 4] = [
    ("You grab ", " hand."),
    ("You reach out and hold ", " hand."),
    ("You gently take hold of ", " hand."),
    ("You clasp ", " hand tenderly."),
];

/// `HOLD_*_SECOND` (`group.rb:481-492`): someone takes YOUR hand, as the
/// prose after their name. Not anchored at the start, as Lich's are not.
const HOLDS_YOURS: [&str; 4] = [
    " grabs your hand.",
    " reaches out and holds your hand.",
    " gently takes hold of your hand.",
    " clasps your hand tenderly.",
];

/// `HOLD_*_THIRD` (`group.rb:494-505`): someone takes a third person's hand,
/// as the prose between the two names and after the second.
const HOLDS_ANOTHERS: [(&str, &str); 4] = [
    (" grabs ", " hand."),
    (" reaches out and holds ", " hand."),
    (" gently takes hold of ", " hand."),
    (" clasps ", " hand tenderly."),
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

    if let Some(event) = unlinked(trimmed) {
        return Some(event);
    }
    // `^You are (?:leading|grouped with) (.*)` (`group.rb:521`). Before the
    // `first` guard, because the members ARE the event, however many.
    for (opener, leading) in [("You are leading ", true), ("You are grouped with ", false)] {
        if trimmed.starts_with(opener) {
            return Some(GroupEvent::Listed { leading, members });
        }
    }
    let first = members.first()?;
    about_one(trimmed, first, members.len())
        .or_else(|| hands(trimmed, &members))
        .or_else(|| about_two(trimmed, &members))
}

/// The lines that name nobody, each anchored at the start as Lich anchors it
/// (after `consume`'s `line.strip`, `group.rb:587`): a player SAYING one puts
/// their own name first.
fn unlinked(trimmed: &str) -> Option<GroupEvent> {
    if trimmed.starts_with("You disband your group") {
        return Some(GroupEvent::Disbanded);
    }
    // `^You are not currently in a group` (`group.rb:512`).
    if trimmed.starts_with("You are not currently in a group") {
        return Some(GroupEvent::NotInGroup);
    }
    // `^You have no group to disband\.` (`group.rb:516`).
    if trimmed.starts_with("You have no group to disband.") {
        return Some(GroupEvent::NoGroupToDisband);
    }
    // `^Your group status is currently (?<status>open|closed)\.`
    // (`group.rb:525`).
    let status = trimmed.strip_prefix("Your group status is currently ")?;
    if status.starts_with("open.") {
        Some(GroupEvent::Status(GroupStatus::Open))
    } else if status.starts_with("closed.") {
        Some(GroupEvent::Status(GroupStatus::Closed))
    } else {
        None
    }
}

/// One participant, the game speaking about them.
fn about_one(trimmed: &str, first: &Member, count: usize) -> Option<GroupEvent> {
    let event = if trimmed.ends_with(" joins your group.") {
        GroupEvent::Joined(first.clone())
    } else if trimmed.ends_with(" leaves your group.") {
        GroupEvent::Left(first.clone())
    } else if trimmed.starts_with("You add ") && trimmed.ends_with(" to your group.") {
        GroupEvent::Added(first.clone())
    } else if trimmed.starts_with("You remove ") && trimmed.ends_with(" from the group.") {
        GroupEvent::Removed(first.clone())
    } else if trimmed.starts_with("But ")
        && trimmed.ends_with(" is already a member of your group!")
    {
        GroupEvent::AlreadyMember(first.clone())
    } else if count == 1 && trimmed.starts_with("You join ") && trimmed.ends_with('.') {
        GroupEvent::JoinedGroup(first.clone())
    } else if refused(trimmed, first) {
        GroupEvent::Refused(named(first))
    } else {
        return None;
    };
    Some(event)
}

/// Whether the line is `<X>'s group status is closed`, the name first, as
/// `Group.join` anchors it (`group.rb:351`): a player saying it puts their
/// own name first, and theirs is not the link that follows.
fn refused(trimmed: &str, first: &Member) -> bool {
    trimmed
        .strip_prefix(first.text.as_str())
        .map(|rest| rest.strip_prefix("'s").unwrap_or(rest))
        .is_some_and(|rest| rest.starts_with(" group status is closed"))
}

/// Someone's hand taken: by you, from you, or between two others. Lich
/// answers each as the adding line it resembles (`group.rb:646-654`).
fn hands(trimmed: &str, members: &[Member]) -> Option<GroupEvent> {
    match members {
        [held]
            if YOU_HOLD
                .iter()
                .any(|(start, end)| trimmed.starts_with(start) && trimmed.ends_with(end)) =>
        {
            Some(GroupEvent::Added(named(held)))
        }
        [taker]
            if HOLDS_YOURS
                .iter()
                .any(|end| trimmed.ends_with(&format!("{}{end}", taker.text))) =>
        {
            Some(GroupEvent::AddedToGroup(taker.clone()))
        }
        // Anchored at both ends, as Lich's `^...$` is.
        [taker, held]
            if HOLDS_ANOTHERS.iter().any(|(verb, end)| {
                trimmed == format!("{}{verb}{}{end}", taker.text, held.text)
            }) =>
        {
            Some(GroupEvent::LeaderAdded {
                leader: taker.clone(),
                member: named(held),
            })
        }
        _ => None,
    }
}

/// Leadership, and a leader adding, removing or being joined.
fn about_two(trimmed: &str, members: &[Member]) -> Option<GroupEvent> {
    let first = members.first()?;
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
    // `^<X> joins <Y's> group.$` (`group.rb:509`), whole.
    if let [member, leader] = members
        && trimmed == format!("{} joins {} group.", member.text, leader.text)
    {
        return Some(GroupEvent::JoinedOther {
            member: member.clone(),
            leader: named(leader),
        });
    }
    None
}

/// A member named by a possessive link -- `Dicate's` -- with the name as its
/// text, since a member's name is not possessive.
fn named(member: &Member) -> Member {
    let mut named = member.clone();
    if let Some(name) = named.text.strip_suffix("'s") {
        named.text = name.to_owned();
    }
    named
}

/// Who leads the group, as far as the game has said.
///
/// Lich's `@@leader`: `nil`, `:self` or a `GameObj` (`group.rb:21`,
/// `:266-286`).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub enum Leader {
    /// Nobody has said: a new session, or one a reconnect invalidated.
    #[default]
    Unknown,
    /// You. Also what a group of one is: Lich sets `:self` when the group
    /// empties (`group.rb:603-605`, `:617-619`), so you leading nobody is
    /// being in no group.
    You,
    /// Someone else, by the link the game sent.
    Other(Member),
}

impl Leader {
    /// `member` as the leader: [`Leader::You`] when the link is you.
    fn of(member: &Member, me: Option<&str>) -> Self {
        if me == Some(member.id.as_str()) {
            Leader::You
        } else {
            Leader::Other(member.clone())
        }
    }
}

/// Whether your group takes new members, as `group` last reported it.
///
/// [`Group::status`] is `None` until then. Lich starts at `:closed`
/// (`group.rb:23`), which is a default and not something the game said
/// (`plan/12` §5.2).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GroupStatus {
    /// Others may join.
    Open,
    /// Others may not.
    Closed,
}

/// Who is grouped with you, by `exist` id.
///
/// **Not a list of names.** Two characters can display the same text, and the
/// possessive pronoun in `adds you to <his> group` is a link carrying the
/// leader's id -- so the id is the identity.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Group {
    members: Vec<Member>,
    leader: Leader,
    status: Option<GroupStatus>,
    checked: bool,
}

impl Group {
    /// Apply one event.
    ///
    /// `me` is the character's own `exist` id
    /// ([`Character::exist_id`](crate::state::Character::exist_id)), once the
    /// login burst has said it: a link that is you becomes [`Leader::You`],
    /// and is never a member of your own group.
    ///
    /// Returns whether anything changed.
    pub fn apply(&mut self, event: &GroupEvent, me: Option<&str>) -> bool {
        let before = self.clone();
        match event {
            GroupEvent::Joined(m) | GroupEvent::Added(m) | GroupEvent::AlreadyMember(m) => {
                self.push(m, me);
            }
            // **Only if the leader is in OUR group** (`group.rb:636-638`,
            // `Group.push(added) if Group.include?(leader)`). The line is
            // broadcast to everyone in the room, so `<X> adds <Y> to his
            // group` is as often news about someone else's group as ours --
            // and pushing unconditionally put strangers on our roster. `<Y>
            // joins <X's> group` and `<X> grabs <Y's> hand` take the same
            // guard (`:652-657`).
            GroupEvent::LeaderAdded { leader, member }
            | GroupEvent::JoinedOther { member, leader } => {
                if self.contains(&leader.id) {
                    self.push(member, me);
                }
            }
            // The same guard, on removal (`group.rb:639-641`): a leader of
            // some other group removing someone says nothing about ours.
            GroupEvent::LeaderRemoved { leader, member } => {
                if self.contains(&leader.id) {
                    self.members.retain(|held| held.id != member.id);
                }
            }
            // A refusal deletes, as `Group.add` does on it (`group.rb:317-319`):
            // whoever the game would not group you with is not in your group.
            GroupEvent::Left(m) | GroupEvent::Removed(m) | GroupEvent::Refused(m) => {
                self.members.retain(|held| held.id != m.id);
            }
            // Joining someone's group replaces whatever was held: you are in
            // THEIR group now, and its other members are unknown until the
            // game names them -- which is why Lich stops trusting its roster
            // here (`checked = false`, `group.rb:629`, `:649`).
            GroupEvent::AddedToGroup(leader) | GroupEvent::JoinedGroup(leader) => {
                self.members.clear();
                self.push(leader, me);
                self.leader = Leader::of(leader, me);
                self.checked = false;
            }
            // **Only if either is in OUR group**, the guard Lich puts on the
            // push (`group.rb:632-635`) and here on the leader too: Lich sets
            // the leader whatever the two are, so a swap in a stranger's
            // group, if the room hears it as it hears `adds`, would make a
            // stranger our leader -- and every group role is read off it
            // (`plan/39` §8).
            GroupEvent::LeaderChanged { from, to } => {
                if self.contains(&from.id) || self.contains(&to.id) {
                    self.push(from, me);
                    self.push(to, me);
                    self.leader = Leader::of(to, me);
                }
            }
            // `GAVE_LEADER_AWAY` (`group.rb:625-627`): the new leader is a
            // member, as they must be to take it.
            GroupEvent::GaveLeadership(m) => {
                self.push(m, me);
                self.leader = Leader::of(m, me);
            }
            // `GIVEN_LEADERSHIP` (`group.rb:598-600`): "you" is prose, so the
            // line itself says who, and no id is needed.
            GroupEvent::LeaderIsYou(_) => self.leader = Leader::You,
            GroupEvent::Disbanded | GroupEvent::NotInGroup => {
                self.emptied();
            }
            // You lead no group -- but a follower hears it too, so it cannot
            // clear the members; it only puts the roster in doubt
            // (`group.rb:514-516`, `:620-621`).
            GroupEvent::NoGroupToDisband => self.checked = false,
            // The `group` command's last line: `Group.check` waits for it
            // (`group.rb:152-157`, `:622-624`), so the roster before it is the
            // game's own answer.
            GroupEvent::Status(status) => {
                self.status = Some(*status);
                self.checked = true;
            }
            // `Group.refresh(*people)` (`group.rb:644-645`): the whole list, as
            // named. `You are leading` makes the leader you; `grouped with`
            // names the leader first (`:610-614`, `Group.leader =
            // people.first`), and a roster naming nobody names no leader.
            GroupEvent::Listed { leading, members } => {
                self.members.clear();
                for member in members {
                    self.push(member, me);
                }
                self.leader = match (leading, members.first()) {
                    (true, _) => Leader::You,
                    (false, Some(first)) => Leader::of(first, me),
                    (false, None) => Leader::Unknown,
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
        let changed = !self.members.is_empty() || self.leader != Leader::You;
        self.members.clear();
        self.leader = Leader::You;
        changed
    }

    /// Add, without duplicating an id already held -- and never you, who are
    /// not grouped with yourself.
    fn push(&mut self, member: &Member, me: Option<&str>) {
        if me != Some(member.id.as_str()) && !self.contains(&member.id) {
            self.members.push(member.clone());
        }
    }

    /// Everyone in the group, in the order the game named them. Not you.
    #[must_use]
    pub fn members(&self) -> &[Member] {
        &self.members
    }

    /// Who leads: unknown, you, or someone else. **"Am I the leader?"** is
    /// `*leader() == Leader::You`, and [`Leader::Unknown`] is not "no".
    #[must_use]
    pub fn leader(&self) -> &Leader {
        &self.leader
    }

    /// Whether your group is open, or `None` until `group` has said.
    #[must_use]
    pub fn status(&self) -> Option<GroupStatus> {
        self.status
    }

    /// Whether the game's answer to `group` has been read since anything
    /// last put the roster in doubt: Lich's `checked?` (`group.rb:38-40`).
    ///
    /// Set by the answer's last line ([`GroupEvent::Status`]); cleared by
    /// joining or being taken into someone's group, whose other members are
    /// not named, and by `You have no group to disband.`; `false` in a new
    /// session and after a reconnect. Lich answers `false` by sending `group`
    /// before it reads the roster (`maybe_check`, `:163-165`). This model
    /// sends nothing, so a consumer that needs the roster sends it.
    #[must_use]
    pub fn checked(&self) -> bool {
        self.checked
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
