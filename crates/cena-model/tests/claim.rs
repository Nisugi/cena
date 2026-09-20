//! Room claiming: is this room mine to hunt?
//!
//! Every room here is folded through the real parser rather than built by
//! hand, for `plan/18`'s reason: `pbarStance` shipped in `vitals` past 17 green
//! tests because every model test used a hand-written snippet. A claim check
//! that reads the wrong component would pass a struct-literal test forever.
//!
//! # Where a hidden player appears, measured
//!
//! `obvious signs of someone hiding` is the last item of the room's `You also
//! see ...` list -- `room objs`. MEASURED across the reference logs: **477
//! lines carry the phrase and 477 of them are in a `You also see` list**, none
//! anywhere else.
//!
//! The room below is the author's own, given verbatim, because an invented one
//! would have matched whatever I had in mind while writing the check:
//!
//! ```text
//! You also see the faenor Demandred disk inlaid with intersecting bands of
//! jet and obvious signs of someone hiding.
//! ```

use cena_model::state::claim::{Claim, Occupants, claim, claim_room};
use cena_model::{GameState, Room};
use cena_protocol::Parser;

/// Fold wire bytes into a `GameState`, as a session would.
fn fold(wire: &[u8]) -> GameState {
    let mut state = GameState::default();
    let mut parser = Parser::new();
    for frame in parser.push_bytes(wire) {
        state.apply(&frame);
    }
    state
}

/// A room with the given players component and `You also see` list.
fn room_with(players: &str, objects: &str) -> Room {
    let wire = format!(
        "<component id='room objs'>{objects}</component>\n\
         <component id='room players'>{players}</component>\n"
    );
    fold(wire.as_bytes()).room
}

/// The author's own room, verbatim -- a disk and a hidden player.
///
/// Given rather than invented: a hand-written line would have matched whatever
/// I had in mind while writing the check, which is how a classifier passes its
/// tests and fails on the wire.
const HIDDEN: &str = "  You also see the <a exist=\"1\" noun=\"disk\">faenor Demandred disk</a> \
inlaid with intersecting bands of jet and obvious signs of someone hiding.";

/// The same room with the hiding clause removed.
const JUST_A_DISK: &str = "  You also see the <a exist=\"1\" noun=\"disk\">faenor Demandred disk</a> \
inlaid with intersecting bands of jet.";

/// An empty room is mine.
#[test]
fn an_empty_room_is_mine() {
    let room = room_with("", JUST_A_DISK);
    assert_eq!(claim_room(&room, &[]), Claim::Mine);
    assert!(claim_room(&room, &[]).is_mine());
}

/// A stranger contests the room, and is named.
#[test]
fn a_stranger_contests_the_room() {
    let room = room_with(
        "Also here: <a exist=\"-10070682\" noun=\"Dicate\">Dicate</a>",
        JUST_A_DISK,
    );
    assert_eq!(
        claim_room(&room, &[]),
        Claim::Contested {
            others: vec!["Dicate".to_owned()],
            hidden: false,
        }
    );
}

/// **A grouped partner does not contest the room.**
///
/// The subtraction in `claim.rb:89`. Without it, a hunting group of two would
/// report every room contested by each other and never hunt.
#[test]
fn a_grouped_partner_does_not_contest_the_room() {
    let room = room_with(
        "Also here: <a exist=\"-10070682\" noun=\"Dicate\">Dicate</a>",
        JUST_A_DISK,
    );
    assert_eq!(
        claim_room(&room, &["Dicate".to_owned()]),
        Claim::Mine,
        "a partner in my group is not an 'other'"
    );
}

/// Only the ungrouped occupants are reported.
#[test]
fn only_the_ungrouped_are_reported() {
    let room = room_with(
        "Also here: <a exist=\"-1\" noun=\"Dicate\">Dicate</a>, \
         <a exist=\"-2\" noun=\"Grhim\">Grhim</a>, \
         <a exist=\"-3\" noun=\"Xorus\">Xorus</a>",
        JUST_A_DISK,
    );
    assert_eq!(
        claim_room(&room, &["Grhim".to_owned()]),
        Claim::Contested {
            others: vec!["Dicate".to_owned(), "Xorus".to_owned()],
            hidden: false,
        },
        "Grhim is mine; the other two are not"
    );
}

/// Name comparison ignores case.
///
/// The group roster and the room roster are two different wire sources for the
/// same name, and a case difference between them would silently contest a
/// room a partner is standing in.
#[test]
fn names_are_compared_without_case() {
    let occupants = Occupants {
        named: vec!["Dicate".to_owned()],
        hidden: false,
    };
    assert_eq!(claim(&occupants, &["DICATE".to_owned()]), Claim::Mine);
    assert_eq!(claim(&occupants, &["dicate".to_owned()]), Claim::Mine);
}

/// **A hidden player is seen in the `You also see` list.**
///
/// `room objs`, not `room players` and not the description. MEASURED at 477 of
/// 477 occurrences across the reference logs; the room here is the author's own.
///
/// The failure it prevents is the one that matters: a hidden player is not in
/// `room players` at all, so a check that read only the typed roster would call
/// an occupied room empty and send a hunter into it.
#[test]
fn a_hidden_player_is_seen_in_the_objects_list() {
    let room = room_with("", HIDDEN);

    assert!(
        room.players.is_empty(),
        "the typed roster cannot see a hidden player -- that is the point"
    );
    assert_eq!(
        claim_room(&room, &[]),
        Claim::Contested {
            others: Vec::new(),
            hidden: true,
        },
        "an unnamed occupant still contests the room"
    );
}

