//! Every process Hydra starts for a player's scripts: the script runner,
//! the script checker and the player's own Lich (`plan/46`, `plan/51`).
//!
//! **None of them is handed a password.** Each runs code Hydra did not
//! write: any script can read its environment (`ENV[...]` in Ruby). The
//! password ladder's environment rung, `CENA_PASSWORD_<ACCOUNT>`
//! (`crates/cena/src/secrets.rs`), lives in Hydra's own environment, which a
//! child inherits unless told otherwise; [`command`] removes every variable
//! of that family before anything else is set. The rest of the environment
//! (`PATH`, Ruby's and the system's own variables) is inherited as before.

use std::ffi::OsStr;

/// What every password variable's name starts with: `CENA_PASSWORD_` and
/// the account (`crates/cena/src/secrets.rs`, `env_name`).
pub const PASSWORD_PREFIX: &str = "CENA_PASSWORD_";

/// A command for `program` whose environment is Hydra's without a password
/// in it: every variable named [`PASSWORD_PREFIX`] and more, matched without
/// regard to case, as Windows matches a variable's name.
#[must_use]
pub fn command(program: impl AsRef<OsStr>) -> tokio::process::Command {
    let mut command = tokio::process::Command::new(program);
    for (name, _) in std::env::vars_os() {
        if is_password(&name) {
            command.env_remove(name);
        }
    }
    command
}

/// Whether the variable `name` holds a password.
fn is_password(name: &OsStr) -> bool {
    name.to_str().is_some_and(|name| {
        name.get(..PASSWORD_PREFIX.len())
            .is_some_and(|start| start.eq_ignore_ascii_case(PASSWORD_PREFIX))
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A variable set in the process that runs this test again; never in the
    /// one `cargo test` started, which has none.
    const PROBE: &str = "CENA_PASSWORD_PROBE";
    /// Not a password: inherited, to show the child has Hydra's environment.
    const KEPT: &str = "HYDRA_PROBE_KEPT";

    #[test]
    fn a_password_variable_is_known_by_its_name() {
        assert!(is_password(OsStr::new("CENA_PASSWORD_NISUGI")));
        assert!(is_password(OsStr::new("cena_password_nisugi")));
        assert!(!is_password(OsStr::new("CENA_DATA_DIR")));
        assert!(!is_password(OsStr::new("PATH")));
    }

    /// A child started through [`command`] has Hydra's environment, less its
    /// password. Rust 2024 makes setting a variable in this process unsafe,
    /// so this test runs itself again in a process that has one, and that
    /// process starts Ruby and asks what it sees.
    #[tokio::test]
    async fn a_child_is_never_handed_a_password() {
        if std::env::var_os(PROBE).is_none() {
            let me = std::env::current_exe().unwrap();
            let ran = std::process::Command::new(me)
                .args([
                    "--exact",
                    "child::tests::a_child_is_never_handed_a_password",
                    "--nocapture",
                ])
                .env(PROBE, "hunter2")
                .env(KEPT, "yes")
                .output()
                .unwrap();
            let said = String::from_utf8_lossy(&ran.stdout);
            let why = String::from_utf8_lossy(&ran.stderr);
            assert!(ran.status.success(), "{said}{why}");
            assert!(said.contains("1 passed"), "the inner run ran: {said}");
            return;
        }
        let ruby = crate::scripts::runner::find_ruby().expect("Ruby, which CI installs");
        let seen = command(ruby)
            .args(["-e", "print ENV.keys.join(\"\\n\")"])
            .output()
            .await
            .unwrap();
        let names = String::from_utf8_lossy(&seen.stdout);
        let names: Vec<&str> = names.lines().collect();
        assert!(names.contains(&KEPT), "the rest is inherited: {names:?}");
        assert!(
            !names.iter().any(|name| is_password(OsStr::new(name))),
            "{names:?}"
        );
    }
}
