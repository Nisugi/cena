//! Asking the human at the keyboard for four fields, and refusing to proceed
//! without one.
//!
//! Split from `main.rs` under `plan/05` Rule 4.1 -- move code down, do not
//! raise the cap -- when the review fixes pushed that file past 400 lines.
//! The seam is real: nothing here knows what a session is, and nothing here
//! touches the network.
//!
//! # The prompt ECHOES the password
//!
//! There is no terminal echo suppression: the password appears on screen as it
//! is typed and stays in scrollback. Suppressing it needs `rpassword` or raw
//! mode, which is the credential ladder `plan/12` §7.1 puts Out for M1. Said
//! plainly here because `main.rs` once claimed the opposite.

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
/// them. The sufficient check is the `is_terminal` refusal in [`ask`]. This
/// one stays because an empty answer at a real terminal is still worth
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
}

/// Ask for the four fields the login needs.
///
/// Split out of `main` under `plan/05` Rule 4.1 -- move code down, do not
/// raise the cap. Clippy caught `main` at 108 lines against a 100 limit.
pub fn ask() -> io::Result<Typed> {
    // **The keyboard must be a keyboard.**
    //
    // The non-empty checks below were described as making "the network
    // unreachable without someone at the keyboard". They do not: a pipe
    // supplying three non-empty lines passes every one of them and reaches a
    // real `A` authentication against the live service (review BI-1).
    //
    // An empty answer is evidence nobody is there; it is not the only such
    // evidence, and a non-empty one is not evidence anybody is. What actually
    // distinguishes the two cases is whether stdin is a terminal.
    //
    // This is the guard `CLAUDE.md` asks for -- "do not log into a live game
    // service without the author present" -- stated as a condition the program
    // can check rather than one it hopes for. It is deliberately a REFUSAL and
    // not a prompt: the headless credential ladder is `plan/12` §7.1's Out
    // column for M1, so there is no correct unattended path yet, and inventing
    // one here would be the wrong place for it.
    if !io::stdin().is_terminal() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "stdin is not a terminal, so nobody is at the keyboard. This \
             program reaches the LIVE login service and does not run \
             unattended (CLAUDE.md, Credentials). The headless credential \
             ladder is plan/12 section 7.1's Out column for M1; when it \
             exists, it -- not this prompt -- is what runs without a human.",
        ));
    }
    // `require`, not `prompt`: an empty answer to any of these means nobody is
    // at the keyboard, and this program reaches the live login service.
    let account = require("account")?;
    let password = require("password")?;
    let character = require("character")?;
    // Game codes are CASE-SENSITIVE on the wire. `M` lists them uppercase and
    // a lowercase code is not recognised -- but the failure is silent and
    // misleading: F still answers, about the account's DEFAULT instance, so
    // the login proceeds pointed at the wrong game and fails four commands
    // later at L. Uppercasing here is a convenience for the human at the
    // prompt; `cena_platform::eaccess` must NOT do it silently (`plan/10`
    // §4.4a), and does not -- it refuses at M with the offered list.
    let game_code = {
        let g = prompt("game code [GST]")?;
        let g = if g.is_empty() { "GST".to_owned() } else { g };
        let upper = g.to_ascii_uppercase();
        if upper != g {
            eprintln!("[input] game code {g:?} -> {upper:?} (codes are case-sensitive)");
        }
        upper
    };
    Ok(Typed {
        account,
        password,
        character,
        game_code,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// **`ask()` refuses when stdin is not a terminal.**
    ///
    /// A test harness runs with stdin redirected, so the test process is
    /// itself the unattended case this guard exists for: calling `ask()` here
    /// exercises the real refusal on the real condition, with no mocking.
    ///
    /// That also means the test can never accidentally reach the network. If
    /// the guard regresses, `ask()` blocks on a prompt instead and the test
    /// hangs rather than logging in -- a failure, and a safe one.
    #[test]
    fn asking_without_a_terminal_refuses_rather_than_prompting() {
        assert!(
            !io::stdin().is_terminal(),
            "this test asserts behaviour under a non-terminal stdin, and the \
             harness is supposed to provide one; if stdin IS a terminal here \
             the test proves nothing"
        );

        let Err(e) = ask() else {
            panic!(
                "ask() succeeded with no terminal attached. This program \
                 reaches the LIVE login service and CLAUDE.md forbids doing \
                 that unattended."
            )
        };
        assert_eq!(
            e.kind(),
            io::ErrorKind::InvalidInput,
            "the refusal must be a refusal, not an incidental read error: {e}"
        );
        let text = e.to_string();
        assert!(
            text.contains("not a terminal"),
            "the message must say WHY, so whoever hit it knows this is a \
             deliberate guard and not a broken prompt: {text}"
        );
    }
}
