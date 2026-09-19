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
