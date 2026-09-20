//! Is this room mine? Room claiming for hunting groups.
//!
//! Ports the rule in `lib/gemstone/claim.rb` (110 lines). The question it
//! answers is a hunting one: **is anyone in this room who is not with me?** If
//! not, the room is mine to hunt; if so, someone else got here first and a
//! courteous hunter moves on.
//!
//! # The rule is four lines; the rest of `claim.rb` is Ruby plumbing
//!
//! `parser_handle` (`claim.rb:87-107`) is the whole of it:
//!
//! ```ruby
//! @others = pcs - self.clustered - self.members
//! @last_room = nav_rm
//! unless @others.empty?
//!   @mine = false
//!   return
//! end
//! @mine = true
//! ```
//!
//! Everything around it -- a `Mutex` held from `<nav>` to `<compass>`, an
//! `ensure` that unlocks it, a `rescue StandardError` around the subtraction, a
//! `Terminal::Table` debug dump -- exists because `claim.rb` runs inside a
//! streaming XML callback that can be re-entered and can raise. Cena's parser
//! hands a consumer a finished [`Room`], so there is no window to lock and
//! nothing partway through to guard.
//!
//! **The three-line `rescue` is the tell.** A subtraction of two arrays cannot
//! raise; what can is `Group.members` (below), which sends a command. Ruby had
//! to defend the rule against its own dependency. Here the dependency is a
//! parameter.
//!
//! # Why group membership is an argument and not a lookup
//!
//! `Claim.members` calls `Group.members`, which calls `maybe_check`, which
//! calls **`Group.check`** (`group.rb:76`, `:163-165`) -- and `Group.check`
//! sends `group` to the game and waits for the reply. A classifier in
//! `cena-model` sends nothing (`plan/12` §3a), so it cannot ask, and a model
//! that silently answered "no group" when it simply had not asked would report
//! a room contested by the character's own grouped hunting partner.
//!
//! So [`claim`] takes the roster it needs. A caller that knows the group passes
//! it; a caller that does not gets the answer for an ungrouped character, which
//! is the right answer for an ungrouped character and a *stated assumption* for
//! anyone else rather than a silent one.
//!
//! **`Claim.clustered` is not ported at all.** It reads `Cluster.connected`
//! guarded by `defined?(Cluster)`, and there is no `Cluster` anywhere in Lich:
//!
//! ```sh
//! find reference/lich-5/lib -iname '*cluster*'    # no results
//! ```
//!
//! It is an optional third-party script -- a way of treating several characters
//! run by one player as one party. A caller that wants it folds those names
//! into the roster it passes, which is what the subtraction did anyway.
//!
//! # A hidden player counts, and has no name
//!
//! `xmlparser.rb:1187-1188` watches room text for `obvious signs of someone
//! hiding` and pushes the symbol `:hidden` into the arrival list -- a member of
//! the occupant list that is not a name, because there is no name to know.
//!
//! This matters and is easy to miss: a hidden player is **not** in `room
//! players`. The sign is the last item of the room's **`You also see ...`**
//! list -- `room objs` -- alongside the disks and the fallen branches:
//!
//! ```text
//! You also see the faenor Demandred disk inlaid with intersecting bands of
//! jet and obvious signs of someone hiding.
//! ```
//!
//! A claim check that read only `room players` would call that room empty,
//! which is exactly the mistake the feature exists to prevent.
//!
//! **MEASURED, and it is always the same place.** Across the reference logs:
//!
//! ```sh
//! # 477 lines carry the phrase; 477 of them are in a `You also see` list,
//! # and 0 are anywhere else.
//! grep -rho 'obvious signs of someone hiding' reference/wiki_clean/ | wc -l
//! ```
//!
//! Those logs are tag-stripped, so they establish the *sentence* rather than
//! the component directly; the component follows from the sentence, because
//! `You also see` is what `room objs` carries. Confirmed by the author
//! (2026-09-20), who also corrected an intermediate draft of this module that
//! had moved the read to `room desc`.
//!
//! Lich reads it from neither component: `xmlparser.rb:1187` sits in the `else`
//! of the component dispatch, matching text outside both, gated by a
//! `@check_obvious_hiding` flag held open from `<nav>` until the room finishes.
//! That is a third answer again, and it is the one shape Cena cannot copy --
//! there is no open window in a finished [`Room`].
//!
//! [`Occupants::from_room`] reads `room objs`, and [`Occupants::hidden`] is the
//! `:hidden` sentinel with its namelessness in the type rather than in a
//! convention.

use crate::state::room::Room;

/// The phrase that reveals an unnamed player.
///
/// `xmlparser.rb:1188`. Matched as a substring rather than compared whole: it
/// is the last clause of the `You also see ...` list, not a line of its own.
const HIDING: &str = "obvious signs of someone hiding";

