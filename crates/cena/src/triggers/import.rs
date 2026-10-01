//! `;trigger import <path>`: a Wrayth settings file (`plan/45` Stage 4), or
//! another player's Hydra triggers file (`plan/54` step 5), into the
//! triggers file, then what could not come. The reading and the merge are
//! `cena_behavior::triggers::wrayth`, `edit::shared` and `edit::import`; this
//! says what they did.
//!
//! **An import that brings commands asks first** (the author, 2026-09-29:
//! *"a popup indicating it contains the commands, list the commands, offer
//! accept all, accept one, cancel"*). With a window open, nothing is written
//! until the player answers there ([`answer`]); without one, the import goes
//! on with every command held, each named with how to approve it. A Wrayth
//! file brings none: its macros are keys, not triggers.

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, MutexGuard, PoisonError};

use cena_behavior::triggers::edit::{self, Merged};
use cena_behavior::triggers::wrayth;
use cena_session::{NoticeKind, SessionHandle};

use super::{Changes, Said, change, counted_as, triggers};

/// `;trigger import <path>`: a Wrayth settings file's highlights, names and
/// ignores into the triggers file, then what could not come.
pub(super) fn import(
    handle: &SessionHandle,
    dir: &Path,
    character: &str,
    others: &Changes,
    path: &Path,
) -> Said {
    let named = path.file_name().map_or_else(
        || path.display().to_string(),
        |file| file.to_string_lossy().into_owned(),
    );
    let hydra = path
        .extension()
        .is_some_and(|x| x.eq_ignore_ascii_case("toml"));
    let origin = if hydra {
        format!("Shared: {named}")
    } else {
        format!("Wrayth: {named}")
    };
    let brought = match fs::read_to_string(path)
        .map_err(|e| e.to_string())
        .and_then(|text| {
            if hydra {
                edit::shared(&text, &origin)
            } else {
                wrayth::read(&text, &origin)
            }
        }) {
        Ok(brought) => brought,
        Err(why) => return vec![(NoticeKind::Error, format!("{}: {why}", path.display()))],
    };
    let sends = sends(&brought);
    if !sends.is_empty()
        && let Some(asked) = ask(dir, others, &named, &origin, &brought, &sends)
    {
        return vec![(NoticeKind::Info, asked)];
    }
    let mut said = change(handle, dir, character, others, &|text| {
        let (text, merged) = edit::import(text, &origin, &brought)?;
        Ok((text, imported(&named, &brought, &merged)))
    });
    if said.iter().any(|(kind, _)| *kind == NoticeKind::Error) {
        return said;
    }
    if brought.sounds > 0 {
        said.push((
            NoticeKind::Info,
            format!(
                "{} play a sound, kept as its file name and played from {}: a path the file \
                 gave is never followed.",
                counted_as(brought.sounds, "trigger"),
                crate::attention::sounds_dir(dir).display()
            ),
        ));
    }
    said.extend(
        brought
            .notes
            .iter()
            .map(|note| (NoticeKind::Warn, format!("{note}."))),
    );
    // No window to ask in: every command came in held, and is named.
    said.extend(sends.iter().map(|(name, line)| {
        (
            NoticeKind::Warn,
            format!("`{name}` would send \"{line}\": held until `;trigger approve {name}`."),
        )
    }));
    said
}

/// The imports waiting on the player's answer, and the window to ask in.
#[derive(Default)]
pub(crate) struct Imports {
    /// Where to ask; none, and an import does not wait.
    pub(super) window: Option<cena_gui::Sessions>,
    /// Each import asked about and not yet answered, by its number.
    waiting: BTreeMap<u64, Waiting>,
    /// The next import's number.
    next: u64,
}

/// An import held until the player answers.
struct Waiting {
    dir: PathBuf,
    file: String,
    origin: String,
    brought: wrayth::Import,
}

/// The imports, locked; a panic elsewhere does not lose them.
pub(super) fn lock(imports: &Mutex<Imports>) -> MutexGuard<'_, Imports> {
    imports.lock().unwrap_or_else(PoisonError::into_inner)
}

/// Each trigger an import brings that sends, and what it sends.
fn sends(brought: &wrayth::Import) -> Vec<(String, String)> {
    brought
        .triggers
        .iter()
        .filter_map(|(name, table)| Some((name.clone(), table.get("send")?.as_str()?.to_owned())))
        .collect()
}

/// Hold the import and ask the player about its commands in the window,
/// and say so; `None` when there is no window to ask in.
fn ask(
    dir: &Path,
    others: &Changes,
    file: &str,
    origin: &str,
    brought: &wrayth::Import,
    sends: &[(String, String)],
) -> Option<String> {
    let mut imports = lock(&others.imports);
    let window = imports.window.clone()?;
    let id = imports.next;
    imports.next += 1;
    imports.waiting.insert(
        id,
        Waiting {
            dir: dir.to_owned(),
            file: file.to_owned(),
            origin: origin.to_owned(),
            brought: brought.clone(),
        },
    );
    drop(imports);
    window.ask_import(cena_ui::triggers::ImportQuestion {
        id,
        file: file.to_owned(),
        triggers: brought.triggers.len(),
        sends: sends.to_vec(),
    });
    Some(format!(
        "{file} brings {} that send commands: nothing is imported until you answer in the window.",
        counted_as(sends.len(), "trigger")
    ))
}

