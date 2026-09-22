//! Why a move did not happen.
//!
//! Ports the `CAUSES` table and `classify` from `lib/common/move.rb:61-93`
//! (458 lines). **The sending half is not here** — `move.rb` is dominated by
//! send-a-direction/read-the-answer/fix-the-obstacle/retry, which needs the
//! authority token and roundtime and is M6, the same split `stash.rb`,
//! `bank.rb` and `fog.rb` took (`plan/20` §0b). What is model work is the
//! knowledge: *which refusal line means what*.
//!
//! `plan/21-mapdb.md:178` already names this as the thing routing needs —
//! *"the hard part is not the primitives; it is move recovery"*. A router
//! that cannot tell "this exit does not exist" from "I am in combat" either
//! deletes a good exit from the map or retries forever against a wall.
//!
//! # THIS CLASSIFIER IS NOT STATELESS, AND THE CORPUS IS WHY
//!
//! Every other classifier in this crate reads one line and answers. This one
//! **must only be asked about a line that answers an outstanding move**, and
//! that is a measured constraint rather than a design preference.
//!
//! MEASURED over the 208 live Lich XML logs (4,468,850 lines), counting each
//! pattern everywhere against only within six lines of a movement command:
//!
//! | cause | after a move | anywhere |
//! |---|---:|---:|
//! | `Map` | 39 | **5,783** |
//! | `Roundtime` | 95 | **12,918** |
//! | `Engaged` | 0 | 11 |
//! | `Hands` | 0 | 1 |
//!
//! The unscoped hits are not near-misses, they are unrelated prose. `Engaged`
//! matched an **amulet description** — *"allows you to THINK LOCATION
//! {target} while engaged in ESP"* — because its pattern is the bare word
//! `engaged`. `Hands` matched *"You glance down at your empty hands."* Fed
//! every line of a session, this table would report a move failure several
//! thousand times an hour while the character stood still.
//!
//! So [`classify`] is a free function a **consumer calls while a move is
//! outstanding**, exactly as `move.rb` only ever calls it on the line that
//! ended an attempt. The consumer is M6's business; this is the knowledge it
//! will need.
//!
//! # Nine of the fourteen causes are unconfirmed here, and that is expected
//!
//! Only `Map`, `Roundtime` and `Closed` were observed after a move in this
//! corpus. The other nine fired zero times — **not because they are wrong but
//! because these logs are a character standing still**: 386 movement commands
//! in 4.4 million lines, none of them into a locked door, a swim, or a fight.
//! Confirming `Injured` or `Swim` needs logs of a character actually
//! traveling.
//!
//! The author settled it (2026-09-21): *"we can take those 9 as being real.
//! They weren't just added there by a hallucinating llm."* They are upstream
//! Lich's observations of live play, and the failure mode of keeping one is
//! benign — an unmatched line is [`MoveFailure::Unknown`], which a caller
//! already has to handle.
//!
//! # Order is load-bearing
//!
//! First match wins, and the specific causes sit above the broad ones
//! (`move.rb:59-60`). `Map`'s pattern includes `too far away`; `Engaged`'s is
//! the bare word. Reordering changes answers, so `CAUSES` is a slice in
//! source order and a test asserts the shadowing cases.

use std::fmt;

/// Why a move did not happen.
///
/// The fourteen `CAUSES` keys of `move.rb:24-38`, as a closed vocabulary —
/// C21's rule for a set the game defines and a port fixes once.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum MoveFailure {
    /// Too wounded: agony, too injured to climb, a stand that keeps failing
    /// with limb wounds.
    Injured,
    /// A stand that keeps failing while overburdened.
    Encumbered,
    /// In combat and cannot leave.
    Engaged,
    /// Not standing, and could not get up.
    Position,
    /// Must be visible to go that way.
    Hidden,
    /// Needs empty hands.
    Hands,
    /// A door or gate that would not open.
    Closed,
    /// The game does not know this exit from here — a bad `wayto`, or the
    /// character is not where the map thinks.
    ///
    /// **The one a router must act on.** This is the cause that justifies
    /// dropping an exit; every other cause means keep it and try later.
    Map,
    /// An NPC or a rule refused entry: guards, tickets, guild membership.
    Denied,
    /// The climb kept failing — a skill roll, not a wound.
    Climb,
    /// The swim kept failing.
    Swim,
    /// The body being dragged would not come.
    Drag,
    /// Still waiting on roundtime when we gave up.
    Roundtime,
    /// None of the above. **Read the line.**
    Unknown,
}

