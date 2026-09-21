//! `Routine::GuildPassword`: a rogue guild's door (9 exits).
//!
//! Upstream (`recognise/routines.rs`, `PASSWORD`): `lean door`, then each verb
//! of `UserVars.rogue_password` -- split on commas -- on the door, then `go
//! door`. The exit is priced only for a profile that has the password, so
//! finding none here is a map newer than the profile, and fails.

use super::{Next, Seen, Solver};

#[derive(Default)]
pub(super) struct GuildPassword {
    /// What is still to send, the next first. `None` until the first turn.
    left: Option<Vec<String>>,
    /// `go door` has been sent.
    gone: bool,
}

impl Solver for GuildPassword {
    fn next(&mut self, seen: &Seen<'_>) -> Next {
        if self.left.is_none() {
            let Some(password) = seen
                .walker
                .settings
                .get("rogue_password")
                .filter(|password| !password.trim().is_empty())
            else {
                return Next::Failed;
            };
            let mut all = vec!["lean door".to_owned()];
            all.extend(
                password
                    .split(',')
                    .map(str::trim)
                    .filter(|verb| !verb.is_empty())
                    .map(|verb| format!("{verb} door")),
            );
            self.left = Some(all);
        }
        let left = self.left.get_or_insert_with(Vec::new);
        if left.is_empty() {
            // Upstream `fput`s the last of it too; a move is what it is.
            if std::mem::replace(&mut self.gone, true) {
                return Next::Done;
            }
            return Next::Go("go door".to_owned());
        }
        Next::Put(left.remove(0))
    }
}

#[cfg(test)]
mod tests {
    use super::super::testing::Scene;
    use super::*;

    #[test]
    fn the_door_is_leant_on_worked_and_gone_through() {
        let mut scene = Scene::at(1, 2);
        scene
            .walker
            .settings
            .insert("rogue_password".into(), "kick, slap,turn".into());
        let mut door = GuildPassword::default();
        let sent: Vec<Next> = (0..5).map(|_| scene.ask(&mut door)).collect();
        assert_eq!(
            sent,
            [
                Next::Put("lean door".into()),
                Next::Put("kick door".into()),
                Next::Put("slap door".into()),
                Next::Put("turn door".into()),
                Next::Go("go door".into()),
            ]
        );
        assert_eq!(Scene::at(2, 2).ask(&mut door), Next::Done);
    }

    #[test]
    fn no_password_is_no_way_through() {
        let mut door = GuildPassword::default();
        assert_eq!(Scene::at(1, 2).ask(&mut door), Next::Failed);
    }
}
