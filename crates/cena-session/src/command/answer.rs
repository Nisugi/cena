//! What answers a line: **the round trip ends at the answer, not at the
//! first prompt** (`plan/12` §4.4, amended 2026-10-01).
//!
//! The game sends a prompt after everything it says, asked for or not (the
//! author: *"A prompt is it's way of saying 'over'"*). A creature walking in
//! ends in a prompt exactly as the reply to `fire` does, so a window that
//! closed at the first prompt after its line closed on whatever the game
//! said first, and the caller decided from a state the reply had not reached
//! (the author's hunt of 2026-09-30: `fire` three times in 170 ms).
//!
//! So a line can name what answers it ([`Answers`]), as Lich's
//! `dothistimeout` names its pattern and eohunter's `send_and_match` its
//! matches. The window then stays open past prompts until one of the main
//! window's lines answers it, and closes at the prompt after that line: the
//! game's "over" for the reply. Only the game's own lines answer: a line a
//! person said is not one (`ChunkLine::is_spoken`'s rule, the crate review of
//! 2026-10-01). A refusal ([`refusal`]) answers any line.
//!
//! A line whose answer does not come within [`WAIT`] closes unanswered
//! ([`Outcome::Timeout`](super::Outcome::Timeout), which never meant "it did
//! not happen"), and is **owed** its answer for [`OWED_FOR`]: the first line
//! that would have answered it is taken as its, and not offered to the line
//! sent after it. A late answer given to the earlier line costs the caller a
//! decision made again; taken by the later line, it would answer a line the
//! game has not answered yet.

use std::time::Duration;

/// Whether one of the game's lines, its markup removed and trimmed, answers
/// the line sent. A plain function, so a send stays `Copy` and the queue
/// needs no allocation for it; a caller with several kinds of line names one
/// function per kind.
pub type Answers = fn(&str) -> bool;

/// How long a line waits for its answer before its window closes
/// unanswered.
pub const WAIT: Duration = Duration::from_secs(3);

/// How long a line whose window closed unanswered stays owed its answer.
pub const OWED_FOR: Duration = Duration::from_secs(10);

/// What answers any line: the game saying no, or not yet.
const REFUSALS: &[&str] = &[
    "...wait ",
    "Wait ",
    "Sorry, you may only type ahead",
    "You can't",
    "You cannot",
    "You are unable",
    "You are still stunned",
    "You are too",
    "You are not",
    "You aren't",
    "You're ",
    "You don't",
    "You do not",
    "You must",
    "You need",
    "You have no",
    "You haven't",
    "You currently have no valid target",
    "You struggle",
    "But you",
    "What were you referring",
    "I could not find",
    "Could not find",
    "That is not",
    "It looks like somebody",
    "Be at peace",
    "Please rephrase",
    "Usage:",
];

/// Whether `line` is the game refusing a line, which answers whatever was
/// sent.
#[must_use]
pub fn refusal(line: &str) -> bool {
    let line = line.trim();
    REFUSALS.iter().any(|no| line.starts_with(no))
}

/// Whether `line` answers a line answered by `answers`: its own answer or a
/// refusal.
#[must_use]
pub fn answered_by(answers: Answers, line: &str) -> bool {
    let line = line.trim();
    refusal(line) || answers(line)
}

/// The game held the line back: how long to wait before it goes again.
/// `None`: `line` is not a holding.
///
/// `...wait N seconds.` says N whole seconds, rounded up, so about half a
/// second less is left; bigshot's `bs_put` waits N less one and Lich's
/// `fput` N (`global_defs.rb:1556`). `Sorry, you may only type ahead 1
/// command.` is the line arriving on top of another.
#[must_use]
pub fn again_after(line: &str) -> Option<Duration> {
    let line = line.trim();
    if line.starts_with("Sorry, you may only type ahead") {
        return Some(Duration::from_secs(1));
    }
    let rest = line
        .strip_prefix("...wait ")
        .or_else(|| line.strip_prefix("Wait "))?;
    let digits: String = rest.chars().take_while(char::is_ascii_digit).collect();
    let seconds: u64 = digits.parse().ok()?;
    Some(Duration::from_millis(
        (seconds.min(30) * 1000).saturating_sub(500).max(250),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn roundtime(line: &str) -> bool {
        line.starts_with("Roundtime: ")
    }

    #[test]
    fn a_line_is_answered_by_its_own_answer_or_a_refusal() {
        assert!(answered_by(roundtime, "Roundtime: 3 sec."));
        assert!(answered_by(roundtime, "  ...wait 1 seconds."));
        assert!(!answered_by(
            roundtime,
            "A behemothic gorefrost golem just went through an arch."
        ));
    }

    #[test]
    fn held_back_is_waited_out_and_anything_else_is_not() {
        assert_eq!(
            again_after("...wait 4 seconds."),
            Some(Duration::from_millis(3500))
        );
        assert_eq!(
            again_after("...wait 1 seconds."),
            Some(Duration::from_millis(500))
        );
        assert_eq!(
            again_after("Sorry, you may only type ahead 1 command."),
            Some(Duration::from_secs(1))
        );
        assert_eq!(again_after("Roundtime: 4 sec."), None);
    }
}
