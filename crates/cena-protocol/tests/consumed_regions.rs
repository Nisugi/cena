//! The two consumed REGIONS: the login `<settings>` blob and the
//! `<!-- CLIENT -->` echo.
//!
//! Split out of `unknown_and_boundary.rs` under Rule 4.1 (`plan/05:352-353`)
//! -- move code down, do not raise the cap.
//!
//! Neither region had a Tier 1 test. An attack on this crate's tests deleted
//! `self.in_settings = true` and the whole suite stayed green while the login
//! blob's 26 private element names leaked into `UnknownTag` on every login --
//! the exact cry-wolf failure the region exists to prevent. Tier 2 does not
//! cover it either: the default replay slice contains no login blob.

use cena_protocol::Parser;
use cena_protocol::frame::Frame;

#[test]
fn the_login_settings_region_is_consumed_whole_and_leaks_no_element_names() {
    // `<settings>` is a client-configuration document with its own vocabulary
    // -- `h`, `dc`, `cmdline`, `ignores`, `panels` and about twenty more, none
    // of them game protocol. It is consumed as a REGION and reported as one
    // frame. Goes RED if the region is disabled: each inner name then becomes
    // its own UnknownTag.
    let mut parser = Parser::new();
    let frames = parser.parse_line(
        "<settings client='1' major='1'><cmdline>x</cmdline><h id='1'/><dc id='2'/>\
         <ignores/><panels/></settings>ordinary prose",
    );

    let settings = frames
        .iter()
        .filter(|f| matches!(f, Frame::ClientSettings))
        .count();
    assert_eq!(settings, 1, "exactly one ClientSettings frame: {frames:#?}");

    let leaked: Vec<&str> = frames
        .iter()
        .filter_map(|f| match f {
            Frame::UnknownTag { name, .. } => Some(name.as_str()),
            _ => None,
        })
        .collect();
    assert!(
        leaked.is_empty(),
        "the blob's private element names must not reach the user: {leaked:?}"
    );

    // The region must end where it says it ends, not eat the rest of the line.
    assert!(
        frames
            .iter()
            .any(|f| matches!(f, Frame::Text(t) if t.content.contains("ordinary prose"))),
        "prose after </settings> must survive: {frames:#?}"
    );
}

#[test]
fn an_unclosed_settings_region_does_not_swallow_the_session() {
    // The blob spans lines (VERIFIED at 513,700 bytes on one), so the region
    // is parser state -- which means a blob whose close never arrives could
    // swallow everything. The prompt breaks it, for the same reason it breaks
    // anything else: it is the resync barrier.
    let mut parser = Parser::new();
    let _ = parser.parse_line("<settings client='1'><h id='1'/>");
    let swallowed = parser.parse_line("<h id='2'/><dc id='3'/>");
    assert!(
        swallowed.is_empty(),
        "lines inside the blob are the blob's: {swallowed:#?}"
    );

    let frames = parser.parse_line("<prompt time='1'>&gt;</prompt>");
    assert!(
        frames.iter().any(|f| matches!(f, Frame::Prompt { .. })),
        "a prompt must break an unclosed settings region: {frames:#?}"
    );
    let after = parser.parse_line("<nav rm='7503251'/>");
    assert!(
        after
            .iter()
            .any(|f| matches!(f, Frame::RoomId { id } if id.as_deref() == Some("7503251"))),
        "and traffic after it must parse normally: {after:#?}"
    );
}

#[test]
fn the_client_echo_region_yields_the_players_command_and_nothing_else() {
    // `<!-- CLIENT --> ... <!-- ENDCLIENT -->` is the client's own traffic,
    // not server output. The player's typed command is the one part worth
    // keeping: it is how a replay knows what the player did.
    let mut parser = Parser::new();
    let frames = parser.parse_line("<!-- CLIENT --><c>;go2 3609<!-- ENDCLIENT -->");

    let commands: Vec<&str> = frames
        .iter()
        .filter_map(|f| match f {
            Frame::ClientCommand { command } => Some(command.as_str()),
            _ => None,
        })
        .collect();
    assert_eq!(
        commands,
        vec![";go2 3609"],
        "the echoed command is the region's one useful frame: {frames:#?}"
    );

    let leaked: Vec<&str> = frames
        .iter()
        .filter_map(|f| match f {
            Frame::UnknownTag { name, .. } => Some(name.as_str()),
            _ => None,
        })
        .collect();
    assert!(
        leaked.is_empty(),
        "the region's own markup must not reach the user as unknown tags: {leaked:?}"
    );
}
