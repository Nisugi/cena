//! Which line of the game's is the answer to a line the hunt sent.
//!
//! The game sends a prompt after everything it says, asked for or not: a
//! creature walking in ends in a prompt exactly as the reply to `fire` does.
//! The driver took the first prompt after its line for the answer, so a
//! creature arriving in that tenth of a second ended the round trip before
//! the reply had come. The next tick decided from a state the reply had not
//! reached, and sent the line again; the roundtime the reply carried was not
//! known when the following step went out (the author's hunt of 2026-09-30:
//! `fire` three times in 170 ms, `raise longbow` 2 ms after `weapon volley`).
//!
//! So an answer is **a line this command is known to be answered with**, as
//! bigshot's `cmd_*` handlers and eohunter's `send_and_match` have it
//! (`eohunter/scripts/eohunter/actions.rb`, `engage.rb:610`, `:701`,
//! `combat.rb:155-167`, `loot.rb:117`): every known answer, refusals
//! included. The driver reads on past a prompt until one is heard, or
//! [`WAIT`] passes.
//!
//! | The line sent | Answered by |
//! |---|---|
//! | `target …` | `You are now targeting`, `You can't target`, the two `discern` lines |
//! | `stance …` | a `You …` line naming a stance |
//! | an attack, a maneuver, `hide`, `assume`, `skin`, `search` | a roundtime line |
//! | `incant`, `cast`, `evoke`, `channel` | a roundtime line, a fizzle, a spell not known |
//! | `prepare`, `prep` | the spell ready, or one readied already |
//! | `loot …` | the search's own lines |
//! | anything | a refusal ([`REFUSALS`]) |
//!
//! A line with no row (`store weapon`, `raise longbow`, a profile's own
//! words) is answered by the first prompt, as before: nothing says what the
//! game answers it with.
//!
//! # A roundtime line is the game's own (the crate review of 2026-10-01, BE-A-2)
//!
//! Anchored at the start of the line, as Lich's `Spell#cast` anchors it
//! (`lib/common/spell.rb:36`, `/^(?:Cast|Sing) Roundtime [0-9]+ Seconds?\.$/`;
//! `:773`, `/^Roundtime: \d+ sec.$/`). The committed fixtures hold these
//! forms and no other (`grep -rhoE "[^<]{0,40}Roundtime[^<]{0,30}"
//! crates/cena-behavior/tests/fixtures`): `Roundtime: 3 sec.`, `Cast
//! Roundtime 1 Second.`, `Sing Roundtime 3 Seconds.`, each at the start of
//! its line; and `Roundtime changed to 1 second.`, Rapid Fire's addendum
//! the line after `Roundtime: 3 sec.` (`arch_kill.xml:20-21`), which is not
//! an answer: taken as one, it would answer the next line too.
//! `contains("Roundtime")` let another player's speech answer an attack:
//! speech starts with the speaker's name, so it cannot start with these.
//!
//! # The refusals that mean "again" (Lich's `fput`, `global_defs.rb:1556`)
//!
//! `...wait N seconds.` and `Wait N sec` are the game holding the line for
//! roundtime the client did not know of: the wire gives roundtime in whole
//! seconds, so its last fraction is not knowable, and Rapid Fire shortens it
//! after the fact. `Sorry, you may only type ahead 1 command.` is the line
//! arriving on top of another. Both are waited out and the line sent again
//! ([`again_after`]), by the driver alone, a bounded number of times
//! (`hunt/drive/send.rs`; the machine no longer does, the crate review of
//! 2026-10-01, BE-A-11).
//!
//! A holding is the answer to whichever line the game read last, which is
//! not always the line just sent: a reply that came later than [`WAIT`]
//! lands in the next line's reading. So a line whose answer was not heard is
//! **owed** one, and the next answer-shaped line the game says is taken as
//! its ([`settles`]) before anything is read for the line now awaited (BE-A-3).
//! The asymmetry is deliberate: a holding wrongly given to an earlier line
//! costs a resend (the next tick decides again from the state); a holding
//! wrongly taken as this line's sends a line twice that went through.

use std::time::Duration;

