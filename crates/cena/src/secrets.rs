//! Where a login password comes from: the credential ladder (`plan/29` Q2,
//! author, 2026-09-23 -- *"we absolutely want keyring for account login
//! information"*).
//!
//! Per **account**, never per character: a password belongs to the account,
//! and several characters share one.
//!
//! 1. **The OS keyring** -- Windows Credential Manager, the macOS Keychain,
//!    Secret Service on Linux -- under service [`KEYRING_SERVICE`], keyed by
//!    the account in lowercase. `VellumFE`'s scheme
//!    (`reference/VellumFE/src/config/profiles.rs`, `keyring_entry`).
//! 2. **An environment variable the user set**, [`env_name`]: for headless
//!    runs on a machine with no keyring.
//! 3. **A prompt that does not echo**, only when a person is at a terminal.
//!    It replaced a prompt that printed the password as it was typed.
//! 4. **Otherwise a refusal that names the account**, never a silent skip
//!    that would look like a connection failure.
//!
//! Hydra writes a password nowhere on its own initiative. A typed one is
//! offered to the keyring only after it logged in, and only if the person
//! says yes ([`offer_to_remember`]), so a mistyped password is never kept.

use std::io::{self, BufRead, Write};

use cena_session::{Event, SessionObserver, State};

/// The keyring's service name: the "folder" Hydra's entries appear under.
pub(crate) const KEYRING_SERVICE: &str = "hydra";

/// Which rung a password came from.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Source {
    /// The OS keyring.
    Keyring,
    /// The account's environment variable.
    Env,
    /// Typed at the prompt; a candidate for [`offer_to_remember`].
    Prompt,
}

/// The environment variable that holds `account`'s password:
/// `CENA_PASSWORD_` and the account, uppercased, with anything that is not
/// a letter or digit as `_` -- an account name is not always a legal
/// variable name.
pub(crate) fn env_name(account: &str) -> String {
    let tail: String = account
        .trim()
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() {
                c.to_ascii_uppercase()
            } else {
                '_'
            }
        })
        .collect();
    format!("CENA_PASSWORD_{tail}")
}

/// The ladder's order, with every rung passed in -- so it can be tested
/// without a keyring, an environment or a keyboard.
///
/// # Errors
///
/// When no rung has a password: the refusal names the account, and says
/// which rungs were tried.
pub(crate) fn choose(
    account: &str,
    keyring: Option<String>,
    env: Option<String>,
    at_terminal: bool,
    prompt: impl FnOnce() -> io::Result<String>,
) -> io::Result<(String, Source)> {
    let usable = |p: &String| !p.is_empty();
    if let Some(password) = keyring.filter(usable) {
        return Ok((password, Source::Keyring));
    }
    if let Some(password) = env.filter(usable) {
        return Ok((password, Source::Env));
    }
    if at_terminal {
        let typed = prompt()?;
        if !typed.is_empty() {
            return Ok((typed, Source::Prompt));
        }
    }
    Err(io::Error::new(
        io::ErrorKind::NotFound,
        format!(
            "no password for account {account}: none in the OS keyring, {} is not set, \
             and {}. Hydra reaches the LIVE login service and does not guess.",
            env_name(account),
            if at_terminal {
                "none was typed"
            } else {
                "nobody is at a terminal to type one"
            }
        ),
    ))
}

/// The password for `account`, from the first rung that has one.
///
/// # Errors
///
/// As [`choose`]: no rung had one, or the prompt could not be read.
pub(crate) fn password(account: &str, at_terminal: bool) -> io::Result<(String, Source)> {
    choose(
        account,
        from_keyring(account),
        std::env::var(env_name(account)).ok(),
        at_terminal,
        || rpassword::prompt_password(format!("password for {account} (not shown): ")),
    )
}

fn entry(account: &str) -> keyring::Result<keyring::Entry> {
    keyring::Entry::new(KEYRING_SERVICE, &account.trim().to_lowercase())
}

/// The keyring's password for `account`, if it has one. Best-effort: a
/// machine with no keyring backend is the ordinary headless case, not an
/// error, and the ladder moves on.
fn from_keyring(account: &str) -> Option<String> {
    match entry(account).and_then(|entry| entry.get_password()) {
        Ok(password) => Some(password),
        Err(keyring::Error::NoEntry) => None,
        Err(e) => {
            eprintln!("[login] the OS keyring could not be read ({e}); trying the next source");
            None
        }
    }
}

