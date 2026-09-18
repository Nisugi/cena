//! Wire framing below the tag level: line decoding, and the two regions that
//! are not markup.
//!
//! Split out of `parser.rs` under Rule 4.1 (`plan/05:352-353`) -- move code
//! down, do not raise the cap. These are the pieces that answer "what are
//! these bytes" rather than "what does this tag mean".

use crate::frame::Frame;
use crate::text;

/// Opens a client-echo region.
pub(super) const CLIENT_OPEN: &str = "<!-- CLIENT -->";
/// Closes one.
pub(super) const CLIENT_CLOSE: &str = "<!-- ENDCLIENT -->";
/// Ordinary comment terminator.
pub(super) const COMMENT_CLOSE: &str = "-->";
/// Closes the login client-settings blob.
pub(super) const SETTINGS_CLOSE: &str = "</settings>";

/// Consume a `<!-- CLIENT --> ... <!-- ENDCLIENT -->` region.
///
/// Returns the command the player typed, when the region carries one, and the
/// remainder of the line after the region. An unterminated region consumes the
/// rest of the line: it is client traffic either way, and guessing where it
/// ends would put settings chatter into the game text.
pub(super) fn client_region(tail: &str) -> (Option<Frame>, &str) {
    let body_start = CLIENT_OPEN.len();
    let (body, after) = match tail[body_start..].find(CLIENT_CLOSE) {
        Some(at) => (
            &tail[body_start..body_start + at],
            &tail[body_start + at + CLIENT_CLOSE.len()..],
        ),
        None => (&tail[body_start..], ""),
    };
    // `<c>` carries the player's own command; everything else in the region
    // is the client's settings traffic and is not protocol.
    let command = body.strip_prefix("<c>").map(|cmd| Frame::ClientCommand {
        command: text::decode_entities(cmd),
    });
    (command, after)
}

/// Decode a wire line: UTF-8 if it is valid, CP1252 otherwise.
///
/// The game stream is really CP1252. Trying UTF-8 first is safe rather than
/// ambiguous: a stray CP1252 high byte is structurally invalid UTF-8, so a
/// line cannot be misclassified. Ported from
/// `reference/VellumFE/src/network.rs:488`.
pub(super) fn decode_wire_line(bytes: &[u8]) -> String {
    match std::str::from_utf8(bytes) {
        Ok(s) => s.to_owned(),
        Err(_) => bytes.iter().map(|&b| cp1252_char(b)).collect(),
    }
}

/// One CP1252 byte as a `char`.
fn cp1252_char(byte: u8) -> char {
    // 0x80-0x9F is where CP1252 differs from Latin-1; everything else is
    // codepoint-identical.
    const HIGH: [char; 32] = [
        '\u{20ac}', '\u{81}', '\u{201a}', '\u{192}', '\u{201e}', '\u{2026}', '\u{2020}',
        '\u{2021}', '\u{2c6}', '\u{2030}', '\u{160}', '\u{2039}', '\u{152}', '\u{8d}', '\u{17d}',
        '\u{8f}', '\u{90}', '\u{2018}', '\u{2019}', '\u{201c}', '\u{201d}', '\u{2022}', '\u{2013}',
        '\u{2014}', '\u{2dc}', '\u{2122}', '\u{161}', '\u{203a}', '\u{153}', '\u{9d}', '\u{17e}',
        '\u{178}',
    ];
    match byte {
        0x80..=0x9F => HIGH[(byte - 0x80) as usize],
        _ => char::from(byte),
    }
}
