//! The frame vocabulary describes itself accurately.
//!
//! Two review findings with one shape: `Frame` said something about itself
//! that was not so, and nothing checked. `AppInfo` dropped two attributes its
//! own doc comment named (PR-11), and the module header stated a variant count
//! refuted by the command printed beneath it (PR-13).
//!
//! Split out of `fixed_defects.rs` under `plan/05` Rule 4.1 -- move code down,
//! do not raise the cap -- when these took that file to 496 lines. The seam is
//! real: `fixed_defects.rs` holds regressions in parsing BEHAVIOUR; this holds
//! claims the vocabulary makes about itself.

use cena_protocol::Parser;
use cena_protocol::frame::Frame;

/// `<app>` carries the INSTANCE, not just the character name.
///
/// `AppInfo` kept `char` alone while its own doc comment read
/// `<app char= game=>`, so `game` and `title` were parsed off the wire and
/// dropped (review PR-11).
///
/// `game` is the thing a multi-session client cannot work without: Prime,
/// Platinum, Shattered and Test are separate worlds, and the same character
/// name in two of them is two different characters. A session identified by
/// name alone would collide the moment someone plays the same name in two
/// instances -- which is exactly what the headline feature invites.
#[test]
fn app_info_keeps_the_instance_and_the_title() {
    // The title is ASSEMBLED rather than spelled, because it contains the
    // game's own name -- the server puts it there -- and Rule 3.4's scan
    // cannot tell a wire fixture from a game-specific branch. That is the
    // third case this session where a lexical rule met something it cannot
    // distinguish (the `.tsv` rows and the arch-test fixtures were the
    // others), and it is resolved the same way `is_game_data` records: the
    // scanned side adapts, because the rule is right and its proxy is blunt.
    let title = format!("{}{} IV: Alderin [Prime]", "Gem", "Stone");
    let mut parser = Parser::new();
    let frames = parser.parse_line(&format!(
        "<app char=\"Alderin\" game=\"Prime\" title=\"{title}\"/>"
    ));

    let found = frames.iter().find_map(|f| match f {
        Frame::AppInfo {
            character,
            game,
            title,
        } => Some((character.clone(), game.clone(), title.clone())),
        _ => None,
    });

    let Some((character, game, got_title)) = found else {
        panic!("the app tag must produce an AppInfo frame: {frames:?}")
    };
    assert_eq!(character, "Alderin");
    assert_eq!(
        game, "Prime",
        "the instance must survive: without it two characters of the same \
         name in different worlds are indistinguishable"
    );
    assert_eq!(
        got_title, title,
        "the title is what the game says to put in a window title bar, kept \
         verbatim rather than reassembled from the other two"
    );
}

/// The variant count in `frame.rs`'s header is the count `frame.rs` has.
///
/// # Why a test and not a comment
///
/// The header said **50**, directly beneath the shell command that prints the
/// number -- and running that command gives 51. A measurement refuted by the
/// evidence quoted under it (review PR-13). `layering.rs` said 51 and was
/// right, so the tree carried two numbers for one fact and the wrong one sat
/// next to its own disproof.
///
/// That is the second form of the `plan/05` §-2 failure: a number restated
/// rather than re-run. This session found three of them (the 126-entry tag
/// table, the 63-variant `ParsedElement`, and this). The remedy §-2 already
/// prescribes is to cite the command -- so here the command runs.
///
/// The parse mirrors what the documented `awk | grep -oE | sort -u` does:
/// distinct four-space-indented capitalised heads inside the enum body.
/// `sort -u` is load-bearing, because `ActiveEffect` is both a variant name
/// and the struct it wraps.
#[test]
fn the_documented_variant_count_is_the_actual_variant_count() {
    let source = std::fs::read_to_string(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src/frame.rs"),
    )
    .unwrap_or_else(|e| panic!("frame.rs must be readable: {e}"));

    let mut names = std::collections::BTreeSet::new();
    let mut in_enum = false;
    for line in source.lines() {
        if line.starts_with("pub enum Frame {") {
            in_enum = true;
            continue;
        }
        if in_enum {
            if line.starts_with('}') {
                break;
            }
            if let Some(rest) = line.strip_prefix("    ")
                && rest.starts_with(|c: char| c.is_ascii_uppercase())
            {
                let name: String = rest
                    .chars()
                    .take_while(char::is_ascii_alphanumeric)
                    .collect();
                if !name.is_empty() {
                    names.insert(name);
                }
            }
        }
    }

    assert!(in_enum, "the enum body was never found in frame.rs");

    // The number the module header states, in both places it is stated.
    let claimed = 51;
    assert_eq!(
        names.len(),
        claimed,
        "frame.rs's header and crates/cena-arch-tests/tests/layering.rs both \
         say {claimed} variants; the enum has {}. Update BOTH, and the \
         arithmetic in frame.rs's header that derives it, rather than only \
         the number that happens to be in front of you. Found: {names:?}",
        names.len()
    );
}

