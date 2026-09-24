//! Asking the human at the keyboard for what a login needs, and refusing to
//! proceed without it.
//!
//! Split from `main.rs` under `plan/05` Rule 4.1 -- move code down, do not
//! raise the cap -- when the review fixes pushed that file past 400 lines.
//! The seam is real: nothing here knows what a session is, and nothing here
//! touches the network.
//!
//! # The password is not asked for here
//!
//! It comes from the credential ladder (`crate::secrets`): the OS keyring,
//! then the account's environment variable, then a prompt that does not
//! echo. This prompt used to read the password with the other fields and
//! print it as it was typed, into scrollback.

use cena_platform::DEFAULT_GAME_CODE;
use std::io::{self, BufRead, IsTerminal, Write};

/// Read one line from stdin, without echoing a prompt into the transcript.
fn prompt(label: &str) -> io::Result<String> {
    print!("{label}: ");
    io::stdout().flush()?;
    let mut s = String::new();
    let read = io::stdin().lock().read_line(&mut s)?;
    // End of input means nobody is typing: a pipe, a redirect, or a closed
    // stdin. Refuse rather than carry on with an empty string.
    if read == 0 {
        return Err(io::Error::new(
            io::ErrorKind::UnexpectedEof,
            format!("stdin closed while asking for {label}"),
        ));
    }
    Ok(s.trim_end_matches(['\r', '\n']).to_owned())
}

/// Ask until a non-empty answer arrives, and **refuse to proceed without a
/// human**.
///
/// # This guard exists because it was needed
///
/// On 2026-09-18 `echo "" | cargo run -p cena` was run to check the prompt
/// sequence. Every prompt fell through in milliseconds on empty input, and
/// the program went straight to `eaccess.play.net` and completed a TLS
/// handshake and a `K` exchange against the **live** login service before it
/// could be stopped.
///
/// No credential was sent -- `K` is unauthenticated, and empty ones would
/// have been rejected at `A` -- but `CLAUDE.md` says do not touch a live game
/// service without the author present, and an empty-string account is proof
/// that nobody is present.
///
/// **This check is not sufficient on its own, and used to claim it was.** It
/// said three required fields made "the network unreachable without someone
/// at the keyboard"; a pipe supplying three non-empty lines passes all of
/// them. The sufficient check is the `is_terminal` refusal every prompt here
/// opens with. This one stays because an empty answer at a real terminal is still worth
/// refusing -- a slipped Enter should not become a login attempt.
///
/// A comment would not have prevented this; a refusal does.
fn require(label: &str) -> io::Result<String> {
    let value = prompt(label)?;
    if value.trim().is_empty() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!(
                "{label} is required, and an empty one means nobody is at the \
                 keyboard. This program reaches the LIVE login service; it \
                 does not run unattended."
            ),
        ));
    }
    Ok(value)
}

/// What the human at the prompt typed.
///
/// Owned `String`s because `cena_platform::Credentials` borrows, and the
/// borrow must
/// outlive the handshake. Nothing here reaches disk.
pub struct Typed {
    pub account: String,
    pub password: String,
    pub character: String,
    pub game_code: String,
    /// Which rung of the ladder the password came from.
    pub password_from: crate::secrets::Source,
}

/// Ask which character to play, when none was named with `--character`.
///
/// Everything else comes from where `--character` gets it: the roster for a
/// character that has logged in before, [`ask_for`] for one that has not.
/// There is one run path (`plan/30` §2); this only names its character.
///
/// # Errors
///
/// Nobody at a terminal, or an empty answer.
pub fn character() -> io::Result<String> {
    // **The keyboard must be a keyboard.**
    //
    // A pipe supplying non-empty lines passes every emptiness check and
    // reaches a real authentication against the live service (review BI-1).
    // What actually distinguishes a person from a pipe is whether stdin is a
    // terminal, so that is what is checked -- the guard `CLAUDE.md` asks for,
    // "do not log into a live game service without the author present",
    // stated as a condition the program can check.
    refuse_unattended(io::stdin().is_terminal())?;
    Ok(require("character")?.trim().to_owned())
}

/// Ask for what the roster does not know about `character` -- its account
/// and game code -- for its first login (`roster.rs`). The password comes
/// from the ladder (`crate::secrets`).
///
/// # Errors
///
/// Nobody at a terminal, an empty answer, or no password.
pub fn ask_for(character: &str) -> io::Result<Typed> {
    refuse_unattended(io::stdin().is_terminal())?;
    eprintln!("[login] {character} has not logged in through Hydra before.");
    let account = require(&format!("account for {character}"))?;
    let (password, password_from) = crate::secrets::password(account.trim(), true)?;
    let game_code = prompt(&format!("game code for {character} [{DEFAULT_GAME_CODE}]"))?;
    let mut typed = tidy(&account, password, character, &game_code);
    typed.password_from = password_from;
    Ok(typed)
}

/// The login for a character the roster knows: its account and game from the
/// roster, and its password from the ladder -- which prompts only when
/// `at_terminal` says a person is there to answer.
///
/// # Errors
///
/// No rung of the ladder had a password.
pub fn from_roster(entry: &crate::roster::Entry, at_terminal: bool) -> io::Result<Typed> {
    let (password, password_from) = crate::secrets::password(&entry.account, at_terminal)?;
    let mut typed = tidy(&entry.account, password, &entry.character, &entry.game_code);
    typed.password_from = password_from;
    Ok(typed)
}

