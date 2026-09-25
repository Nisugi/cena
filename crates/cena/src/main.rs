//! The `cena` binary: Hydra.
//!
//! One run path. `--character A --character B` names the characters to play,
//! `--record` / `--no-record` says whether combat and loot go to the
//! character's database (`setup::recording`), which `;loot` and `;combat`
//! report on;
//! with none named, Hydra asks for one at the terminal. Either way they go on
//! the session table (`play.rs`), with the web hub under `--web`.
//!
//! **How the whole workspace fits together is [`architecture`]**: the crate
//! graph, the flow of one line of game text, the three seams, and the tests
//! that hold each in place. Read it first. **The words it uses are
//! [`glossary`]**: one per concept, each linked to the item it names.
//!
//! This was M1's end-to-end slice, run for real: a single session narrated
//! criterion by criterion, a looping `look` behind `--demo`, and measurement
//! probes behind `--capture`, `--psm` and `--typeahead`. M6 retired all of it
//! (author, 2026-09-24: *"get rid of any test behaviors, like look"*; of the
//! probes, *"delete them"*). The criteria it demonstrated are still tested,
//! against a scripted game: `cena-behavior/tests/stop_and_interleave.rs` and
//! `authority_is_released.rs`, on the same looping behavior, now test support.
//!
//! # This reaches the live game
//!
//! `CLAUDE.md`: "do not log into a live game service without the author
//! present." No test in this workspace reaches the network. Only the author
//! runs this binary.
//!
//! # Credentials
//!
//! The password comes from the credential ladder (`secrets.rs`): the OS
//! keyring, the account's environment variable, or a prompt that does not
//! echo. It is never logged, and the types that hold it redact themselves.
//! It **is** kept in memory for the session's life, because every reconnect
//! is a full re-login (`LiveConnector`). The wire is written to disk, with
//! the credential redacted (`Redactions`).

mod architecture;
mod ask;
mod combat;
mod commands;
mod connector;
mod frontend;
mod glossary;
mod hunt;
mod interrupt;
mod learn;
mod loot;
mod play;
mod roster;
mod secrets;
mod setup;
mod sorter;
mod travel;
mod watch;

/// What the operator is told before anything happens.
///
/// A function rather than eight lines in `main` because it has its own reason to
/// change -- what is true about credentials and logging -- and because it has
/// been WRONG TWICE, each time claiming less was kept than actually was:
///
/// 1. "Not stored" became false at M2 step 8: reconnecting is a full re-login,
///    so the password stays for the session.
/// 2. "Nothing is written to disk" was false from the moment the session sink
///    landed. This banner said it while the same `main` printed two file paths
///    eight lines later. A reassurance the next screenful contradicts is worse
///    than no reassurance: it is the kind of claim someone acts on.
///
/// **Hydra**, not `cena`: `CLAUDE.md` says anything user-facing carries the real
/// name, and the working name is the repository, the crate prefixes and the env
/// vars.
fn banner() {
    eprintln!("Hydra -- your characters, one process, against the live game.");
    eprintln!("{BANNER_NOTE}");
}

/// The banner's second paragraph, a constant so a test can read it.
///
/// Two sentences on two lines, then a blank one. It had collapsed to "for the
/// (ten spaces) session," -- a `\` continuation whose backslash and newline
/// were lost and whose next-line indent was kept, so the operator read a gap
/// mid-sentence (review finding 14).
const BANNER_NOTE: &str = "The password is never logged, but it IS kept in memory for the \
     session,\nbecause reconnecting re-logs in. The wire IS written to disk -- \
     see the\nlog paths below.\n";

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    banner();
    let mut characters = play::characters();
    if characters.is_empty() {
        characters.push(ask::character()?);
    }
    Box::pin(play::play(characters)).await
}

#[cfg(test)]
mod tests {
    use super::BANNER_NOTE;

    /// The banner reads as two sentences, not as a sentence with a hole in it.
    ///
    /// Review finding 14: a lost `\` continuation left its next-line indent in
    /// the string, and the operator saw "for the          session,".
    #[test]
    fn the_banner_has_no_collapsed_gap() {
        assert!(!BANNER_NOTE.contains("  "), "{BANNER_NOTE:?}");
        assert!(BANNER_NOTE.contains(
            "for the session,
because"
        ));
    }
}