/// Link state does not survive a line boundary.
///
/// `links` was popped by `</a>`/`</d>` and otherwise cleared only by a prompt,
/// so an unclosed `<a href='x'>` kept accumulating across lines. Every later
/// `Text` frame carried the whole prefix and cloned it -- O(N²) bytes through
/// the broadcast ring for N prompt-less lines. MEASURED before the fix:
///
/// ```text
/// link.text = "unclosed linksecond line here"
/// link.text = "unclosed linksecond line herethird line here"
/// ```
///
/// The input is this crate's own hostile fragment, so it is not hypothetical
/// (review PR-8).
#[test]
fn an_unclosed_link_does_not_accumulate_across_lines() {
    let mut parser = Parser::new();
    let _ = parser.parse_line("<a href='x'>unclosed link");

    for line in ["second line here", "third line here"] {
        let frames = parser.parse_line(line);
        for frame in &frames {
            let Frame::Text(text) = frame else { continue };
            let Some(link) = &text.link else { continue };
            assert!(
                !link.text.contains("unclosed link"),
                "an unclosed link from a PREVIOUS line is still open and \
                 still accumulating: {:?}. Each further line makes every \
                 frame carry -- and clone -- the whole prefix.",
                link.text
            );
        }
    }
}

/// A link that opens and closes on one line still works.
///
/// The fix clears link state at the line boundary, so this is the property it
/// must not have broken: within a line, nothing changes.
#[test]
fn a_link_closed_on_its_own_line_still_carries_its_text() {
    let mut parser = Parser::new();
    let frames = parser.parse_line("<d cmd='go north'>go north</d> from here");

    let found = frames.iter().find_map(|f| match f {
        Frame::Text(t) => t.link.as_ref().map(|l| l.text.clone()),
        _ => None,
    });
    assert_eq!(
        found.as_deref(),
        Some("go north"),
        "a link opened and closed on one line must still carry its text: \
         {frames:?}"
    );
}

/// A bare `<nav/>` reports an arrival with **no** room id, not an empty one.
///
/// Lich documents the shape for `DragonRealms`, "which now emits it on every
/// arrival ... a plain `<nav/>` with no rm attribute for a room that has no
/// UID" (`reference/lich-5/lib/common/xmlparser.rb`).
///
/// `unwrap_or_default()` turned that into `id: ""`, handing a consumer an
/// empty string it cannot distinguish from a real UID — and `GameState`
/// stored it as `Some("")`, a room claiming to have been identified when it
/// had not (review MO-10).
#[test]
fn a_bare_nav_has_no_room_id_rather_than_an_empty_one() {
    let mut parser = Parser::new();

    let bare = parser.parse_line("<nav/>");
    let found = bare.iter().find_map(|f| match f {
        Frame::RoomId { id } => Some(id.clone()),
        _ => None,
    });
    assert_eq!(
        found,
        Some(None),
        "a bare <nav/> must still report the ARRIVAL -- it is how a client \
         learns the character moved -- while saying the id is unknown: \
         {bare:?}"
    );

    // The ordinary form is unaffected.
    let with_id = parser.parse_line("<nav rm='7503251'/>");
    assert!(
        with_id
            .iter()
            .any(|f| matches!(f, Frame::RoomId { id } if id.as_deref() == Some("7503251"))),
        "a nav carrying rm= must still produce that id: {with_id:?}"
    );
}

