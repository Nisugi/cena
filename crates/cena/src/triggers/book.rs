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
    let sounds_folder = crate::attention::sounds_dir(dir);
    let offered = Book {
        events: cena_session::trigger::LineEvent::every(),
        guard_words: super::form::guard_words(),
        sounds: super::form::sounds(&sounds_folder),
        sounds_folder: sounds_folder.display().to_string(),
        file: shown,
        ..Book::default()
    };
    let read = file(dir).and_then(|text| {
        Ok((
            edit::list(&text)?,
            edit::switches(&text)?,
            edit::tables(&text)?,
        ))
    });
    match read {
        Ok((listed, switches, tables)) => Book {
            triggers: listed
                .into_iter()
                .map(|listed| {
                    let table = tables
                        .iter()
                        .find(|(name, _)| *name == listed.name)
                        .map(|(_, table)| table.clone())
                        .unwrap_or_default();
                    let (off_for, changed_for) = cena_ui::triggers::copies(&table);
                    Entry {
                        form: cena_ui::triggers::from_table(&listed.name, &table),
                        name: listed.name,
                        category: listed.category,
                        enabled: listed.enabled,
                        refused: listed.refused,
                        summary: listed.summary,
                        held: listed.held,
                        origin: listed.origin,
                        off_for,
                        changed_for,
                    }
                })
                .collect(),
            categories: switches.categories,
            kinds: switches
                .kinds
                .into_iter()
                .map(|(kind, on)| (kind.to_owned(), on))
                .collect(),
            problem: None,
            ..offered
        },
        Err(why) => Book {
            problem: Some(format!("Nothing here can be changed while {why}")),
            ..offered
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
            Change::Save { was, form, copy_of } => {
                // A send the form changed is the player's own line now:
                // approved (`plan/54` §1 row 2). One it left alone keeps its
                // approval, or its hold; a copy's, the original's (GU-D-6).
                let from = was.as_deref().or(copy_of.as_deref());
                let before = edit::tables(&old)?
                    .into_iter()
                    .find(|(name, _)| Some(name.as_str()) == from)
                    .and_then(|(_, table)| {
                        table
                            .get("send")
                            .and_then(|s| s.as_str().map(str::to_owned))
                    });
                let approve = form.send.is_some() && form.send != before;
                let text = edit::save(
                    &old,
                    was.as_deref(),
                    copy_of.as_deref(),
                    &form.name,
                    cena_ui::triggers::to_table(form),
                    approve,
                )?;
                let done = match was {
                    None => format!("`{}` added", form.name.trim()),
                    Some(was) if was != form.name.trim() => {
                        format!("`{was}` saved as `{}`", form.name.trim())
                    }
                    Some(_) => format!("`{}` saved", form.name.trim()),
                };
                (text, done)
            }
            Change::OffFor {
                name,
                character,
                off,
            } => {
                let key = format!("for.{character}.enabled");
                let text = if *off {
                    edit::set(&old, name, &key, toml::Value::Boolean(false))?.0
                } else {
                    edit::unset(&old, name, &key)?
                };
                let state = if *off { "off" } else { "back on" };
                (text, format!("`{name}` {state} for {character}"))
            }
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

    /// The form saved: a new trigger, a rename keeping what the form does
    /// not edit, a send approved only when the form changed it, a trigger
    /// the file would refuse refused with why, and off for one character.
    #[test]
    fn the_form_saves_through_the_writer() {
        let dir = scratch("save");
        std::fs::write(
            cena_behavior::triggers::path(&dir),
            "[trigger.theirs]
text = 'webbed'
send = 'stand'
origin = 'a shared file'

[trigger.theirs.for.Dicate]
look = { bold = true }
",
        )
        .expect("written");
        let changes = Changes::new();
        let save = |was: Option<&str>, form: view::Form| Change::Save {
            was: was.map(str::to_owned),
            form: Box::new(form),
            copy_of: None,
        };

        // Recoloured, renamed, its send untouched: still held, its copy kept.
        let mut form = book(&dir).triggers[0].form.clone();
        form.name = "webbed".to_owned();
        form.look = Some(view::Look {
            color: "#ff4040".to_owned(),
            ..view::Look::default()
        });
        let said = apply(&dir, &changes, &save(Some("theirs"), form.clone()));
        assert!(said.contains("`theirs` saved as `webbed`"), "{said}");
        let entry = &book(&dir).triggers[0];
        assert_eq!(entry.name, "webbed");
        assert_eq!(
            entry.held.as_deref(),
            Some("stand"),
            "a send left alone stays held"
        );
        assert_eq!(
            entry.changed_for,
            ["Dicate"],
            "one character's copy is kept"
        );
        assert_eq!(entry.origin.as_deref(), Some("a shared file"));

        // The send changed in the form: the player's own now.
        form.send = Some("stand up".to_owned());
        apply(&dir, &changes, &save(Some("webbed"), form));
        assert_eq!(book(&dir).triggers[0].held, None);

        // A new one, and one the file would refuse.
        let new = view::Form {
            name: "rock".to_owned(),
            text: "a rock".to_owned(),
            squelch: true,
            ..view::Form::default()
        };
        assert!(apply(&dir, &changes, &save(None, new)).contains("`rock` added"));
        let bad = view::Form {
            name: "bad".to_owned(),
            text: "(".to_owned(),
            regex: true,
            squelch: true,
            ..view::Form::default()
        };
        let said = apply(&dir, &changes, &save(None, bad));
        assert!(said.starts_with("Triggers: not changed:"), "{said}");
        assert_eq!(book(&dir).triggers.len(), 2);

        // Off for one character, and back.
        let off = |off| Change::OffFor {
            name: "rock".to_owned(),
            character: "Nisugi".to_owned(),
            off,
        };
        apply(&dir, &changes, &off(true));
        let rock =
            |dir: &std::path::Path| book(dir).triggers.into_iter().find(|t| t.name == "rock");
        assert_eq!(
            rock(&dir).map(|t| t.off_for),
            Some(vec!["Nisugi".to_owned()])
        );
        apply(&dir, &changes, &off(false));
        assert_eq!(rock(&dir).map(|t| t.off_for), Some(Vec::new()));
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// *Duplicate* on a trigger from elsewhere whose send is held: the copy
    /// keeps its origin and its hold (GU-D-6), unless the copy's send is
    /// changed, which approves it as any edit does (`plan/54` §1 row 2).
    #[test]
    fn a_duplicate_keeps_the_hold_of_the_trigger_it_copies() {
        let dir = scratch("duplicate");
        std::fs::write(
            cena_behavior::triggers::path(&dir),
            "[trigger.theirs]\ntext = 'webbed'\nsend = 'give my silver to Thief'\n\
             origin = 'Shared: theirs.toml'\n",
        )
        .expect("written");
        let changes = Changes::new();
        let mut copy = book(&dir).triggers[0].form.clone();
        copy.name = "theirs copy".to_owned();
        let duplicate = |form: &view::Form| Change::Save {
            was: None,
            form: Box::new(form.clone()),
            copy_of: Some("theirs".to_owned()),
        };
        let said = apply(&dir, &changes, &duplicate(&copy));
        assert!(said.contains("`theirs copy` added"), "{said}");
        let made = |dir: &std::path::Path, name: &str| {
            book(dir).triggers.into_iter().find(|t| t.name == name)
        };
        let first = made(&dir, "theirs copy").expect("the copy");
        assert_eq!(
            first.held.as_deref(),
            Some("give my silver to Thief"),
            "the copy's send is held as the original's is"
        );
        assert_eq!(first.origin.as_deref(), Some("Shared: theirs.toml"));

        // A copy whose send the player changed is theirs: approved.
        copy.name = "mine".to_owned();
        copy.send = Some("stand".to_owned());
        apply(&dir, &changes, &duplicate(&copy));
        assert_eq!(made(&dir, "mine").and_then(|t| t.held), None);

        // An approved original's copy is approved for the same line, and no
        // other.
        apply(&dir, &changes, &Change::Approve("theirs".to_owned()));
        copy.name = "approved copy".to_owned();
        copy.send = Some("give my silver to Thief".to_owned());
        apply(&dir, &changes, &duplicate(&copy));
        assert_eq!(made(&dir, "approved copy").and_then(|t| t.held), None);
        let _ = std::fs::remove_dir_all(&dir);
    }
}
