//! Which line of the game's answers a line the hunt sent: what the hunt
//! hands the session as the line's [`Answers`], so its round trip ends at
//! that answer and not at the first prompt (`cena_session::command::answer`,
//! `plan/12` §4.4 as amended 2026-10-01).
//!
//! The game sends a prompt after everything it says, asked for or not: a
//! creature walking in ends in a prompt exactly as the reply to `fire` does
//! (the author's hunt of 2026-09-30: `fire` three times in 170 ms, `raise
//! longbow` 2 ms after `weapon volley`). So an answer is **a line this
//! command is known to be answered with**, as bigshot's `cmd_*` handlers and
//! eohunter's `send_and_match` have it (`eohunter/scripts/eohunter/
//! actions.rb`, `engage.rb:610`, `:701`, `combat.rb:155-167`, `loot.rb:117`).
//! The session adds the refusals, which answer any line, and keeps a line's
//! late answer its own.
//!
//! | The line sent | Answered by |
//! |---|---|
//! | `target …` | `You are now targeting`, the `discern` line (and a refusal: `You can't target`) |
//! | `stance …` | a `You …` line naming a stance |
//! | an attack, a maneuver, `hide`, `assume`, `skin`, `search` | a roundtime line |
//! | `incant`, `cast`, `evoke`, `channel` | a roundtime line, a fizzle, a spell not known |
//! | `prepare`, `prep` | the spell ready, or one readied already |
//! | `loot …` | the search's own lines |
//!
//! A line with no row (`store weapon`, `raise longbow`, a profile's own
//! words) is answered by the first prompt: nothing says what the game
//! answers it with.
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

use cena_session::command::answer::Answers;

/// What answers `sent`, or `None` for a line with no row in the module's
/// table: the first prompt ends its round trip.
#[must_use]
pub(super) fn answers(sent: &str) -> Option<Answers> {
    let first = sent.split_whitespace().next()?.to_ascii_lowercase();
    Some(match first.as_str() {
        "target" => target,
        "stance" => stance,
        "incant" | "cast" | "evoke" | "channel" => cast,
        "prepare" | "prep" => prepare,
        "loot" => loot,
        word if ROUND.contains(&word) => round,
        _ => return None,
    })
}

/// The words whose answer is a roundtime line.
const ROUND: &[&str] = &[
    "attack", "kill", "fire", "ambush", "hurl", "jab", "punch", "kick", "grapple", "mstrike",
    "cman", "weapon", "shield", "feat", "warcry", "smite", "throw", "hide", "assume", "skin",
    "search", "stomp",
];

fn round(line: &str) -> bool {
    line.starts_with("Roundtime: ")
}

fn target(line: &str) -> bool {
    line.starts_with("You are now targeting")
        || line.starts_with("You discern that you are the origin")
}

fn stance(line: &str) -> bool {
    line.starts_with("You ") && line.contains("stance")
}

fn cast(line: &str) -> bool {
    round(line)
        || line.starts_with("Cast Roundtime ")
        || line.starts_with("Sing Roundtime ")
        || line.starts_with("Your magic fizzles")
        || line.starts_with("Cast at what?")
        || line.ends_with("leaving you casting at nothing but thin air!")
}

fn prepare(line: &str) -> bool {
    line.ends_with(" is ready.") || line.starts_with("You already have a spell readied")
}

fn loot(line: &str) -> bool {
    [
        "You search",
        "You find",
        "You gather",
        "You rummage",
        "You discover",
        "There is nothing",
        "There was nothing",
        "There is no loot",
        "Nothing to loot",
    ]
    .iter()
    .any(|did| line.starts_with(did))
}

#[cfg(test)]
mod tests {
    use super::*;
    use cena_session::command::answer::answered_by;

    fn heard(sent: &str, line: &str) -> bool {
        answers(sent).is_some_and(|answers| answered_by(answers, line))
    }

    /// The author's hunt of 2026-09-30: a golem left the room between the
    /// `fire` and its reply.
    #[test]
    fn a_creature_leaving_does_not_answer_fire() {
        let leaving =
            "A behemothic gorefrost golem just went through a rune-carved white granite arch.";
        assert!(!heard("fire #583850503", leaving));
        assert!(heard("fire #583850503", "Roundtime: 4 sec."));
    }

    #[test]
    fn each_kind_is_answered_by_its_own_line_and_by_a_refusal() {
        let arriving = "A niveous giant warg pads in, deadly quiet despite its great size.";
        for (sent, answer) in [
            ("target #1", "You are now targeting a niveous giant warg."),
            ("target #1", "You can't target that."),
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
            assert!(heard(sent, answer), "{sent}: {answer}");
        }
    }

    /// The crate review of 2026-10-01, BE-A-2: a word in someone's speech is
    /// not the game's roundtime.
    #[test]
    fn roundtime_in_someone_elses_words_answers_nothing() {
        for said in [
            "Bob says, \"Roundtime is brutal here.\"",
            "You say, \"Cast Roundtime 3 Seconds.\"",
            "Bob exclaims, \"Roundtime: 4 sec.\"",
        ] {
            assert!(!heard("fire #1", said), "{said}");
            assert!(!heard("incant 515", said), "{said}");
        }
        for (sent, line) in [
            ("fire #1", "Roundtime: 3 sec."),
            ("incant 515", "Cast Roundtime 1 Second."),
            ("incant 1003", "Sing Roundtime 3 Seconds."),
            ("incant 515", "Roundtime: 3 sec."),
        ] {
            assert!(heard(sent, line), "{sent}: {line}");
        }
        assert!(!heard("fire #1", "Cast Roundtime 1 Second."));
        assert!(
            !heard("fire #1", "Roundtime changed to 1 second."),
            "an addendum"
        );
    }

    #[test]
    fn a_line_with_no_row_names_no_answer() {
        assert!(answers("raise longbow").is_none());
        assert!(answers("FIRE #1").is_some());
    }
}