/// How long the driver reads on for a line's answer before it gives up and
/// lets the tick decide.
pub(super) const WAIT: Duration = Duration::from_secs(3);

/// What answers a line, whatever it is: the game saying no.
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

/// The kinds of line the hunt knows the answer to.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Kind {
    Target,
    Stance,
    /// Anything that ends in roundtime.
    Round,
    Cast,
    Prepare,
    Loot,
}

/// The game's roundtime lines (the module's doc for the forms).
fn round(line: &str) -> bool {
    line.starts_with("Roundtime: ")
}

fn cast_round(line: &str) -> bool {
    round(line) || line.starts_with("Cast Roundtime ") || line.starts_with("Sing Roundtime ")
}

fn refusal(line: &str) -> bool {
    REFUSALS.iter().any(|no| line.starts_with(no))
}

/// The words whose answer is a roundtime line.
const ROUND: &[&str] = &[
    "attack", "kill", "fire", "ambush", "hurl", "jab", "punch", "kick", "grapple", "mstrike",
    "cman", "weapon", "shield", "feat", "warcry", "smite", "throw", "hide", "assume", "skin",
    "search", "stomp",
];

fn kind(sent: &str) -> Option<Kind> {
    let first = sent.split_whitespace().next()?.to_ascii_lowercase();
    Some(match first.as_str() {
        "target" => Kind::Target,
        "stance" => Kind::Stance,
        "incant" | "cast" | "evoke" | "channel" => Kind::Cast,
        "prepare" | "prep" => Kind::Prepare,
        "loot" => Kind::Loot,
        word if ROUND.contains(&word) => Kind::Round,
        _ => return None,
    })
}

/// Whether `heard`, the main window's lines since `sent` went out, holds
/// its answer.
#[must_use]
pub(super) fn heard(sent: &str, heard: &str) -> bool {
    let Some(kind) = kind(sent) else {
        return true;
    };
    heard
        .lines()
        .map(str::trim)
        .any(|line| refusal(line) || answers(kind, line))
}

/// Whether `line` settles `sent`, an earlier line still owed its answer:
/// its own answer or a refusal; for a line with no row, a refusal alone,
/// since nothing says what else answers it.
#[must_use]
pub(super) fn settles(sent: &str, line: &str) -> bool {
    let line = line.trim();
    refusal(line) || kind(sent).is_some_and(|kind| answers(kind, line))
}

/// Whether the hunt knows what answers `sent` (a row in the module's table).
#[must_use]
pub(super) fn known(sent: &str) -> bool {
    kind(sent).is_some()
}

/// One trimmed line, against a kind's own answers (refusals aside).
fn answers(kind: Kind, line: &str) -> bool {
    match kind {
        Kind::Target => {
            line.starts_with("You are now targeting")
                || line.starts_with("You discern that you are the origin")
        }
        Kind::Stance => line.starts_with("You ") && line.contains("stance"),
        Kind::Round => round(line),
        Kind::Cast => {
            cast_round(line)
                || line.starts_with("Your magic fizzles")
                || line.starts_with("Cast at what?")
                || line.ends_with("leaving you casting at nothing but thin air!")
        }
        Kind::Prepare => {
            line.ends_with(" is ready.") || line.starts_with("You already have a spell readied")
        }
        Kind::Loot => {
            [
                "You search",
                "You find",
                "You gather",
                "You rummage",
                "You discover",
            ]
            .iter()
            .any(|did| line.starts_with(did))
                || line.starts_with("There is nothing")
                || line.starts_with("There was nothing")
                || line.starts_with("There is no loot")
                || line.starts_with("Nothing to loot")
        }
    }
}

