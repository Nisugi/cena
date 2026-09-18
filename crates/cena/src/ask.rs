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

use std::io::{self, BufRead, Write};

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
/// that nobody is present. Three required fields now refuse it, so the
/// network is unreachable without someone at the keyboard.
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
/// Owned `String`s because [`Credentials`] borrows, and the borrow must
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