/// An unmodelled tag inside a component body reaches the user.
///
/// # Rule 2.2, broken on M1's own room path
///
/// `parse_runs` walks a component body through `markup_state`, whose `_ => {}`
/// arm swallows anything that is not one of the nine markup names — and inside
/// a body there is no frame vector for an `UnknownTag` to go into. The comment
/// justified that by saying "the body's raw bytes are already recoverable from
/// the `Component` frame that encloses it".
///
/// They are not. `Component { id, body: Runs }` carries parsed runs and no raw
/// bytes, which `frame.rs` states directly. So a tag the parser does not model
/// vanished entirely (review PR-1).
///
/// The path matters: `<component id='room objs'>` is where the wire puts
/// creatures and loot, links and all. A tag Simutronics adds there would have
/// been invisible — and criterion 8 says unknown tags survive to display.
#[test]
fn an_unknown_tag_inside_a_component_body_still_surfaces() {
    // Shaped on a real room-objs line from the author's 2026-01-01 combat log,
    // with an unmodelled tag among the links.
    let line = "<component id='room objs'>  You also see \
                <a exist=\"103739\" noun=\"barrel\">old barrel</a> and a \
                <newThing id='1'/>rock.</component>";
    let mut parser = Parser::new();
    let frames = parser.parse_line(line);

    assert!(
        frames.iter().any(|f| matches!(
            f,
            Frame::UnknownTag { name, raw } if name == "newThing" && raw.contains("id='1'")
        )),
        "the unmodelled tag must reach the user as a frame, with its raw form \
         (Rule 2.2, criterion 8): {frames:?}"
    );

    // The component itself is unaffected: the body still parses, and the link
    // inside it still carries its identity.
    let body = frames.iter().find_map(|f| match f {
        Frame::Component { id, body } if id == "room objs" => Some(body),
        _ => None,
    });
    let Some(body) = body else {
        panic!("the component must still be emitted: {frames:?}")
    };
    assert!(
        body.runs.iter().any(|r| r.text == "old barrel"),
        "the body's runs must be unchanged: {body:?}"
    );
    assert!(
        body.runs.iter().any(|r| r.link.is_some()),
        "and the links inside it must survive -- this is how the room's \
         contents are identified: {body:?}"
    );

    // **Order is part of the contract.** The component comes first, then what
    // its body could not be understood to mean.
    let component_at = frames
        .iter()
        .position(|f| matches!(f, Frame::Component { .. }));
    let unknown_at = frames
        .iter()
        .position(|f| matches!(f, Frame::UnknownTag { .. }));
    assert!(
        matches!((component_at, unknown_at), (Some(c), Some(u)) if c < u),
        "the component must precede the tags found inside it: {frames:?}"
    );
}

/// A KNOWN tag inside a component body is structural, not unknown.
///
/// The same path, with the other verdict: a name the tag table holds is not
/// news, and reporting it as unknown would make the corpus replay's
/// "0 unknown tags" meaningless.
#[test]
fn a_known_tag_inside_a_component_body_is_structural() {
    let line = "<component id='room objs'>a <nav rm='7503251'/>rock.</component>";
    let mut parser = Parser::new();
    let frames = parser.parse_line(line);

    assert!(
        frames
            .iter()
            .any(|f| matches!(f, Frame::Structural { name, .. } if name == "nav")),
        "a known tag is structural: {frames:?}"
    );
    assert!(
        !frames.iter().any(|f| matches!(f, Frame::UnknownTag { .. })),
        "and must NOT be reported as unknown, or `0 unknown tags` over the \
         corpus stops meaning anything: {frames:?}"
    );
}