/// **The nameless sign is never subtracted against the group.**
///
/// It has no name, so it cannot be matched -- and Lich does the same, pushing
/// `:hidden` into the list it subtracts from with nothing that can remove it.
///
/// This does not strand a hiding partner; see
/// `a_hiding_group_member_is_named_and_is_subtracted` below, which is the case
/// that makes refusing to subtract the sign *right* rather than merely
/// cautious.
#[test]
fn the_nameless_sign_cannot_be_subtracted() {
    let room = room_with("", HIDDEN);
    let big_group = ["Dicate".to_owned(), "Grhim".to_owned(), "Xorus".to_owned()];
    assert!(
        !claim_room(&room, &big_group).is_mine(),
        "no roster can clear an occupant with no name"
    );
}

/// **A hiding member of your own group is visible, BY NAME.**
///
/// > *"a grouped partner that is hidden is visible to the group
/// > `Also here: Demandred who is hiding`"* -- the author, 2026-09-20.
///
/// So the two hiding facts are complementary rather than contradictory: the
/// roster names everyone it can, including a hiding partner, and the nameless
/// `room objs` sign covers only the stranger it cannot name.
///
/// An earlier draft of `claim.rs` claimed a hiding partner "contests their own
/// room". That was wrong, and this is the test that would have caught it.
#[test]
fn a_hiding_group_member_is_named_and_is_subtracted() {
    let room = room_with(
        "Also here: <a exist=\"-1\" noun=\"Demandred\">Demandred</a> who is hiding, \
         <a exist=\"-2\" noun=\"Kiyna\">Kiyna</a>.",
        JUST_A_DISK,
    );

    assert_eq!(
        room.players
            .iter()
            .map(|p| p.noun.as_str())
            .collect::<Vec<_>>(),
        ["Demandred", "Kiyna"],
        "a hiding partner is an ordinary named entry in the roster"
    );

    assert_eq!(
        claim_room(&room, &["Demandred".to_owned(), "Kiyna".to_owned()]),
        Claim::Mine,
        "both are in my group, so the room is mine even though one hides"
    );

    // And a hiding STRANGER still contests it.
    assert_eq!(
        claim_room(&room, &["Kiyna".to_owned()]),
        Claim::Contested {
            others: vec!["Demandred".to_owned()],
            hidden: false,
        },
        "hiding does not exempt someone from contesting the room"
    );
}

/// A hidden player and a stranger are reported separately.
#[test]
fn a_hidden_player_and_a_stranger_are_both_reported() {
    let room = room_with(
        "Also here: <a exist=\"-1\" noun=\"Dicate\">Dicate</a>",
        HIDDEN,
    );
    assert_eq!(
        claim_room(&room, &[]),
        Claim::Contested {
            others: vec!["Dicate".to_owned()],
            hidden: true,
        }
    );
}

/// **A room that never reported its players is `Unknown`, not `Mine`.**
///
/// `plan/12` §5.2, and the reason [`Room::saw_players`] exists. Lich has no
/// such state because its check only ever runs inside a room arrival; here a
/// caller may hold a `Room` at any time.
///
/// Guard-before-clear in spirit: the same room WITH an empty players component
/// is asserted `Mine` below, so this test cannot pass merely because nothing
/// works.
#[test]
fn a_room_that_never_reported_players_is_unknown() {
    let never_told = fold(b"<component id='room desc'>A quiet clearing.</component>\n").room;
    assert!(!never_told.saw_players());
    assert_eq!(
        claim_room(&never_told, &[]),
        Claim::Unknown,
        "silence is not an empty room"
    );
    assert!(
        !claim_room(&never_told, &[]).is_mine(),
        "Unknown must not read as mine"
    );

    // The same room, having stated an empty roster, IS mine.
    let stated_empty = room_with("", JUST_A_DISK);
    assert!(stated_empty.saw_players());
    assert_eq!(claim_room(&stated_empty, &[]), Claim::Mine);
}

/// `Occupants::is_empty` agrees with the claim.
#[test]
fn occupants_is_empty_matches_the_claim() {
    let empty = Occupants::default();
    assert!(empty.is_empty());
    assert_eq!(claim(&empty, &[]), Claim::Mine);

    let hidden_only = Occupants {
        named: Vec::new(),
        hidden: true,
    };
    assert!(
        !hidden_only.is_empty(),
        "a hidden player makes the room non-empty"
    );

    let named_only = Occupants {
        named: vec!["Dicate".to_owned()],
        hidden: false,
    };
    assert!(!named_only.is_empty());
}

/// Ordinary description text does not invent a hidden player.
///
/// The phrase is matched as a substring, so this is where an over-broad match
/// would show.
#[test]
fn ordinary_description_text_does_not_hide_anyone() {
    for objects in [
        JUST_A_DISK,
        "  You also see signs of a struggle.",
        "  You also see a hiding place.",
        "  You also see someone here.",
        "",
    ] {
        let room = room_with("", objects);
        assert_eq!(
            claim_room(&room, &[]),
            Claim::Mine,
            "should not read a hidden player from: {objects:?}"
        );
    }
}
