//! Shared streams, merged across characters (`plan/29` §5a R4).
//!
//! The author's rules, 2026-09-23:
//!
//! - **The same line is a duplicate line.** A thought on a channel reaches
//!   every listening character verbatim, and so does another player's speech
//!   in a room two characters share. Identical text on the same stream from
//!   two sessions is one line.
//! - **Within one second**: *"you can give it a 1s max matching window"*. A
//!   repeat after that is a new line, as is a repeat from the same character.
//! - **Tagged by character**, so the line says whose it was.
//! - **Which streams**: thoughts, speech, logons, deaths and announcements. No
//!   other stream merges; more are revisited only if asked for.
//!
//! Pure: the caller supplies the clock, so the window is testable and this
//! crate stays free of a runtime.

use std::collections::VecDeque;
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};

use crate::view::{StoryLine, StyledRun};

/// The streams that merge. Their ids are the game's, MEASURED in `plan/15`
/// (the stream-declaration table): `thoughts`, `speech`, `logons`, `death`,
/// `announcements`.
pub const MERGED_STREAMS: &[&str] = &["thoughts", "speech", "logons", "death", "announcements"];

/// How far apart two arrivals may be and still be one occurrence (author).
pub const MATCH_WINDOW: Duration = Duration::from_secs(1);

/// One merged line: its text once, and every character that received it.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct MergedLine {
    /// Canonical decimal id. A line sent again with the same id is the same
    /// line gaining a character, not a new one.
    pub id: String,
    /// The game's stream id: one of [`MERGED_STREAMS`].
    pub stream: String,
    /// The text, styled as the first character received it.
    pub runs: Vec<StyledRun>,
    /// Who received it, in arrival order: each character's tag.
    pub from: Vec<String>,
}

/// A line seen recently enough that another character's copy may still
/// arrive.
#[derive(Debug)]
struct Recent {
    at: Instant,
    text: String,
    sessions: Vec<String>,
    line: MergedLine,
}

/// Merges shared-stream lines across sessions. See the module docs.
#[derive(Debug, Default)]
pub struct Merger {
    next: u64,
    recent: VecDeque<Recent>,
}

impl Merger {
    /// A merger with nothing seen.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Offer one line, received at `now` by `session` -- tagged `tag` -- and
    /// get back what to show: a new line, or an earlier one gaining this
    /// character. `None` when the line's stream does not merge.
    pub fn offer(
        &mut self,
        now: Instant,
        session: &str,
        tag: &str,
        line: &StoryLine,
    ) -> Option<MergedLine> {
        if !MERGED_STREAMS.contains(&line.stream.as_str()) {
            return None;
        }
        while self
            .recent
            .front()
            .is_some_and(|r| now.saturating_duration_since(r.at) > MATCH_WINDOW)
        {
            self.recent.pop_front();
        }
        let text: String = line.runs.iter().map(|run| run.text.as_str()).collect();
        let same = self.recent.iter_mut().find(|r| {
            r.line.stream == line.stream
                && r.text == text
                && !r.sessions.iter().any(|s| s == session)
        });
        if let Some(seen) = same {
            seen.sessions.push(session.to_owned());
            seen.line.from.push(tag.to_owned());
            return Some(seen.line.clone());
        }
        let merged = MergedLine {
            id: self.next.to_string(),
            stream: line.stream.clone(),
            runs: line.runs.clone(),
            from: vec![tag.to_owned()],
        };
        self.next += 1;
        self.recent.push_back(Recent {
            at: now,
            text,
            sessions: vec![session.to_owned()],
            line: merged.clone(),
        });
        Some(merged)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::view::Closed;

    fn line(stream: &str, text: &str) -> StoryLine {
        StoryLine {
            stream: stream.to_owned(),
            runs: vec![StyledRun {
                text: text.to_owned(),
                bold: false,
                monospace: false,
                preset: None,
            }],
            truncated: false,
            closed: Closed::Main,
        }
    }

    #[test]
    fn one_thought_heard_by_two_characters_is_one_line_tagged_twice() {
        let mut merger = Merger::new();
        let at = Instant::now();
        let thought = line("thoughts", "[General] Someone: hello");
        let first = merger.offer(at, "0", "Nisugi", &thought).unwrap();
        let second = merger
            .offer(at + Duration::from_millis(300), "1", "Nerten", &thought)
            .unwrap();
        assert_eq!(first.id, second.id, "the same line, gaining a character");
        assert_eq!(second.from, ["Nisugi", "Nerten"]);
    }

    #[test]
    fn after_the_window_or_from_the_same_character_it_is_a_new_line() {
        let mut merger = Merger::new();
        let at = Instant::now();
        let said = line("speech", "Someone says, \"hi\"");
        let first = merger.offer(at, "0", "Nisugi", &said).unwrap();
        // The same character hearing it again is a repeat, not a duplicate.
        let again = merger
            .offer(at + Duration::from_millis(100), "0", "Nisugi", &said)
            .unwrap();
        assert_ne!(first.id, again.id);
        // Another character, but past the 1 s window: a new occurrence.
        let late = merger
            .offer(at + Duration::from_millis(1_200), "1", "Nerten", &said)
            .unwrap();
        assert_ne!(late.id, first.id);
        assert_eq!(late.from, ["Nerten"]);
    }

    #[test]
    fn only_the_shared_streams_merge_and_only_identical_text() {
        let mut merger = Merger::new();
        let at = Instant::now();
        assert!(
            merger
                .offer(at, "0", "Nisugi", &line("main", "You look around."))
                .is_none()
        );
        assert!(
            merger
                .offer(at, "0", "Nisugi", &line("inv", "a sword"))
                .is_none()
        );
        // Own speech reads differently to each character: kept as two lines.
        let own = merger
            .offer(at, "0", "Nisugi", &line("speech", "You say, \"hi\""))
            .unwrap();
        let heard = merger
            .offer(at, "1", "Nerten", &line("speech", "Nisugi says, \"hi\""))
            .unwrap();
        assert_ne!(own.id, heard.id);
        // The same text on two different streams is two lines.
        let a = merger
            .offer(at, "0", "Nisugi", &line("logons", "X arrives."))
            .unwrap();
        let b = merger
            .offer(at, "1", "Nerten", &line("death", "X arrives."))
            .unwrap();
        assert_ne!(a.id, b.id);
    }
}
