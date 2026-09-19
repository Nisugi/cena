//! **A quote inside an attribute value**, and why no repair is needed.
//!
//! Possessive names are everywhere in this game -- `Imaera's Lace`,
//! `jack-o'-lantern`, `Ta'Vaalor`, `Widowmaker's Road` -- so "the value ends at
//! the first matching quote" truncates them silently. Lich repairs a malformed
//! version of this input (`XMLCleaner.clean_nested_quotes`,
//! `reference/lich-5/lib/games.rb:259-280`). Cena does not, because **the server
//! never sends it**: it is well-formed by two different mechanisms, and this file
//! pins both so a future change to `text::attribute` cannot quietly break them.
//!
//! | Mechanism | Example |
//! |---|---|
//! | escape as `&apos;` | `<d cmd='forage Imaera&apos;s Lace'>Imaera's Lace</d>` |
//! | switch to double quotes | `<label value="Ta'Vaalor Environs"/>` |
//!
//! MEASURED 2026-09-19: **0** malformed attribute regions in **11,576,123** tags
//! from 42 of the author's 504 deliberate foraging sessions
//! (`forge_sessions/*/raw.xml.log`) -- the corpus most likely to hold possessive
//! herb names, 14 of which appear in the survey's own forage list. Also **0** in
//! **995,407** tags across 8 characters in the log archive, against **1,637**
//! well-formed double-quoted values containing an apostrophe.
//!
//! # Why there is no fix here, only tests
//!
//! One was written and reverted. Ending a value at "the last quote followed by
//! whitespace, `/` or `>`" passes for `title='Tsetem's Items'` while making `id`
//! swallow the rest of the tag -- a LATER attribute's terminator satisfies an
//! EARLIER attribute's scan. Tests for the malformed shape went green while
//! 995,407 tags' worth of working parsing broke, because the falsification was
//! scoped to the defect and not to what already worked. Full record in
//! `plan/15` §2a.4a.

use cena_protocol::{Frame, Parser};

fn frames(wire: &[u8]) -> Vec<Frame> {
    Parser::new().push_bytes(wire)
}

/// The `cmd` of the first link-bearing text run.
fn link_cmd(wire: &[u8]) -> Option<String> {
    frames(wire).into_iter().find_map(|f| match f {
        Frame::Text(t) => t.link.map(|l| format!("{:?}", l.kind)),
        _ => None,
    })
}

#[test]
fn an_apos_entity_in_a_command_round_trips_to_a_sendable_string() {
    // The real wire line, from `20260817-213302-survey`. A behavior has to be
    // able to send this command verbatim, so the entity MUST be decoded.
    let cmd = link_cmd(b"<d cmd='forage Imaera&apos;s Lace'>Imaera's Lace</d>\n")
        .expect("the link must carry its command");
    assert!(
        cmd.contains("forage Imaera's Lace"),
        "`&apos;` was not decoded, so a forage command would be unsendable: {cmd}"
    );
}

#[test]
fn the_display_text_keeps_the_literal_apostrophe() {
    // Both forms appear on the SAME line: escaped in the attribute, literal in
    // the text. Neither may leak into the other.
    let text: Vec<String> = frames(b"<d cmd='forage Imaera&apos;s Lace'>Imaera's Lace</d>\n")
        .into_iter()
        .filter_map(|f| match f {
            Frame::Text(t) => Some(t.content),
            _ => None,
        })
        .collect();
    assert_eq!(text, vec!["Imaera's Lace".to_owned()]);
}

#[test]
fn a_double_quoted_value_carries_an_apostrophe_intact() {
    // `<label id='TVLabel' value="Ta'Vaalor Environs" .../>`, from the archive.
    let found = frames(b"<label id='TVLabel' value=\"Ta'Vaalor Environs\" justify='4'/>\n")
        .into_iter()
        .find_map(|f| match f {
            Frame::Label { id, value } => Some((id, value)),
            _ => None,
        });
    assert_eq!(
        found,
        Some(("TVLabel".to_owned(), "Ta'Vaalor Environs".to_owned())),
        "the server switches to double quotes for values holding an apostrophe"
    );
}

#[test]
fn a_possessive_subtitle_survives_and_the_attributes_after_it_are_found() {
    // The archive's most common shape, 412 occurrences on `subtitle` alone. The
    // reverted fix broke exactly this: a later attribute's closing quote ended
    // an earlier attribute's value.
    let found = frames(
        b"<streamWindow id='room' title='Room' subtitle=\" - Widowmaker's Road\" \
          location='center' target='drop'/>\n",
    )
    .into_iter()
    .find_map(|f| match f {
        Frame::StreamWindow {
            id,
            title,
            subtitle,
            attrs,
        } => Some((id, title, subtitle, attrs)),
        _ => None,
    })
    .expect("a StreamWindow must be produced");

    let (id, title, subtitle, attrs) = found;
    assert_eq!(id, "room", "the FIRST attribute must not absorb the rest");
    assert_eq!(title.as_deref(), Some("Room"));
    assert_eq!(subtitle.as_deref(), Some(" - Widowmaker's Road"));
    assert_eq!(
        attrs
            .iter()
            .find(|(k, _)| k == "location")
            .map(|(_, v)| v.as_str()),
        Some("center"),
        "an attribute AFTER the possessive one must still be found"
    );
}

#[test]
fn a_possessive_noun_is_read_whole() {
    // `noun="jack-o'-lantern"` -- 92 occurrences. The noun registry (`plan/18`
    // step 5) keys on this, so a truncation here is a permanently wrong key.
    let nouns: Vec<String> = frames(
        b"<component id='room objs'><a exist=\"1\" noun=\"jack-o'-lantern\">a \
          jack-o'-lantern</a></component>\n",
    )
    .into_iter()
    .filter_map(|f| match f {
        Frame::Component { body, .. } => Some(
            body.runs
                .iter()
                .filter_map(|r| r.link.as_ref().map(|l| l.text.as_str()))
                .collect::<String>(),
        ),
        _ => None,
    })
    .collect();
    assert_eq!(nouns, vec!["a jack-o'-lantern".to_owned()]);
}

#[test]
fn every_attribute_of_a_mixed_quoting_tag_is_read() {
    // `plan/15` §2a.4: quoting is not uniform WITHIN one tag. This is the guard
    // for the whole scan, not just the possessive case.
    let frame = frames(b"<streamWindow id='a' title=\"b\" subtitle='c' location=\"d\"/>\n")
        .into_iter()
        .find_map(|f| match f {
            Frame::StreamWindow { attrs, .. } => Some(attrs),
            _ => None,
        })
        .expect("a StreamWindow must be produced");
    let got: Vec<(&str, &str)> = frame
        .iter()
        .map(|(k, v)| (k.as_str(), v.as_str()))
        .collect();
    assert_eq!(
        got,
        vec![
            ("id", "a"),
            ("title", "b"),
            ("subtitle", "c"),
            ("location", "d")
        ],
        "mixed quoting within one tag must not confuse the walk"
    );
}