/// Once `observer`'s session is `Ready` -- so the password is known to be
/// right -- ask whether to keep it in the keyring, and do so on a yes.
///
/// Only for a typed password: one that came from the keyring is already
/// there, and one from the environment is the user's own arrangement.
/// Default no.
///
/// The answer is read on a plain detached thread, **not `spawn_blocking`**:
/// the runtime waits for blocking tasks when it shuts down, so an unanswered
/// question would hold the process open after Ctrl-C had already stopped the
/// session. A detached thread ends with the process.
///
/// `turn` is shared by every session's offer, so when several characters
/// log in at once their questions are asked one at a time rather than
/// talking over each other on one terminal.
pub(crate) async fn offer_to_remember(
    account: String,
    password: String,
    observer: SessionObserver,
    turn: std::sync::Arc<std::sync::Mutex<()>>,
) {
    let Ok((snapshot, mut events)) = observer.subscribe().await else {
        return;
    };
    if snapshot.lifecycle != State::Ready {
        loop {
            match events.recv().await {
                Ok(o) if o.event == Event::StateChanged(State::Ready) => break,
                Ok(_) | Err(tokio::sync::broadcast::error::RecvError::Lagged(_)) => {}
                Err(tokio::sync::broadcast::error::RecvError::Closed) => return,
            }
        }
    }
    std::thread::spawn(move || {
        let _turn = turn
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        print!("Save the password for {account} in the OS keyring? [y/N]: ");
        let _ = io::stdout().flush();
        let mut answer = String::new();
        let _ = io::stdin().lock().read_line(&mut answer);
        let said = if answer.trim().eq_ignore_ascii_case("y") {
            match entry(&account).and_then(|entry| entry.set_password(&password)) {
                Ok(()) => format!("[login] saved to the OS keyring for {account}"),
                Err(e) => format!("[login] the OS keyring would not save it ({e})"),
            }
        } else {
            format!("[login] not saved; {account} will be asked for again next time")
        };
        eprintln!("{said}");
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    fn never_asked() -> io::Result<String> {
        panic!("the prompt must not be reached");
    }

    #[test]
    fn the_keyring_comes_first_then_the_environment_then_the_prompt() {
        let got = choose(
            "acct",
            Some("k".into()),
            Some("e".into()),
            true,
            never_asked,
        );
        assert_eq!(got.ok(), Some(("k".to_owned(), Source::Keyring)));

        let got = choose("acct", None, Some("e".into()), true, never_asked);
        assert_eq!(got.ok(), Some(("e".to_owned(), Source::Env)));

        let got = choose("acct", None, None, true, || Ok("typed".into()));
        assert_eq!(got.ok(), Some(("typed".to_owned(), Source::Prompt)));
    }

    #[test]
    fn an_empty_entry_is_no_entry() {
        // A blank variable or keyring entry would otherwise be sent to the
        // live service as a password.
        let got = choose(
            "acct",
            Some(String::new()),
            Some(String::new()),
            true,
            || Ok("typed".into()),
        );
        assert_eq!(got.ok(), Some(("typed".to_owned(), Source::Prompt)));
    }

    #[test]
    fn with_nothing_and_nobody_at_a_terminal_it_refuses_naming_the_account() {
        let Err(e) = choose("MyAcct", None, None, false, never_asked) else {
            panic!("a password was invented");
        };
        let said = e.to_string();
        assert!(said.contains("MyAcct"), "{said}");
        assert!(said.contains("CENA_PASSWORD_MYACCT"), "{said}");

        // At a terminal, an empty answer is refused too.
        assert!(choose("MyAcct", None, None, true, || Ok(String::new())).is_err());
    }

    #[test]
    fn the_variable_name_is_legal_whatever_the_account_is_called() {
        assert_eq!(env_name("nisugi"), "CENA_PASSWORD_NISUGI");
        assert_eq!(env_name(" my-acct.2 "), "CENA_PASSWORD_MY_ACCT_2");
    }
}