impl MoveFailure {
    /// Every cause, in the order `move.rb` lists them.
    pub const ALL: [Self; 14] = [
        Self::Injured,
        Self::Encumbered,
        Self::Engaged,
        Self::Position,
        Self::Hidden,
        Self::Hands,
        Self::Closed,
        Self::Map,
        Self::Denied,
        Self::Climb,
        Self::Swim,
        Self::Drag,
        Self::Roundtime,
        Self::Unknown,
    ];

    /// The symbol Lich uses, for logs and for a caller that round-trips.
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Injured => "injured",
            Self::Encumbered => "encumbered",
            Self::Engaged => "engaged",
            Self::Position => "position",
            Self::Hidden => "hidden",
            Self::Hands => "hands",
            Self::Closed => "closed",
            Self::Map => "map",
            Self::Denied => "denied",
            Self::Climb => "climb",
            Self::Swim => "swim",
            Self::Drag => "drag",
            Self::Roundtime => "roundtime",
            Self::Unknown => "unknown",
        }
    }

    /// Read a symbol back.
    #[must_use]
    pub fn parse(text: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|c| c.as_str() == text)
    }

    /// Whether this cause means the exit itself is wrong.
    ///
    /// `move`'s return is tri-state (`move.rb:11-13`): `true` moved, `false`
    /// **the exit is wrong and a caller may drop it from the map**, `nil`
    /// blocked for now so keep it. Only [`Self::Map`] is the false case —
    /// every other cause is a condition that passes.
    ///
    /// This is the distinction `plan/21` needs, and getting it backwards
    /// silently corrupts the map: an exit deleted because the character was
    /// in combat is a route that can never be found again.
    #[must_use]
    pub fn invalidates_the_exit(self) -> bool {
        self == Self::Map
    }
}

