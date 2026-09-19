//! **`Frame::InjuryImage` claimed a scope the parser did not check.**
//!
//! The variant's own documentation said *"`<image id= name=>` inside the
//! injuries dialog"*, and the parser emitted it for **every** `<image>` tag on
//! the wire.
//!
//! MEASURED over 24 files across 6 characters: **2,155** `<image>` tags, of which
//! **1,313** are `nomap.jpg` map tiles and a further ~50 are the weapon and
//! shield buttons (`SwordBtn`, `ShieldBtn`, `NoSwordBtn`, `NoShieldBtn`). Fewer
//! than 300 are body parts. So the overwhelming majority of `InjuryImage` frames
//! described no injury at all, and a consumer trusting the name would have read
//! `id: "nomap", name: "nomap.jpg"` as a body part.
//!
//! Lich scopes it explicitly -- `if (name == 'image') and
//! @active_ids.include?('injuries')` (`reference/lich-5/lib/common/
//! xmlparser.rb:809`) -- which is the same check by a different mechanism: it
//! walks a stack of open element ids, Cena carries the enclosing `dialogData` id
//! on the frame.
//!
//! That is the pattern `ProgressBar` already uses, and for the identical reason:
//! the same tag shape means different things depending on which dialog encloses
//! it, and keying on the tag alone let a target's health overwrite the player's.

use cena_protocol::{Frame, Parser};

fn images(wire: &[u8]) -> Vec<(String, String, Option<String>)> {
    Parser::new()
        .push_bytes(wire)
        .into_iter()
        .filter_map(|f| match f {
            Frame::InjuryImage { id, name, dialog } => Some((id, name, dialog)),
            _ => None,
        })
        .collect()
}

#[test]
fn an_image_inside_the_injuries_dialog_carries_that_dialog() {
    // The real wire shape, from `GSIV-Getho`.
    let found = images(
        b"<dialogData id='injuries'><image id=\"leftArm\" name=\"Injury1\" height=\"0\" width=\"0\"/></dialogData>\n",
    );
    assert_eq!(
        found,
        vec![(
            "leftArm".to_owned(),
            "Injury1".to_owned(),
            Some("injuries".to_owned())
        )]
    );
}

#[test]
fn a_map_tile_is_not_reported_as_an_injury() {
    // 1,313 of the 2,155 `<image>` tags in the sample. `id='nomap'` is a map
    // tile, and a consumer reading `InjuryImage` by name would have taken it for
    // a body part.
    let found = images(b"<image id='nomap' name='nomap.jpg' height='0' width='0'/>\n");
    assert_eq!(
        found,
        vec![("nomap".to_owned(), "nomap.jpg".to_owned(), None)],
        "an image outside any dialog must not claim to be an injury"
    );
}

#[test]
fn a_weapon_button_is_not_reported_as_an_injury() {
    let found =
        images(b"<image id='unsheathe' name='SwordBtn' cmd='_ready weapon' height='29'/>\n");
    assert_eq!(found[0].2, None, "a toolbar button claimed a dialog");
}

#[test]
fn an_image_in_another_dialog_carries_that_other_dialog() {
    // Not merely "injuries or nothing": the frame reports WHICH dialog, so a
    // consumer can tell an unfamiliar one apart rather than being told `None`.
    let found = images(
        b"<dialogData id='mapViewMain'><image id='mapIcon18008' name='eyesoff.jpg'/></dialogData>\n",
    );
    assert_eq!(found[0].2, Some("mapViewMain".to_owned()));
}

#[test]
fn the_dialog_does_not_leak_past_its_close() {
    // The enclosing-dialog state has to end with the dialog. A leak here would
    // make every later image on the line an injury.
    let found = images(
        b"<dialogData id='injuries'><image id=\"head\" name=\"head\"/></dialogData><image id='nomap' name='nomap.jpg'/>\n",
    );
    assert_eq!(found[0].2, Some("injuries".to_owned()));
    assert_eq!(
        found[1].2, None,
        "the injuries dialog leaked onto an image after its close"
    );
}
