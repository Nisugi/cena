//! `;keys import` (`plan/52` step 9): a Wrayth key set into this
//! character's keys, or every character's. Typed in a play window and done
//! there, since the keys are the window's and a character running headless
//! has none; answered in the window's Hydra pane.

use std::path::Path;
use std::sync::Arc;

use cena_session::{Body, Notice, NoticeKind};

use super::App;
use crate::keys::page::{KeyChange, Place};
use crate::keys::{Whose, wrayth, write};
use crate::sessions::{Seat, lock};

/// What `;keys` says when it is not given `import` and a file.
const USAGE: &str = "keys import <Wrayth settings file>: its key sets into this character's keys; keys import global <file>: into every character's.";

impl App {
    /// `line`, typed on `seat`'s window, done here when it is `;keys`
    /// (with the character's own command symbol): whether it was.
    pub(super) fn keys_command(&mut self, seat: &Arc<Seat>, line: &str) -> bool {
        let symbol = seat
            .handle
            .command_symbol()
            .unwrap_or(cena_session::command::claimant::DEFAULT_SYMBOL);
        let Some(rest) = line.trim().strip_prefix(symbol) else {
            return false;
        };
        let mut words = rest.trim_start().splitn(2, char::is_whitespace);
        if !words
            .next()
            .is_some_and(|word| word.eq_ignore_ascii_case("keys"))
        {
            return false;
        }
        let rest = words.next().unwrap_or_default().trim();
        let said = match rest.strip_prefix("import") {
            Some(file) if !file.trim().is_empty() => {
                let file = file.trim();
                let (every, file) = match file.strip_prefix("global") {
                    Some(after) if after.starts_with(char::is_whitespace) => (true, after.trim()),
                    _ => (false, file),
                };
                self.import(seat, Path::new(file.trim_matches('"')), every)
            }
            _ => vec![USAGE.to_owned()],
        };
        let kind = if said.len() > 1 {
            NoticeKind::Warn
        } else {
            NoticeKind::Info
        };
        lock(&seat.story).tell(Notice {
            kind,
            body: Body::Lines(said),
        });
        true
    }

    /// The Wrayth key sets in `file` into `seat`'s character's keys, or
    /// every character's (`every`); what was done, a line at a time.
    fn import(&mut self, seat: &Arc<Seat>, file: &Path, every: bool) -> Vec<String> {
        let text = match std::fs::read_to_string(file) {
            Ok(text) => text,
            Err(why) => return vec![format!("Keys: {} could not be read: {why}", file.display())],
        };
        let (imported, mut said) = wrayth::read(&text);
        let data = self.keys_file.as_deref().and_then(Path::parent);
        let (path, whose, whom) = if every {
            (
                self.keys_file.clone(),
                Whose::Every,
                "every character's keys".to_owned(),
            )
        } else {
            (
                self.mine_path(&seat.game, &seat.name),
                Whose::Character,
                format!("{}'s keys", seat.name),
            )
        };
        let (Some(data), Some(path)) = (data, path) else {
            return vec!["Keys: nothing is kept here to import into.".to_owned()];
        };
        // A key already doing what it would is left as it is.
        let (mut changes, mut already, mut moved) = (Vec::new(), 0, Vec::new());
        for key in &imported {
            if key.set == 0 && self.keys.beneath(whose, &key.chord) == Some(&key.does) {
                already += 1;
                continue;
            }
            if key.set == 0 && self.keys.beneath(whose, &key.chord).is_some() {
                moved.push(key.chord.written());
            }
            changes.push(KeyChange::Bind {
                key: key.chord.written(),
                does: key.does.clone(),
                was: None,
                place: Place {
                    set: key.set,
                    every,
                },
            });
        }
        if let Err(why) = write::apply_all(data, (&path, whose), &changes, &self.keys) {
            return vec![format!("Keys: nothing was imported: {why}")];
        }
        self.read_keys();
        let name = file
            .file_name()
            .map_or_else(String::new, |name| name.to_string_lossy().into_owned());
        let mut lines = vec![format!(
            "Keys: {} imported from {name} into {whom}; {already} already so.",
            changes.len()
        )];
        if !moved.is_empty() {
            lines.push(format!(
                "These now do what Wrayth had them do, not Hydra's: {}. Restore one on the Keys page for Hydra's.",
                moved.join(", ")
            ));
        }
        if !said.is_empty() {
            lines.push("Not imported:".to_owned());
            lines.append(&mut said);
        }
        lines
    }
}