/// The player's answer to import `id`: import it, approving the commands
/// of the triggers `accept` names (renamed as the import renamed them), or,
/// with `None`, import nothing. Told to every running character.
pub(crate) fn answer(others: &Changes, id: u64, accept: Option<Vec<String>>) -> String {
    let Some(waiting) = lock(&others.imports).waiting.remove(&id) else {
        return "Triggers: that import was already answered.".to_owned();
    };
    let Some(accept) = accept else {
        return format!("Triggers: {} not imported.", waiting.file);
    };
    let path = triggers::path(&waiting.dir);
    let made = cena_session::store::changing(&path, || {
        let old = super::file(&waiting.dir)?;
        let (mut text, merged) = edit::import(&old, &waiting.origin, &waiting.brought)?;
        for name in &accept {
            let given = merged
                .renamed
                .iter()
                .find(|(had, _)| had == name)
                .map_or(name.as_str(), |(_, given)| given.as_str());
            // The import refused it under the name it gave it (BE-F-7).
            if merged.refused.iter().any(|refused| refused.name == given) {
                continue;
            }
            text = edit::approve(&text, given)?.0;
        }
        super::write(&waiting.dir, &text)?;
        Ok::<String, String>(format!(
            "{} imported, {} approved",
            imported(&waiting.file, &waiting.brought, &merged),
            counted_as(accept.len(), "command")
        ))
    });
    match made {
        Ok(done) => {
            others.tell("the Triggers window", &done);
            format!("Triggers: {done}; every running character reads it again.")
        }
        Err(why) => format!("Triggers: {} not imported: {why}", waiting.file),
    }
}

/// What an import brought, in a sentence.
fn imported(file: &str, brought: &wrayth::Import, merged: &Merged) -> String {
    let [strings, names, ignores] = brought.counts;
    if strings + names + ignores == 0 {
        // Another player's triggers file, which Wrayth's counts do not describe.
        let mut parts = vec![format!(
            "{file} imported: {}",
            counted_as(brought.triggers.len(), "trigger")
        )];
        parts.extend(
            merged
                .renamed
                .iter()
                .map(|(had, given)| format!("`{had}` is taken, so it came in as `{given}`")),
        );
        parts.extend(
            merged
                .refused
                .iter()
                .map(|refused| format!("left out {refused}")),
        );
        return parts.join("; ");
    }
    let off = if brought.ignores_on == Some(false) {
        ", the ignores off as the file had them"
    } else {
        ""
    };
    let mut parts = vec![format!(
        "{file} imported: {} (\"{}\"), {} (\"{}\"), {} (\"{}\"){off}",
        counted_as(strings, "highlight"),
        wrayth::STRINGS,
        counted_as(names, "name"),
        wrayth::NAMES,
        counted_as(ignores, "ignore"),
        wrayth::IGNORES,
    )];
    if merged.replaced > 0 {
        parts.push(format!(
            "{} from an earlier import of it replaced",
            merged.replaced
        ));
    }
    parts.extend(
        merged
            .renamed
            .iter()
            .map(|(had, given)| format!("`{had}` is taken, so it came in as `{given}`")),
    );
    parts.extend(
        merged
            .refused
            .iter()
            .map(|refused| format!("left out {refused}")),
    );
    parts.join("; ")
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A command accepted in the window, of a trigger the import renamed and
    /// then refused, is passed over; the rest of the import comes in (BE-F-7:
    /// the refusal was looked for under the old name, so `approve` failed on
    /// the new one and nothing was imported).
    #[test]
    fn an_accepted_trigger_renamed_then_refused_does_not_fail_the_import() {
        let dir = std::env::temp_dir().join(format!("cena-trigger-import-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).expect("a folder");
        fs::write(
            triggers::path(&dir),
            "[trigger.stunned]\ntext = 'You are stunned'\nsquelch = true\n",
        )
        .expect("written");
        let origin = "Shared: theirs.toml";
        let brought = edit::shared(
            "[trigger.stunned]\nregex = '('\nsend = 'stand'\n\n\
             [trigger.webbed]\ntext = 'webbed'\nsend = 'stance defensive'\n",
            origin,
        )
        .expect("their file");
        let others = Changes::new();
        let id = {
            let mut imports = lock(&others.imports);
            imports.waiting.insert(
                7,
                Waiting {
                    dir: dir.clone(),
                    file: "theirs.toml".to_owned(),
                    origin: origin.to_owned(),
                    brought,
                },
            );
            7
        };
        let said = answer(
            &others,
            id,
            Some(vec!["stunned".to_owned(), "webbed".to_owned()]),
        );
        assert!(said.contains("theirs.toml imported"), "{said}");
        assert!(
            said.contains("`stunned` is taken, so it came in as `stunned (Shared)`"),
            "a Hydra file's rename is not called Wrayth's: {said}"
        );
        let listed = edit::list(&super::super::file(&dir).expect("the file")).expect("listed");
        let names: Vec<(&str, Option<&str>)> = listed
            .iter()
            .map(|t| (t.name.as_str(), t.held.as_deref()))
            .collect();
        assert_eq!(
            names,
            [("stunned", None), ("webbed", None)],
            "webbed approved"
        );
        let _ = fs::remove_dir_all(&dir);
    }
}