/// The guard every prompt here opens with, split out so it can be tested on BOTH answers.
///
/// Its only input is whether stdin is a terminal, and a test cannot choose
/// that for its own process -- `cargo test` from a PowerShell prompt inherits
/// the console, and CI does not. So the one test that called `character()` asserted
/// a non-terminal first, and failed for a developer running the suite by hand
/// (review finding 15). Taking the bit as an argument lets both branches be
/// checked everywhere.
fn refuse_unattended(stdin_is_terminal: bool) -> io::Result<()> {
    if stdin_is_terminal {
        return Ok(());
    }
    Err(io::Error::new(
        io::ErrorKind::InvalidInput,
        "stdin is not a terminal, so nobody is at the keyboard. This \
         program reaches the LIVE login service and does not run \
         unattended (CLAUDE.md, Credentials). The headless credential \
         ladder is plan/12 section 7.1's Out column for M1; when it \
         exists, it -- not this prompt -- is what runs without a human.",
    ))
}

/// Clean up what was typed, **without touching the password**.
///
/// # Names are trimmed, because nothing downstream does it
///
/// A trailing space on the account or character name was sent as typed
/// (review finding 16). The `C` walk compares names with
/// `eq_ignore_ascii_case` and no trim (`cena-platform`'s `resolve_char_code`),
/// so `"Nisugi "` is not on the account -- a FATAL stop for a character that
/// exists. The account went to `A` with the space inside it, which the server
/// can only read as a different account -- a refusal, and plausibly a strike
/// (INFERRED: not something to test against the live service). Both are
/// invisible on screen. A name never legitimately begins or ends in whitespace.
///
/// # The password is NOT trimmed
///
/// Whitespace is a legal password byte, and the hash covers every byte
/// (`plan/10` §3.3). Trimming it would turn a correct password into a wrong
/// one -- the very failure trimming names prevents.
///
/// # Game codes are CASE-SENSITIVE on the wire
///
/// `M` lists them uppercase and a lowercase code is not recognised -- but the
/// failure is silent and misleading: F still answers, about the account's
/// DEFAULT instance, so the login proceeds pointed at the wrong game and fails
/// four commands later at L. Uppercasing here is a convenience for the human
/// at the prompt; `cena_platform::eaccess` must NOT do it silently (`plan/10`
/// §4.4a), and does not -- it refuses at M with the offered list.
fn tidy(account: &str, password: String, character: &str, game_code: &str) -> Typed {
    let g = game_code.trim();
    let g = if g.is_empty() { DEFAULT_GAME_CODE } else { g };
    let upper = g.to_ascii_uppercase();
    if upper != g {
        eprintln!("[input] game code {g:?} -> {upper:?} (codes are case-sensitive)");
    }
    Typed {
        account: account.trim().to_owned(),
        password,
        character: character.trim().to_owned(),
        game_code: upper,
        password_from: crate::secrets::Source::Prompt,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// **`character()` refuses when stdin is not a terminal.**
    ///
    /// A test harness usually runs with stdin redirected, so the test process
    /// is itself the unattended case this guard exists for: calling `character()`
    /// here exercises the real refusal on the real condition, with no mocking.
    ///
    /// That also means the test can never accidentally reach the network. If
    /// the guard regresses, `character()` blocks on a prompt instead and the test
    /// hangs rather than logging in -- a failure, and a safe one.
    ///
    /// **Skipped when stdin IS a terminal**, which is `cargo test` typed at a
    /// PowerShell prompt. It used to assert the opposite and fail there (review
    /// finding 15); calling `character()` instead would block on a real prompt. The
    /// guard's two answers are checked on every machine by the test below.
    #[test]
    fn asking_without_a_terminal_refuses_rather_than_prompting() {
        if io::stdin().is_terminal() {
            eprintln!(
                "skipped: stdin is a terminal here, so character() would prompt. \
                 refuse_unattended's own test covers the guard."
            );
            return;
        }

        let Err(e) = character() else {
            panic!(
                "character() succeeded with no terminal attached. This program \
                 reaches the LIVE login service and CLAUDE.md forbids doing \
                 that unattended."
            )
        };
        assert_eq!(
            e.kind(),
            io::ErrorKind::InvalidInput,
            "the refusal must be a refusal, not an incidental read error: {e}"
        );
    }

    #[test]
    fn the_unattended_guard_refuses_a_pipe_and_admits_a_terminal() {
        let e = refuse_unattended(false).expect_err("a pipe is nobody");
        assert_eq!(e.kind(), io::ErrorKind::InvalidInput);
        let text = e.to_string();
        assert!(
            text.contains("not a terminal"),
            "the message must say WHY, so whoever hit it knows this is a \
             deliberate guard and not a broken prompt: {text}"
        );
        assert!(refuse_unattended(true).is_ok());
    }

    #[test]
    fn names_are_trimmed_and_the_password_is_not() {
        // Finding 16: `"Nisugi "` is not on the account, so the `C` walk
        // stops FATAL on a character that exists.
        let typed = tidy(" someacct\t", " pass word ".to_owned(), "Nisugi ", " gs3 ");
        assert_eq!(typed.account, "someacct");
        assert_eq!(typed.character, "Nisugi");
        assert_eq!(
            typed.password, " pass word ",
            "whitespace is a legal password byte; trimming it sends a wrong \
             password and costs a strike"
        );
        assert_eq!(typed.game_code, "GS3");
    }

    #[test]
    fn an_empty_game_code_is_the_default() {
        let typed = tidy("a", "p".to_owned(), "c", "  ");
        assert_eq!(typed.game_code, "GST");
    }
}
