//! Three defects found by auditing `reference/wiki_clean/Wrayth protocol.txt`
//! against the code, 2026-09-18, after the author pointed out that Claude kept
//! re-deriving facts already documented on the wiki they wrote.
//!
//! Each was invisible to a fully green suite, and each is proven falsifiable
//! by reverting its fix.

use cena_protocol::frame::LinkKind;
use cena_protocol::{Frame, Parser};

/// Every frame a set of wire lines produces.
fn frames(lines: &[&str]) -> Vec<Frame> {
    let mut parser = Parser::new();
    let mut out = Vec::new();
    for line in lines {
        out.extend(parser.parse_line(line));
    }
    out
}

/// Wiki `:9` and `:50`: `<stream id=X>...</stream>` is the **paired** form of
/// the `pushStream` redirect, so its body belongs to that channel.
///
/// Cena mapped it to `Frame::StreamWindow` -- a WINDOW DECLARATION -- and let
/// its text fall to `main` untagged. Live: 31 `<stream id="Spells">` rows in
/// one corpus file, each a spell-list entry, delivered as story prose. A
/// behavior filtering on the stream saw none of them.
///
/// Goes RED on removing the `"stream"` arm from `dispatch.rs` (verified):
/// the text's stream becomes `""`, which is main.
#[test]
fn a_paired_stream_routes_its_body_to_that_stream() {
    let got = frames(&[r#"<stream id="Spells">Ranger Base:</stream>"#]);
    let text = got
        .iter()
        .find_map(|f| match f {
            Frame::Text(t) => Some(t),
            _ => None,
        })
        .expect("the body must reach the consumer as text");
    assert_eq!(
        text.stream, "Spells",
        "paired `<stream id=X>` text belongs to X, exactly as `pushStream` \
         text does. An empty stream means main, which is where spell-list rows \
         were landing."
    );

    // And the routing must not leak past the close.
    let after = frames(&[r#"<stream id="Spells">inside</stream>"#, "outside"]);
    let last = after
        .iter()
        .rev()
        .find_map(|f| match f {
            Frame::Text(t) if t.content.contains("outside") => Some(t),
            _ => None,
        })
        .expect("text after the close must still arrive");
    assert_eq!(
        last.stream, "",
        "`</stream>` ends the redirect; text after it is main's again"
    );
}

/// Wiki `:317-318` documents `<a char= game=>` for player references.
///
/// Cena funnelled every `<a>` with no `href`/`exist`/`cmd` into
/// `LinkKind::DirectText`, which means "**send the link text as a command**".
/// So `<a char='Someone'>Someone</a>` became a link that would send a player's
/// NAME to the game -- invention rather than omission, and the failure a
/// consumer is least able to detect.
///
/// Goes RED on restoring the shared `else` branch in `markup.rs` (verified):
/// `command()` returns `Some("Nisugi")`.
#[test]
fn an_unactionable_anchor_does_not_fabricate_a_command() {
    let got = frames(&[r"<a char='Nisugi' game='GSIV'>Nisugi</a>"]);
    let link = got
        .iter()
        .find_map(|f| match f {
            Frame::Text(t) => t.link.as_ref(),
            _ => None,
        })
        .expect("the anchor still marks its text, so a link is still reported");
    assert_eq!(
        link.kind,
        LinkKind::NotActionable,
        "an `<a>` carrying none of href/exist/cmd is not a command link"
    );
    assert_eq!(
        link.command(),
        None,
        "and it must yield NO command: DirectText here would send the \
         player's own name to the game"
    );

    // The bare `<d>` case must keep working -- it is 85% of direct links.
    let d = frames(&["<d>east</d>"]);
    let dlink = d
        .iter()
        .find_map(|f| match f {
            Frame::Text(t) => t.link.as_ref(),
            _ => None,
        })
        .expect("a bare <d> is a link");
    assert_eq!(
        dlink.kind,
        LinkKind::DirectText,
        "a bare `<d>` still means 'the text is the command' -- every room exit \
         is one, and scoping the fallback to `<d>` must not break it"
    );
}
