//! The read boundary: raw socket bytes reassembled into whole lines.
//!
//! Split out of `parser.rs` under Rule 4.1 (`plan/05:352-353`) -- move code
//! down, do not raise the cap.
//!
//! A [`Parser`] never sees a TCP fragment. Bytes are buffered until a
//! newline arrives, so a tag split across two reads is rejoined before parsing and
//! cannot desync the parser. Enforcing that here rather than assuming it of
//! the caller is what lets a test drive a split at every byte offset.

use super::wire::decode_wire_line;
use super::{MAX_LINE_BYTES, Parser};
use crate::frame::Frame;

impl Parser {
    /// Feed raw socket bytes; get frames for every **complete** line.
    ///
    /// This is the read boundary. Bytes are buffered until a `\n` arrives, so
    /// a tag split across two TCP reads is rejoined before parsing and cannot
    /// desync the parser. An unterminated trailing fragment stays pending for
    /// the next call -- and is therefore never parsed as if it were complete,
    /// which is the failure this method exists to prevent.
    pub fn push_bytes(&mut self, chunk: &[u8]) -> Vec<Frame> {
        let mut frames = Vec::new();
        for &byte in chunk {
            if byte == b'\n' {
                // The newline ends an oversized line's discarded tail too:
                // this is the known boundary the drop was waiting for.
                if std::mem::take(&mut self.dropping_oversized_line) {
                    self.pending.clear();
                    continue;
                }
                let line = decode_wire_line(&self.pending);
                self.pending.clear();
                frames.extend(self.parse_line(&line));
            } else if self.dropping_oversized_line {
                // Mid-discard: swallow bytes until the newline above.
            } else if self.pending.len() < MAX_LINE_BYTES {
                self.pending.push(byte);
            } else if !self.dropping_oversized_line {
                // Runaway line. The buffer must not grow without bound, but it
                // must also not be re-parsed: the cap falls at an arbitrary
                // byte, which is usually mid-tag, and feeding that to
                // `parse_line` split `<pushStream id='room'/>` into a
                // `MalformedTag { raw: "<push" }` plus the literal text
                // `Stream id='room'/>BODY` -- raw markup rendered to the user,
                // the exact bug this parser's header says it does not inherit,
                // merely moved from the tag scanner to the length guard.
                //
                // So the oversized buffer is reported once, as one typed
                // frame, and the rest of the line is DISCARDED up to the next
                // newline. Resuming on a known boundary is what keeps
                // `push_bytes` and `parse_line` in agreement about which
                // frames a line yields.
                //
                // ONE exception, and it is the case that made the cap
                // necessary: the login `<settings>` blob is legitimately
                // bigger than the cap -- VERIFIED at 513,700 bytes on one line
                // -- and is consumed as a region rather than parsed. Handing
                // it to `parse_line` opens that region and the rest of the
                // blob is then swallowed by the region, not misread as
                // markup, so there is nothing to truncate mid-tag. Without
                // this branch the blob became a MalformedTag on every login,
                // which is the cry-wolf failure the region exists to prevent.
                let raw = decode_wire_line(&self.pending);
                self.pending.clear();
                self.dropping_oversized_line = true;
                if raw.trim_start().starts_with("<settings") {
                    frames.extend(self.parse_line(&raw));
                    // The region now owns the rest of the line, so the tail
                    // is fed through rather than dropped.
                    self.dropping_oversized_line = false;
                } else {
                    frames.push(Frame::MalformedTag { raw });
                }
            }
        }
        frames
    }

    /// Bytes buffered awaiting a newline. Zero except mid-line.
    #[must_use]
    pub fn pending_len(&self) -> usize {
        self.pending.len()
    }
}
