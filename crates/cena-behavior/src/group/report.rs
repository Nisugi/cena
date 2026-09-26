//! What each member tells the leader, what the leader tells them back, and
//! the role a member reads off the game's group.
//!
//! # A report is the answers to every question, at once
//!
//! bigshot's leader asks each follower a question at a time, as a remote
//! call that stalls it when the follower is slow (`ready_to_rest?`,
//! `ready_to_hunt?`, `rt?`, `encumbrance?`; `bigshot.lic:1185-1249`).
//! eohunter turned the calls around: each follower pushes one `Report` a
//! tick, the answers to all of them (`scripts/eohunter/group.rb:159`, in
//! eohunter's tree; its lines below are that file's). Hydra keeps
//! eohunter's shape, in one process: a member's [`Report`] is what its own
//! hunt knows of itself, and the leader reads the latest (`plan/39` §3).
//!
//! **A field no rule reads is not here** (`plan/05` §−1). `plan/39` §5
//! lists more for the board: hidden and sneaky (the movement barrier),
//! looting (the leader's wait before it leaves a room), the rest a follower
//! has prepared for (the rest-prep barrier), and the leader's phase, target,
//! looter and rooms. Each arrives with the Stage 3 rule that reads it.

use cena_map::RoomId;
use cena_session::State;
use cena_session::group::{Group, Leader};

use crate::hunt::said::Why;

/// A member's part in the group, read off the game's group and never
/// chosen (`plan/39` §8, question 2: *"passing group lead is like passing
/// the leader"*).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Role {
    /// It leads a group with members in it.
    Lead,
    /// Someone else leads the group it is in.
    Follow,
    /// It leads nobody and follows nobody: the solo hunt, unchanged.
    Solo,
}

/// The role the game's group gives this character; `None` while the game
/// has not said who leads.
///
/// bigshot's roles are three readings of one variable (`solo?`,
/// `leading?`, `following?`, `bigshot.lic:7199-7212`); these are three
/// readings of [`Group::leader`]. **Solo is leading nobody**, as bigshot's
/// `solo?` is a leader whose group is empty, and as Lich sets `:self` when
/// a group empties (`reference/lich-5/lib/gemstone/group.rb:603-605`).
///
/// [`Leader::Unknown`] is **not** solo and never lead. It is a new session
/// before the burst's `IconJOINED` has said, or one a reconnect cleared
/// with the character still grouped; either way only the `group` command
/// says who leads (Lich's `Group.check`,
/// `reference/lich-5/lib/gemstone/group.rb:152-157`). Read as solo,
/// a follower would hunt off alone after every reconnect. The caller sends
/// `group` and holds.
#[must_use]
pub fn role(group: &Group) -> Option<Role> {
    match group.leader() {
        Leader::Unknown => None,
        Leader::Other(_) => Some(Role::Follow),
        Leader::You if group.is_empty() => Some(Role::Solo),
        Leader::You => Some(Role::Lead),
    }
}

/// What keeps a member from acting or moving (`plan/39` §8, question 7's
/// table). The report carries the one that stops it most: dead, then
/// stuck, then down, then roundtime.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Hindrance {
    /// In hard or cast roundtime (eohunter's `rt`, `group.rb:198`).
    Roundtime,
    /// Sitting, kneeling, lying or prone: it can stand, or be pulled up
    /// (`bigshot.lic:7501`, `sitting|^lying|prone`).
    Down,
    /// Unable to move: stunned, webbed, bound, asleep, frozen, immobilized,
    /// held in place, horrified or staggered -- `group_member_stunned?`'s
    /// words (`bigshot.lic:6717-6730`) and question 7's *"bound"*.
    Stuck,
    /// Dead.
    Dead,
}

/// One member's report: what the group's rules read of it.
///
/// Every member publishes one, the leader included; the leader's is the
/// group's reference for the room.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Report {
    /// The character's name, as the host table knows it. The rules name
    /// members by it, as bigshot's do (`get_names`, `bigshot.lic:1001-1002`).
    pub name: String,
    /// Its session's lifecycle: [`State::Ready`], or on its way back
    /// (`Reconnecting` and the steps after it), or given up (`Closed`)
    /// (`plan/39` §8a).
    pub link: State,
    /// Its room, by the map's number; `None` when the map cannot place it.
    pub room: Option<RoomId>,
    /// Why it wants the group to rest: its own hunt's reason, or `None`
    /// (`ready_to_rest?`, `bigshot.lic:9007-9041`).
    pub rest: Option<Why>,
    /// Why it is not ready to hunt again: its own rest's words, or `None`
    /// when it is (`ready_to_hunt?`, `bigshot.lic:8947-8974`).
    pub unready: Option<&'static str>,
    /// What keeps it from acting or moving, if anything does.
    pub hindrance: Option<Hindrance>,
    /// Whether it is in the leader's game group.
    pub grouped: bool,
    /// Its health, percent; `None` until the game has said.
    pub health: Option<u32>,
    /// Encumbrance percent still free under its own `rest.encumbered`
    /// (`group_encumbrance`, `bigshot.lic:1240-1249`, from `encumbrance?`,
    /// `:8798-8801`); `None` when either is unknown.
    pub headroom: Option<i32>,
}

impl Report {
    /// Connected, and not dead: a member who can take a part the group
    /// hands out (the loot, the lead).
    #[must_use]
    pub fn present(&self) -> bool {
        self.link == State::Ready && self.hindrance != Some(Hindrance::Dead)
    }
}

/// What the leader publishes for its followers, beside its own report.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Leading {
    /// The number of the rest under way, or of the last one.
    pub rest: u32,
    /// The rest whose own prep the leader has finished, if any.
    ///
    /// **A number, not a flag.** bigshot's `rest_prep_done?` is a flag that
    /// stays true from the last rest until the next is prepared, and the
    /// leader reads it straight after asking for the new prep
    /// (`bigshot.lic:7541-7560`, `plan/39` §0e). Naming the rest cannot be
    /// read for the wrong one.
    pub prepared: Option<u32>,
}
