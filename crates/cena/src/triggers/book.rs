//! The trigger editor's side of the binary (`plan/54` step 1): the file read
//! into the window's [`Book`], and a change from the window made through
//! the writer `;trigger` uses, then told to every running character.

use std::path::Path;

use cena_behavior::triggers::edit::{self, Switch};
use cena_ui::triggers::{self as view, Book, Change, Entry};

use super::follow::Changes;
use super::{file, write};

/// Who the running characters are told made a change: not a character's
/// name, so every one of them reads the file again.
const BY: &str = "the Triggers window";

/// The file under `dir`, as the editor shows it.
pub(crate) fn book(dir: &Path) -> Book {
    let shown = cena_behavior::triggers::path(dir).display().to_string();
    let read = file(dir).and_then(|text| Ok((edit::list(&text)?, edit::switches(&text)?)));
    match read {
        Ok((listed, switches)) => Book {
            triggers: listed
                .into_iter()
                .map(|listed| Entry {
                    name: listed.name,
                    category: listed.category,
                    enabled: listed.enabled,
                    refused: listed.refused,
                    summary: listed.summary,
                    held: listed.held,
                    origin: listed.origin,
                })
                .collect(),
            categories: switches.categories,
            kinds: switches
                .kinds
                .into_iter()
                .map(|(kind, on)| (kind.to_owned(), on))
                .collect(),
            file: shown,
            problem: None,
        },
        Err(why) => Book {
            file: shown,
            problem: Some(format!("Nothing here can be changed while {why}")),
            ..Book::default()
        },
    }
}

/// Give the window the file as it is now.
pub(crate) fn send(dir: &Path, gui: Option<&cena_gui::Sessions>) {
    if let Some(gui) = gui {
        gui.triggers(book(dir));
    }
}

/// Make `change` to the file under `dir`, tell every running character, and
/// say what was done or why it was not.
pub(crate) fn apply(dir: &Path, others: &Changes, change: &Change) -> String {
    let path = cena_behavior::triggers::path(dir);
    let made = cena_session::store::changing(&path, || {
        let old = file(dir)?;
        let (text, done) = match change {
            Change::Switch(which, on) => {
                let state = if *on { "on" } else { "off" };
                let (which, what) = match which {
                    view::Switch::Trigger(name) => (Switch::Trigger(name), format!("`{name}`")),
                    view::Switch::Category(category) => {
                        (Switch::Category(category), format!("category {category}"))
                    }
                    view::Switch::Every(kind) => (Switch::Every(kind), format!("every {kind}")),
                };
                (edit::switch(&old, which, *on)?, format!("{what} {state}"))
            }
            Change::Approve(name) => {
                let (text, line) = edit::approve(&old, name)?;
                (text, format!("`{name}` approved: it sends \"{line}\""))
            }
            Change::Remove(name) => (edit::remove(&old, name)?, format!("`{name}` removed")),
        };
        write(dir, &text)?;
        Ok::<String, String>(done)
    });
    match made {
        Ok(done) => {
            others.tell(BY, &done);
            format!("Triggers: {done}; every running character reads it again.")
        }
        Err(why) => format!("Triggers: not changed: {why}"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scratch(test: &str) -> std::path::PathBuf {
        let dir =
            std::env::temp_dir().join(format!("cena-trigger-book-{test}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("a folder");
        dir
    }

    /// The window's view of the file, and a change made through the writer
    /// `;trigger` uses: switched, approved, removed, or refused with why.
    #[test]
    fn the_book_reads_the_file_and_a_change_is_made_or_refused() {
        let dir = scratch("change");
        std::fs::write(
            cena_behavior::triggers::path(&dir),
            "[trigger.theirs]\ncategory = 'Combat'\ntext = 'webbed'\nsend = 'stand'\norigin = 'a shared file'\n",
        )
        .expect("written");
        let changes = Changes::new();

        let read = book(&dir);
        assert_eq!(read.problem, None);
        assert_eq!(read.triggers.len(), 1);
        assert_eq!(read.triggers[0].held.as_deref(), Some("stand"));
        assert_eq!(read.categories, [("Combat".to_owned(), true)]);

        let said = apply(&dir, &changes, &Change::Approve("theirs".to_owned()));
        assert!(said.contains("approved"), "{said}");
        assert_eq!(book(&dir).triggers[0].held, None);

        let said = apply(
            &dir,
            &changes,
            &Change::Switch(view::Switch::Every("nonsense".to_owned()), false),
        );
        assert!(said.starts_with("Triggers: not changed:"), "{said}");

        apply(
            &dir,
            &changes,
            &Change::Switch(view::Switch::Trigger("theirs".to_owned()), false),
        );
        assert!(!book(&dir).triggers[0].enabled);
        apply(&dir, &changes, &Change::Remove("theirs".to_owned()));
        assert!(book(&dir).triggers.is_empty());
        let _ = std::fs::remove_dir_all(&dir);
    }
}
