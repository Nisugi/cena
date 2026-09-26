//! Where a line ends is the same fact for every reader: `plan/45` §4a.
//!
//! Two things assemble lines today. The model's `route_text` closes a line on
//! [`TextFrame::ends_line`] and nothing else; Despana's `LineAssembler`
//! (`cena-ui`) also closes one at every prompt and splits at an embedded
//! newline. M8 publishes the model's line to every viewer, with the triggers'
//! responses attached, so the two must not disagree about where a line is.
//!
//! On text they disagree in exactly two cases, and both are frame facts:
//!
//! 1. **A line still open at a prompt.** Despana flushes it there; the model
//!    keeps it and joins it to whatever that stream says next.
//! 2. **A newline inside a text frame.** Despana splits there; the model
//!    does not. `push_bytes` splits the wire on newlines before any frame
//!    exists (`line_boundaries.rs`), so only a decoded entity could put one
//!    back.
//!
//! Room components are the third difference and are not text lines at all:
//! the model keeps them as room state, so they are not measured here.

use cena_protocol::Parser;
use cena_protocol::frame::Frame;
use std::collections::BTreeMap;
use std::path::PathBuf;

/// Every committed fixture, in name order.
fn fixtures() -> Vec<PathBuf> {
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures");
    let mut paths: Vec<PathBuf> = std::fs::read_dir(&dir)
        .map(|entries| {
            entries
                .filter_map(Result::ok)
                .map(|entry| entry.path())
                .filter(|path| path.extension().is_some_and(|ext| ext == "xml"))
                .collect()
        })
        .unwrap_or_default();
    paths.sort();
    paths
}

/// A fixture's frames, through the byte-level read boundary.
fn frames(path: &std::path::Path) -> Vec<Frame> {
    let bytes = std::fs::read(path).unwrap_or_default();
    let mut parser = Parser::new();
    let mut frames = parser.push_bytes(&bytes);
    frames.extend(parser.push_bytes(b"\n"));
    frames
}

/// What the two readers would disagree about in one fixture.
#[derive(Default)]
struct Disagreements {
    /// `stream: text` of each line still open when a prompt arrived.
    open_at_prompt: Vec<String>,
    /// Text frames carrying a newline.
    newlines: Vec<String>,
}

fn disagreements(frames: &[Frame]) -> Disagreements {
    let mut found = Disagreements::default();
    // The text of each stream's unfinished line, as the model holds it.
    let mut open: BTreeMap<String, String> = BTreeMap::new();
    for frame in frames {
        match frame {
            Frame::Text(text) => {
                if text.content.contains('\n') {
                    found
                        .newlines
                        .push(format!("{:?}: {:?}", text.stream, text.content));
                }
                let line = open.entry(text.stream.clone()).or_default();
                line.push_str(&text.content);
                if text.ends_line {
                    open.remove(&text.stream);
                }
            }
            Frame::Prompt { .. } => {
                for (stream, line) in std::mem::take(&mut open) {
                    found.open_at_prompt.push(format!("{stream:?}: {line:?}"));
                }
            }
            _ => {}
        }
    }
    found
}

#[test]
fn the_fixtures_are_all_read() {
    // A test over zero files passes; this one must not.
    assert!(fixtures().len() >= 20, "{:?}", fixtures());
}

#[test]
fn no_line_is_left_open_at_a_prompt() {
    let mut report = Vec::new();
    for path in fixtures() {
        for line in disagreements(&frames(&path)).open_at_prompt {
            report.push(format!("{}: {line}", path.display()));
        }
    }
    assert!(
        report.is_empty(),
        "{} open at a prompt:\n{}",
        report.len(),
        report.join("\n")
    );
}

#[test]
fn no_text_frame_carries_a_newline() {
    let mut report = Vec::new();
    for path in fixtures() {
        for text in disagreements(&frames(&path)).newlines {
            report.push(format!("{}: {text}", path.display()));
        }
    }
    assert!(
        report.is_empty(),
        "{} newlines:\n{}",
        report.len(),
        report.join("\n")
    );
}

#[test]
fn an_open_line_at_a_prompt_is_seen() {
    // The measurement must be able to fail: a stream interrupted by a push
    // and never finished before the prompt.
    let mut parser = Parser::new();
    let frames = parser.push_bytes(
        b"You hear <pushStream id=\"thoughts\"/>[General] Bob: hi<popStream/>\n<prompt time=\"1\">&gt;</prompt>\n",
    );
    let found = disagreements(&frames);
    assert_eq!(found.open_at_prompt.len(), 1, "{:?}", found.open_at_prompt);
}
