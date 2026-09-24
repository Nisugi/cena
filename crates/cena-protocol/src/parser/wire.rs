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

impl super::Parser {
    // Moved here from `parser.rs` when that file passed its 500-line facade
    // cap: the settings blob is a wire region, like the client region below.
    /// Consume a line belonging to an open login `<settings>` blob.
    ///
    /// Returns what is left to parse: the whole line when no blob is open,
    /// what follows `</settings>` when the blob closes on it, and `None` when
    /// the blob consumed all of it.
    ///
    /// Inside the blob the line is client configuration rather than game
    /// output, so it is consumed whole. A `<prompt>` breaks the region for the
    /// same reason it breaks a capture: a blob whose close never arrives must
    /// not swallow the rest of the session.
    pub(super) fn continue_settings_blob<'a>(&mut self, line: &'a str) -> Option<&'a str> {
        if self.settings != SettingsRegion::Open {
            return Some(line);
        }
        // Whichever comes FIRST ends the region, and a prompt ends it AT the
        // prompt: what precedes it on the line is still blob. Both are what
        // `push_bytes` does, scanning byte by byte, and this used to differ
        // on each -- checking the close before the prompt regardless of
        // order, and re-parsing the blob's bytes before a prompt as game
        // text. Found by `push_bytes_and_parse_line_agree`.
        let close = line.find(SETTINGS_CLOSE);
        let prompt = line.find("<prompt");
        match (close, prompt) {
            (Some(at), p) if p.is_none_or(|p| at < p) => {
                self.settings = SettingsRegion::Outside;
                let after = &line[at + SETTINGS_CLOSE.len()..];
                (!after.is_empty()).then_some(after)
            }
            (_, Some(at)) => {
                self.settings = SettingsRegion::Outside;
                Some(&line[at..])
            }
            _ => None,
        }
    }
}

/// The parser's position relative to the login `<settings>` blob.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(super) enum SettingsRegion {
    /// No blob open. The ordinary case.
    #[default]
    Outside,
    /// Inside a blob whose close has not arrived.
    Open,
    /// `push_bytes` saw the blob close on the line it is still reading.
    ///
    /// That line's blob bytes were never buffered, so at its newline
    /// `pending` holds only what followed `</settings>` -- nothing, when the
    /// close ended the line -- and parsing that empty remainder emitted a
    /// BLANK LINE that `parse_line` never produced for the same bytes. Found
    /// by `push_bytes_and_parse_line_agree` on its first run. A third state
    /// rather than a fourth `bool` on `Parser`: the two cannot both hold.
    ClosedThisLine,
}

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
        // Stripped like every other decoded text: `&#27;` here is a real ESC
        // by the time a replay prints it.
        command: text::strip_control_chars(&text::decode_entities(cmd)),
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
