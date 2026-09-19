//! Tier 1 goldens for the ROOM fixture -- M1's headline path.
//!
//! Split out of `golden_fixtures.rs` under Rule 4.1 (`plan/05:352-353`) --
//! move code down, do not raise the cap -- when the missing room-exits golden
//! was added.
//!
//! Each test names the fact it protects and fails with that fact in the
//! message, rather than snapshotting the frame stream: a snapshot goes
//! *different* on any change and gets regenerated, which is how the reference
//! waves a parser diff through.

use cena_protocol::Parser;
use cena_protocol::frame::{Frame, LinkKind};

/// Parse a committed fixture into frames, through the byte-level read
/// boundary so the fixtures exercise reassembly too.
fn parse_fixture(name: &str) -> Vec<Frame> {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures")
        .join(name);
    // No `unwrap`/`expect`/`panic!` here: clippy.toml's allow-*-in-tests
    // covers `#[test]` functions, not helpers, and the workspace denies all
    // three. An unreadable fixture yields no bytes and therefore no frames,
    // which every caller already asserts against -- `assert!(!frames.
    // is_empty())` and the specific-fact assertions all fail loudly.
    let bytes = std::fs::read(&path).unwrap_or_default();
    let mut parser = Parser::new();
    let mut frames = parser.push_bytes(&bytes);
    // A fixture need not end in a newline; flush whatever is pending.
    frames.extend(parser.push_bytes(b"\n"));
    frames
}

// ---------------------------------------------------------------------------
// Room
// ---------------------------------------------------------------------------

#[test]
fn the_room_fixture_yields_the_room_id() {
    let frames = parse_fixture("room.xml");
    let ids: Vec<&str> = frames
        .iter()
        .filter_map(|f| match f {
            Frame::RoomId { id } => id.as_deref(),
            _ => None,
        })
        .collect();
    assert_eq!(
        ids,
        vec!["7503251"],
        "<nav rm=> is the room-change signal; without it a behavior cannot \
         know it moved"
    );
}

#[test]
fn room_components_arrive_parsed_rather_than_as_markup() {
    // The Rule 2.1 fix. Vellum hands the layer above
    // `"Also here: <a exist=\"-11047747\" ...>Alderin</a>"` as a String.
    let frames = parse_fixture("room.xml");
    let players: Vec<&Frame> = frames
        .iter()
        .filter(|f| matches!(f, Frame::Component { id, .. } if id == "room players"))
        .collect();
    assert!(
        players.len() >= 2,
        "the fixture carries an arrival and a departure, so `room players` \
         must appear at least twice; got {}",
        players.len()
    );

    let occupied = players
        .iter()
        .find_map(|f| match f {
            Frame::Component { body, .. } if !body.is_blank() => Some(body),
            _ => None,
        })
        .expect("one `room players` component is non-empty in this fixture");

    assert!(
        !occupied.plain().contains('<'),
        "a component body must reach the caller parsed, never as markup \
         (Rule 2.1, plan/05:270-274). Got: {:?}",
        occupied.plain()
    );
    let links: Vec<&LinkKind> = occupied.links().map(|l| &l.kind).collect();
    assert_eq!(
        links,
        vec![&LinkKind::Exist {
            id: "-11047747".to_owned(),
            noun: "Alderin".to_owned(),
        }],
        "the player in the room is a link with an exist id, and that id is \
         how a behavior targets them"
    );
}

#[test]
fn an_empty_room_players_component_is_a_message_not_an_absence() {
    // The departure. `<compDef id='room players'></compDef>` means "nobody is
    // here", which is different from the component not arriving.
    let frames = parse_fixture("room.xml");
    assert!(
        frames.iter().any(
            |f| matches!(f, Frame::Component { id, body } if id == "room players" && body.is_blank())
        ),
        "an empty `room players` must still produce a Component frame"
    );
}

#[test]
fn the_compass_yields_its_directions() {
    let frames = parse_fixture("room.xml");
    let compass = frames
        .iter()
        .find_map(|f| match f {
            Frame::Compass { directions } => Some(directions),
            _ => None,
        })
        .expect("room.xml carries a <compass>");
    assert_eq!(compass, &vec!["e".to_owned(), "out".to_owned()]);
}

#[test]
fn the_room_stream_is_pushed_and_popped() {
    let frames = parse_fixture("room.xml");
    let pushed = frames
        .iter()
        .any(|f| matches!(f, Frame::StreamPush { id } if id == "room"));
    let popped = frames
        .iter()
        .any(|f| matches!(f, Frame::StreamPop { id: Some(id) } if id == "room"));
    let cleared = frames
        .iter()
        .any(|f| matches!(f, Frame::ClearStream { id } if id == "room"));
    assert!(
        pushed && popped && cleared,
        "the room frame envelope is clear/push/pop; got push={pushed} \
         pop={popped} clear={cleared}"
    );
}

#[test]
fn exit_links_keep_their_coordinates() {
    // `<a exist= coord= noun=>` is the movement form; coord is what a Travel
    // behavior would route on.
    let frames = parse_fixture("room.xml");
    let coords: Vec<&str> = frames
        .iter()
        .filter_map(|f| match f {
            Frame::Text(t) => t.link.as_ref()?.coord.as_deref(),
            _ => None,
        })
        .collect();
    assert!(
        coords.contains(&"2524,1864"),
        "exit links carry coord=; got {coords:?}"
    );
}

#[test]
fn room_exits_carry_the_command_a_travel_behavior_would_send() {
    // THE GOLDEN THAT WAS MISSING. `room.xml` has contained
    // `<compDef id='room exits'>Obvious exits: <d>east</d>, <d>out</d></compDef>`
    // since it was cut, and nothing asserted on it -- so a bare `<d>` shipping
    // `LinkKind::Direct { cmd: "" }` went unnoticed on M1's own room path,
    // against M1's own committed fixture. A Travel behavior reading exits got
    // two empty strings.
    //
    // The bare form is not an edge case: 27,527 bare `<d>` against 4,890
    // `<d cmd=>` in a 40-file corpus sample, so 85% of direct links are this
    // shape, and every room exit in the game is one.
    //
    // Goes RED if `LinkKind::DirectText` stops resolving through
    // `Link::command()`, or if the exits component stops parsing.
    let frames = parse_fixture("room.xml");
    let exits = frames
        .iter()
        .find_map(|f| match f {
            Frame::Component { id, body } if id == "room exits" => Some(body),
            _ => None,
        })
        .expect("room.xml carries a `room exits` component");

    let commands: Vec<&str> = exits.links().filter_map(|l| l.command()).collect();
    assert_eq!(
        commands,
        vec!["east", "out"],
        "a bare <d> carries its command as link text; an empty command here \
         means Travel has nothing to send. Got {:?}",
        exits
            .links()
            .map(|l| (&l.kind, l.text.as_str()))
            .collect::<Vec<_>>()
    );

    // And the `<d cmd=>` form still works, from the departure line in the
    // same fixture: `just went <d cmd='go out'>out</d>`.
    let with_cmd: Vec<&str> = frames
        .iter()
        .filter_map(|f| match f {
            Frame::Text(t) => t.link.as_ref(),
            _ => None,
        })
        .filter(|l| matches!(l.kind, LinkKind::Direct { .. }))
        .filter_map(|l| l.command())
        .collect();
    assert_eq!(
        with_cmd,
        vec!["go out"],
        "an explicit cmd= must still be preferred over the link text"
    );
}