/// The game held the line back: how long to wait before it goes again.
/// `None`: it was not held.
///
/// `...wait N seconds.` says N whole seconds, rounded up, so about half a
/// second less is left; bigshot's `bs_put` waits N less one and Lich's
/// `fput` N.
#[must_use]
pub(super) fn again_after(heard: &str) -> Option<Duration> {
    heard.lines().map(str::trim).find_map(|line| {
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
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The author's hunt of 2026-09-30: a golem left the room between the
    /// `fire` and its reply, and its prompt was taken for the answer.
    #[test]
    fn a_creature_leaving_does_not_answer_fire() {
        let leaving =
            "A behemothic gorefrost golem just went through a rune-carved white granite arch.\n";
        assert!(!heard("fire #583850503", leaving));
        let reply = format!(
            "{leaving}You take aim and fire a faewood arrow at a grim gigas skald!\nRoundtime: 4 sec.\n"
        );
        assert!(heard("fire #583850503", &reply));
    }

    #[test]
    fn each_kind_is_answered_by_its_own_line_and_by_a_refusal() {
        let arriving = "A niveous giant warg pads in, deadly quiet despite its great size.\n";
        for (sent, answer) in [
            ("target #1", "You are now targeting a niveous giant warg."),
            ("stance offensive", "You are now in an offensive stance."),
            ("weapon volley #1", "Roundtime: 3 sec."),
            ("incant 515", "Cast Roundtime 0 Seconds."),
            ("incant 9708", "That is not something you can prepare."),
            ("prep 650", "Your spell is ready."),
            ("loot #1", "You search the giant warg."),
            ("loot #1", "...wait 1 seconds."),
            (
                "fire #1",
                "You currently have no valid target.  You will need to specify one.",
            ),
        ] {
            assert!(!heard(sent, arriving), "{sent}");
            assert!(heard(sent, &format!("{arriving}{answer}\n")), "{sent}");
        }
    }

    /// The crate review of 2026-10-01, BE-A-2: speech reaches the main
    /// window, and a word in it is not the game's roundtime.
    #[test]
    fn roundtime_in_someone_elses_words_answers_nothing() {
        for said in [
            "Bob says, \"Roundtime is brutal here.\"",
            "You say, \"Cast Roundtime 3 Seconds.\"",
            "Bob exclaims, \"Roundtime: 4 sec.\"",
        ] {
            assert!(!heard("fire #1", &format!("{said}\n")), "{said}");
            assert!(!heard("incant 515", &format!("{said}\n")), "{said}");
        }
        for (sent, line) in [
            ("fire #1", "Roundtime: 3 sec."),
            ("incant 515", "Cast Roundtime 1 Second."),
            ("incant 1003", "Sing Roundtime 3 Seconds."),
            ("incant 515", "Roundtime: 3 sec."),
        ] {
            assert!(heard(sent, &format!("{line}\n")), "{sent}: {line}");
        }
        assert!(!heard("fire #1", "Cast Roundtime 1 Second.\n"));
        assert!(
            !heard("fire #1", "Roundtime changed to 1 second.\n"),
            "an addendum"
        );
    }

    #[test]
    fn an_owed_line_is_settled_by_its_answer_or_a_refusal() {
        assert!(settles("fire #1", "Roundtime: 3 sec."));
        assert!(settles("fire #1", "...wait 1 seconds."));
        assert!(!settles("fire #1", "A golem just went through an arch."));
        assert!(!settles("target #1", "Roundtime: 3 sec."));
        assert!(settles("raise longbow", "  ...wait 1 seconds."));
        assert!(!settles("raise longbow", "You raise a longbow."));
        assert!(known("fire #1"));
        assert!(!known("raise longbow"));
    }

    #[test]
    fn a_line_with_no_known_answer_is_answered_by_anything() {
        assert_eq!(kind("raise longbow"), None);
        assert!(heard("raise longbow", ""));
        assert_eq!(kind("FIRE #1"), Some(Kind::Round));
    }

    #[test]
    fn held_back_is_waited_out_and_anything_else_is_not() {
        assert_eq!(
            again_after("You nock an arrow.\n...wait 4 seconds.\n"),
            Some(Duration::from_millis(3500))
        );
        assert_eq!(
            again_after("...wait 1 seconds.\n"),
            Some(Duration::from_millis(500))
        );
        assert_eq!(
            again_after("Sorry, you may only type ahead 1 command.\n"),
            Some(Duration::from_secs(1))
        );
        assert_eq!(again_after("Roundtime: 4 sec.\n"), None);
    }
}
