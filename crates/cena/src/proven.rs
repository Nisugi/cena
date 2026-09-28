//! What a login leaves behind once it is proven `Ready`: its roster entry,
//! and a password to keep. Moved out of `play.rs` when the M7 and GUI lines
//! met there and took it past its cap (`plan/05` Rule 4.4: move code down).

use std::path::Path;
use std::sync::Arc;

use cena_session::{Event, SessionObserver, State};

use crate::ask::Typed;
use crate::{roster, secrets};

/// What a login leaves behind once it is proven: its roster entry, and --
/// for a typed password -- the offer to keep it in the keyring. Taken from
/// the login before it is handed to the connector, started once the session
/// exists.
pub(crate) struct Proven {
    entry: roster::Entry,
    typed_password: Option<(String, String)>,
    /// A password typed in the window's launcher with its box ticked: kept
    /// in the keyring once the login is proven, without a question.
    kept: Option<(String, String)>,
}

impl Proven {
    pub(crate) fn of(typed: &Typed) -> Self {
        Self {
            entry: roster::Entry::of(typed),
            typed_password: (typed.password_from == secrets::Source::Prompt)
                .then(|| (typed.account.clone(), typed.password.clone())),
            kept: (typed.password_from == secrets::Source::Window { keep: true })
                .then(|| (typed.account.clone(), typed.password.clone())),
        }
    }

    /// When `observer`'s login reaches `Ready`: record the roster entry, keep
    /// a password the window's box asked to keep, and offer one typed at the
    /// terminal to the keyring, one question at a time (`turn`). Each change
    /// is word to `changed`.
    pub(crate) fn on_ready(
        self,
        observer: &SessionObserver,
        turn: &Arc<std::sync::Mutex<()>>,
        changed: &Arc<tokio::sync::Notify>,
    ) {
        if let Some((account, password)) = self.typed_password {
            tokio::spawn(secrets::offer_to_remember(
                account,
                password,
                observer.clone(),
                Arc::clone(turn),
                Arc::clone(changed),
            ));
        }
        let observer = observer.clone();
        let (entry, kept, changed) = (self.entry, self.kept, Arc::clone(changed));
        tokio::spawn(async move {
            if until_ready(&observer).await {
                remember(&cena_session::character_store::data_dir(), entry);
                if let Some((account, password)) = kept {
                    match secrets::keep(&account, &password) {
                        Ok(()) => eprintln!("[login] saved to the OS keyring for {account}"),
                        Err(e) => eprintln!("[login] the OS keyring would not save it ({e})"),
                    }
                }
                changed.notify_one();
            }
        });
    }
}

/// Record `entry` in the roster, saying so if it cannot be.
fn remember(dir: &Path, entry: roster::Entry) {
    let character = entry.character.clone();
    if let Err(e) = roster::record(dir, entry) {
        eprintln!(
            "[{character}] could not be added to the roster ({e}); `--character {character}` will ask again"
        );
    }
}

/// Resolves `true` once `observer`'s session is `Ready`; `false` if it ended
/// first.
async fn until_ready(observer: &SessionObserver) -> bool {
    let Ok((snapshot, mut events)) = observer.subscribe().await else {
        return false;
    };
    if snapshot.lifecycle == State::Ready {
        return true;
    }
    loop {
        match events.recv().await {
            Ok(o) if o.event == Event::StateChanged(State::Ready) => return true,
            Ok(_) | Err(tokio::sync::broadcast::error::RecvError::Lagged(_)) => {}
            Err(tokio::sync::broadcast::error::RecvError::Closed) => return false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A password typed in the window is kept once proven only when its box
    /// was ticked, and is never asked about at the terminal.
    #[test]
    fn a_window_password_is_kept_only_when_its_box_was_ticked() {
        let login = |remember| cena_ui::Login {
            account: "acct".to_owned(),
            password: cena_ui::Password::new("pw".to_owned()),
            game: "gst".to_owned(),
            character: "Ashryn".to_owned(),
            remember,
        };
        let ticked = Proven::of(&crate::ask::from_window(&login(true)));
        assert_eq!(ticked.kept, Some(("acct".to_owned(), "pw".to_owned())));
        assert_eq!(ticked.typed_password, None, "never asked at the terminal");
        let unticked = Proven::of(&crate::ask::from_window(&login(false)));
        assert_eq!((unticked.kept, unticked.typed_password), (None, None));
    }
}
