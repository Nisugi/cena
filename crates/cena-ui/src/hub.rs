//! What a hub asks of whoever runs the sessions: start a character, quit
//! one, log a stopped one back in, shut Hydra down (`plan/29` step 5c); and,
//! from the window's launcher, log one in by what was typed, and keep the
//! roster and the kept passwords (`plan/49` Stage C).
//!
//! Two hubs ask it -- Despana's page and the window's (`plan/47` §4) -- and
//! one answerer, the binary, which alone knows the roster and the keyring.
//! Here so both name the same requests rather than each its own copy.

use std::fmt;
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

/// A request from a hub, for whoever runs the sessions.
///
/// A session is named by its number, `cena_session::SessionId`'s, which a
/// hub has from its card ([`SessionCard::session`](crate::SessionCard::session),
/// canonical decimal): this crate cannot name the session crate's type, and
/// each frontend parses at its own edge.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum HubRequest {
    /// Start this character: one the hub offered as available.
    Add(String),
    /// Quit this session and take it off the table.
    Remove(u32),
    /// Log this stopped session's character back in.
    Reconnect(u32),
    /// Shut Hydra down in order, as Ctrl-C does.
    Shutdown,
    /// Log a character in by what the player typed. Only the window's hub
    /// asks it: Despana's wire has no such message, so a password never
    /// crosses a socket.
    Login(Login),
    /// Take this character off the roster, named as the roster names it.
    Forget(String),
    /// Forget the password kept for this account.
    ForgetPassword(String),
    /// Star this roster character as a favourite, or unstar it.
    Favourite(String, bool),
}

/// A login typed whole, in the window's launcher.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Login {
    /// The account.
    pub account: String,
    /// Its password.
    pub password: Password,
    /// The game's code: `GS3` is Prime.
    pub game: String,
    /// The character, by name.
    pub character: String,
    /// Keep the password in the OS keyring once the login is proven.
    pub remember: bool,
}

/// A password, which says it is hidden when printed: a request is logged by
/// its `Debug`, and a password must never be.
#[derive(Clone, PartialEq, Eq)]
pub struct Password(String);

impl Password {
    /// `password`, to be kept hidden.
    #[must_use]
    pub fn new(password: String) -> Self {
        Self(password)
    }

    /// The password itself, for the login that needs it and nothing else.
    #[must_use]
    pub fn expose(&self) -> &str {
        &self.0
    }
}

impl fmt::Debug for Password {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("Password(hidden)")
    }
}

/// One character on the roster, as the window's launcher shows it: never a
/// password, only whether one is kept.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RosterCard {
    /// The character, as the player spells it.
    pub character: String,
    /// The account it is on.
    pub account: String,
    /// The game's code it logs in to.
    pub game: String,
    /// Whether its account's password is kept where Hydra can read it
    /// without asking: the keyring, or the account's variable.
    pub kept: bool,
    /// Whether the player starred it.
    pub favourite: bool,
}

/// What answers a hub's requests: the owner of the session table. It returns
/// one line for the hub that asked. A closure, not a trait: there is one
/// answerer.
pub type HubControl =
    Arc<dyn Fn(HubRequest) -> Pin<Box<dyn Future<Output = String> + Send>> + Send + Sync>;

#[cfg(test)]
mod tests {
    use super::*;

    /// A login printed -- as a request is, when logged -- never shows its
    /// password.
    #[test]
    fn a_password_never_prints() {
        let login = HubRequest::Login(Login {
            account: "ashryn01".to_owned(),
            password: Password::new("hunter2".to_owned()),
            game: "GS3".to_owned(),
            character: "Ashryn".to_owned(),
            remember: true,
        });
        let printed = format!("{login:?} {login:#?}");
        assert!(!printed.contains("hunter2"), "{printed}");
        assert!(printed.contains("Password(hidden)"));
        let HubRequest::Login(login) = login else {
            unreachable!()
        };
        assert_eq!(login.password.expose(), "hunter2");
    }
}
