//! What the window's Not launched tab asks of the binary (`plan/49` Stage C),
//! answered, each with one line for the hub: the roster kept, a password
//! forgotten, an account's characters listed. Out of `play.rs`, which holds
//! the session table and sat at its cap.

use std::path::Path;

use cena_ui::{Account, Listing, Saved};

use crate::{roster, secrets};

/// Take the character `name` means off the roster.
pub(crate) fn forget(dir: &Path, name: &str) -> String {
    match roster::forget(dir, name) {
        Ok(Some(entry)) => format!("{} is off the roster.", entry.character),
        Ok(None) => format!("{name} is not on the roster."),
        Err(e) => format!("The roster could not be changed: {e}"),
    }
}

/// Forget the password the OS keyring keeps for `account`.
pub(crate) fn forget_password(account: &str) -> String {
    match secrets::forget(account) {
        // The keyring's is gone; the account's variable is the player's own,
        // and still answers.
        Ok(()) if secrets::saved(account) => format!(
            "The OS keyring keeps no password for {account}; {} still holds one.",
            secrets::env_name(account)
        ),
        Ok(()) => format!("The password kept for {account} is forgotten."),
        Err(e) => format!("The OS keyring would not forget it: {e}"),
    }
}

/// Star the character `name` means, or unstar it.
pub(crate) fn favourite(dir: &Path, name: &str, star: bool) -> String {
    match roster::favourite(dir, name, star) {
        Ok(Some(entry)) if star => format!("{} is a favourite.", entry.character),
        Ok(Some(entry)) => format!("{} is no longer a favourite.", entry.character),
        Ok(None) => format!("{name} is not on the roster."),
        Err(e) => format!("The roster could not be changed: {e}"),
    }
}

/// Put `saved` on the roster without playing it: a character an account's
/// listing named, so its account and game are the service's.
pub(crate) fn remember(dir: &Path, saved: &Saved) -> String {
    let entry = roster::Entry {
        character: saved.character.trim().to_owned(),
        account: saved.account.trim().to_owned(),
        game_code: saved.game.trim().to_owned(),
        favourite: saved.favourite,
    };
    let character = entry.character.clone();
    match roster::record(dir, entry) {
        Ok(()) => format!("{character} is on the roster."),
        Err(e) => format!("The roster could not be changed: {e}"),
    }
}

/// Ask the login service which characters `account` has on its game, and
/// hand the window the list. Its password goes to the OS keyring only when
/// the player ticked the box, and only now the service has proved it.
pub(crate) async fn characters(
    pin: &Path,
    account: Account,
    gui: Option<&cena_gui::Sessions>,
) -> String {
    let (name, game) = (account.account.trim(), account.game.trim());
    let credentials = cena_platform::Credentials {
        account: name,
        password: account.password.expose(),
        character: "",
        game_code: game,
    };
    let listed = cena_platform::list_characters(credentials, pin, |line| {
        eprintln!("[login] {line}");
    })
    .await;
    let characters = match listed {
        Ok(characters) => characters,
        Err(e) => return format!("{name}'s characters could not be listed: {e}"),
    };
    let kept = if account.remember {
        match secrets::keep(name, account.password.expose()) {
            Ok(()) => " Its password is kept.".to_owned(),
            Err(e) => format!(" The OS keyring would not keep its password ({e})."),
        }
    } else {
        String::new()
    };
    let said = match characters.len() {
        1 => format!("{name} has one character on {game}.{kept}"),
        n => format!("{name} has {n} characters on {game}.{kept}"),
    };
    if let Some(gui) = gui {
        gui.characters(Listing {
            account: name.to_owned(),
            game: game.to_owned(),
            characters,
        });
    }
    said
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scratch(test: &str) -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!("cena-launcher-{test}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        dir
    }

    fn saved(favourite: bool) -> Saved {
        Saved {
            character: " Newt ".to_owned(),
            account: "newacct".to_owned(),
            game: "GSF".to_owned(),
            favourite,
        }
    }

    /// A listed character added is on the roster with its account and game,
    /// starred when added by its star; a later login keeps the star; and
    /// each answer names the character, not the roster's key.
    #[test]
    fn a_listed_character_is_remembered_starred_and_forgotten() {
        let dir = scratch("remember");
        assert_eq!(remember(&dir, &saved(true)), "Newt is on the roster.");
        let entry = roster::find(&dir, "GSF:Newt").ok().flatten();
        assert_eq!(
            entry.as_ref().map(|e| (e.account.as_str(), e.favourite)),
            Some(("newacct", true))
        );
        assert_eq!(remember(&dir, &saved(false)), "Newt is on the roster.");
        assert_eq!(
            roster::find(&dir, "GSF:Newt")
                .ok()
                .flatten()
                .map(|e| e.favourite),
            Some(true),
            "adding again does not unstar it"
        );
        assert_eq!(
            favourite(&dir, "GSF:Newt", false),
            "Newt is no longer a favourite."
        );
        assert_eq!(favourite(&dir, "GSF:Newt", true), "Newt is a favourite.");
        assert_eq!(forget(&dir, "GSF:Newt"), "Newt is off the roster.");
        assert_eq!(forget(&dir, "GSF:Newt"), "GSF:Newt is not on the roster.");
        assert_eq!(
            favourite(&dir, "GSF:Newt", true),
            "GSF:Newt is not on the roster."
        );
        let _ = std::fs::remove_dir_all(&dir);
    }
}