/// Who is in the room, for the purpose of claiming it.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Occupants {
    /// Named players, from `room players`.
    pub named: Vec<String>,
    /// Whether the room shows signs of a hidden player.
    ///
    /// Lich's `:hidden` sentinel (`xmlparser.rb:1439`), as a `bool` rather than
    /// a nameless entry in [`Self::named`]: there is at most one such sign
    /// however many people are hiding, and it can never be subtracted against a
    /// group roster because it has no name to match.
    pub hidden: bool,
}

impl Occupants {
    /// Read the occupants of a room.
    ///
    /// Named players come from `room players`; the hidden sign comes from
    /// `room objs`, the `You also see ...` list, which is where the game puts
    /// it -- measured at 477 of 477 occurrences.
    #[must_use]
    pub fn from_room(room: &Room) -> Self {
        let named = room
            .players
            .iter()
            .map(|player| player.noun.clone())
            .collect();
        // The sign is a clause of the description, not a `<player>` tag, so the
        // typed roster cannot see it however carefully it is read.
        let hidden = room
            .component("room objs")
            .is_some_and(|runs| runs.plain().contains(HIDING));
        Self { named, hidden }
    }

    /// Is the room empty of players?
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.named.is_empty() && !self.hidden
    }
}

/// Whether a room is the character's to hunt.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Claim {
    /// Nobody else is here.
    Mine,
    /// Someone else is here. Carries who, so a caller can say why.
    Contested {
        /// Named occupants who are not with the character.
        others: Vec<String>,
        /// Whether an unnamed hidden player is also present.
        ///
        /// Separate from `others` because a hidden player cannot be named, and
        /// a caller reporting "contested by Foo" must not be able to print an
        /// empty name.
        hidden: bool,
    },
    /// **The room has not said.**
    ///
    /// `room players` has never arrived for this room, so "no players" and "not
    /// told yet" are indistinguishable. Lich has no such state -- `@mine`
    /// starts `false` and its `parser_handle` is only ever called from inside a
    /// room arrival, so the question is never asked before the answer exists.
    /// Here a caller may hold a `Room` at any time, and [`Room::saw_players`]
    /// exists precisely to keep this separable (`plan/12` §5.2).
    Unknown,
}

impl Claim {
    /// Is the room mine?
    ///
    /// **`Unknown` is not mine**, deliberately: a hunter that treats "I have
    /// not been told" as "nobody is here" walks into an occupied room.
    #[must_use]
    pub const fn is_mine(&self) -> bool {
        matches!(self, Self::Mine)
    }
}

/// Claim a room, given who is with the character.
///
/// Ports `parser_handle`'s subtraction (`claim.rb:89-96`): the room is mine
/// when every occupant is a member of my party. `with_me` is the combined
/// group and cluster roster; pass an empty slice for an ungrouped character.
///
/// Comparison is case-insensitive, because the group roster and the room
/// roster are two different wire sources for the same names.
///
/// ```
/// use cena_model::state::claim::{Claim, Occupants, claim};
///
/// let alone = Occupants::default();
/// assert_eq!(claim(&alone, &[]), Claim::Mine);
///
/// let with_a_stranger = Occupants { named: vec!["Grhim".into()], hidden: false };
/// assert!(!claim(&with_a_stranger, &[]).is_mine());
///
/// // ...but not if they are in my group.
/// assert_eq!(claim(&with_a_stranger, &["Grhim".into()]), Claim::Mine);
/// ```
#[must_use]
pub fn claim(occupants: &Occupants, with_me: &[String]) -> Claim {
    let others: Vec<String> = occupants
        .named
        .iter()
        .filter(|name| {
            !with_me
                .iter()
                .any(|mine| mine.eq_ignore_ascii_case(name.as_str()))
        })
        .cloned()
        .collect();

    // **A hidden player is never subtracted.** They have no name, so they
    // cannot be matched against the roster -- and Lich does the same, pushing
    // `:hidden` into the list it subtracts from with nothing that can remove
    // it. A grouped partner who hides therefore contests their own room; that
    // is the conservative answer and the one the game's information allows.
    if others.is_empty() && !occupants.hidden {
        Claim::Mine
    } else {
        Claim::Contested {
            others,
            hidden: occupants.hidden,
        }
    }
}

/// Claim the room the character is standing in.
///
/// Returns [`Claim::Unknown`] when the room has never reported its players,
/// rather than reading an empty roster as an empty room.
#[must_use]
pub fn claim_room(room: &Room, with_me: &[String]) -> Claim {
    if !room.saw_players() {
        return Claim::Unknown;
    }
    claim(&Occupants::from_room(room), with_me)
}