impl fmt::Display for MoveFailure {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// The phrases for each cause, **in `move.rb`'s order — first match wins**.
///
/// Substrings rather than regexes: every Ruby pattern here is an alternation
/// of literals, so a `contains` over the alternatives is the same test without
/// a regex engine in this crate. Two patterns needed care and are noted at
/// their entries.
///
/// The order is the one thing that must not drift. `Map` holds `too far away`
/// and `Engaged` holds the bare word `engaged`, so a line can satisfy several
/// and only the first is returned.
const CAUSES: &[(MoveFailure, &[&str])] = &[
    (
        MoveFailure::Injured,
        &["far too much agony", "too injured to be doing"],
    ),
    (MoveFailure::Encumbered, &["overburdened"]),
    (
        MoveFailure::Engaged,
        &[
            "engaged",
            "retreat out of combat",
            "while in combat",
            "next to impossible while in combat",
        ],
    ),
    (
        MoveFailure::Hidden,
        &[
            "remain hidden or invisible",
            "can't be seen",
            "without being seen",
            "no one can see you",
            "can't see you",
        ],
    ),
    (
        MoveFailure::Hands,
        &[
            "hands were empty",
            "hands full",
            // `both hands (?:free|might help)` -- the alternation expanded.
            "both hands free",
            "both hands might help",
            "empty hands",
        ],
    ),
    (
        MoveFailure::Position,
        &[
            // `stand(?:ing)? ?(?:up )?first` covers four spellings.
            "stand first",
            "stand up first",
            "standing first",
            "standing up first",
            "must be standing",
            "while sitting",
            "while lying down",
            "from that position",
            "already sitting",
            "should stand up",
            "standing up might help",
            "get up first",
        ],
    ),
    (
        MoveFailure::Closed,
        &[
            "appears to be closed",
            "seems to be closed",
            "squeeze between the stone doors",
        ],
    ),
    (
        MoveFailure::Swim,
        &[
            "attempt to swim",
            "begin to sink",
            "paddle back to safety",
            "around in the water",
            "swift current",
            "failure to swim",
            "current catches you",
        ],
    ),
    (MoveFailure::Drag, &["try to drag"]),
    (
        MoveFailure::Denied,
        &[
            "may not pass",
            "unseen force prevents",
            "aren't allowed to enter",
            "only performers",
            "see your ticket",
            "registered groups",
            "reputation precedes",
            "\"Abandoned.\"",
            "leave promptly",
            "open to invitees",
            "unable to follow you",
            "check in",
        ],
    ),
    (
        MoveFailure::Map,
        &[
            "can't go there",
            "can't go in that direction",
            "can't swim in that direction",
            "could not find what you were referring",
            "what were you referring",
            "where are you trying to go",
            "plan to do that here",
            "can't go to",
            "become impassable",
            "too far away",
            "too far above",
        ],
    ),
];

/// Read the line that ended a move.
///
/// **Only call this about a line answering an outstanding move.** See the
/// module doc: fed arbitrary traffic this table matches thousands of
/// unrelated lines, because `engaged` and `too far away` are ordinary English.
///
/// Returns [`MoveFailure::Unknown`] rather than `None` for an unrecognised
/// line, which is `move.rb:89-92`'s answer and the right one: the caller
/// already knows the move failed, so the question is only *why*, and "I do not
/// know, here is the line" is a real answer. `None` would make every caller
/// handle a case that means the same thing.
#[must_use]
pub fn classify(line: &str) -> MoveFailure {
    // Roundtime is matched first and anchored, because `move.rb:74` anchors
    // it with `^` where every other pattern is unanchored. A line merely
    // CONTAINING "wait 4" is not the game telling us to wait.
    let trimmed = line.trim_start();
    if is_roundtime(trimmed) {
        return MoveFailure::Roundtime;
    }

    let lower = line.to_lowercase();
    for (cause, phrases) in CAUSES {
        if phrases.iter().any(|p| lower.contains(&p.to_lowercase())) {
            return *cause;
        }
    }
    MoveFailure::Unknown
}

/// `^\.{3}wait \d|^wait \d` — the only anchored pattern in the table.
fn is_roundtime(line: &str) -> bool {
    let rest = line.strip_prefix("...").unwrap_or(line);
    rest.strip_prefix("wait ")
        .or_else(|| rest.strip_prefix("Wait "))
        .is_some_and(|tail| tail.starts_with(|c: char| c.is_ascii_digit()))
}

/// Why a stand kept failing, when **the line cannot say**.
///
/// `stand_failure_cause` (`move.rb:105-118`), and the reason it exists is a
/// real ambiguity rather than a shortcut:
///
/// > *"You struggle, but fail to stand" is the same text for a heavy pack and
/// > for leg wounds, so look at the character rather than the line.*
///
/// So this takes the two character facts instead of a line. A port that read
/// only the line would answer [`MoveFailure::Position`] for a character who
/// is in fact overburdened, and a caller would then retry standing forever
/// instead of dropping something.
///
/// Falls back to `Position`, which is Lich's default and the one that is true
/// whatever else is: the character is down.
#[must_use]
pub fn stand_failure_cause(overburdened: bool, limb_wounds: bool) -> MoveFailure {
    if overburdened {
        MoveFailure::Encumbered
    } else if limb_wounds {
        MoveFailure::Injured
    } else {
        MoveFailure::Position
    }
}

/// The cause for a line from the shared skill-roll branch.
///
/// `roll_cause` (`move.rb:100-103`): swim, drag and guard lines are named for
/// what they are; **anything else on that branch is a climb**. The branch is
/// only entered for a climb or a swim, so the fallback is information rather
/// than a guess — which is why [`MoveFailure::Climb`] has no phrases of its
/// own in `CAUSES` and can only be reached here.
#[must_use]
pub fn roll_cause(line: &str) -> MoveFailure {
    match classify(line) {
        c @ (MoveFailure::Swim | MoveFailure::Drag | MoveFailure::Denied) => c,
        _ => MoveFailure::Climb,
    }
}

/// How many times a remedy that should work first time is retried.
///
/// `MAX_REMEDIES` (`move.rb:47-50`). Stand, unhide, empty hands, retreat,
/// open, stow: *"past this, the remedy is not working and no number of
/// repeats will change that."*
pub const MAX_REMEDIES: u8 = 3;

/// How many times a skill roll is retried.
///
/// `MAX_ROLLS` (`move.rb:52-56`). A climb or swim *"legitimately fails several
/// times before succeeding"*, so it gets a much longer leash than a remedy —
/// the distinction is between a condition to fix and a die to re-roll.
pub const MAX_ROLLS: u8 = 20;
