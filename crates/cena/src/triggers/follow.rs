//! Each character's triggers task: its trigger sends, sent ([`send`]), and
//! the triggers file read again whenever another character changes it.
//!
//! One file serves every character, and `;trigger` runs in the session of
//! the character it was typed at, which holds no handle on the others. So a
//! change is told on [`Changes`], which the table owns and hands to every
//! character as it does the hunt's `Party`, and each character's task reads
//! the file again for itself, and says who changed it. The character that
//! made the change read it at once, with all there was to say, and is not
//! told twice.
//!
//! One task for both, because the session's events ending is the only word
//! that the character has stopped, and each needs it.

use std::path::PathBuf;

use cena_session::{Event, Notice, NoticeKind, SessionHandle};
use tokio::sync::broadcast::{self, error::RecvError};

use super::act::send;
use super::load::{counted, reload};

/// How many changes a character may fall behind before it only knows that
/// the file changed, not who changed it.
const BEHIND: usize = 16;

/// Word of each change to the triggers file, to every character following.
#[derive(Clone)]
pub(crate) struct Changes(broadcast::Sender<Changed>);

/// One change to the triggers file: which character made it, and what it
/// did.
#[derive(Clone, Debug)]
pub(crate) struct Changed {
    by: String,
    done: String,
}

/// One character's place on [`Changes`], taken before it first reads the
/// file, so no change falls between the reading and the following.
pub(crate) struct Following(broadcast::Receiver<Changed>);

impl Changes {
    /// Nobody following yet.
    pub(crate) fn new() -> Self {
        Self(broadcast::channel(BEHIND).0)
    }

    /// A place for one character, from now on.
    pub(crate) fn follow(&self) -> Following {
        Following(self.0.subscribe())
    }

    /// Tell every character following that `by` changed the file: `done`.
    pub(super) fn tell(&self, by: &str, done: &str) {
        // Nobody following is nobody to tell.
        let _ = self.0.send(Changed {
            by: by.to_owned(),
            done: done.to_owned(),
        });
    }
}

/// Run `character`'s triggers until its session ends: each trigger send in
/// its `events`, sent through `handle`, and each change another character
/// makes to the file under `dir`, read into its session.
pub(crate) async fn run(
    mut events: broadcast::Receiver<Event>,
    following: Following,
    handle: SessionHandle,
    dir: PathBuf,
    character: String,
) {
    let mut heard = following.0;
    let mut open = true;
    loop {
        tokio::select! {
            event = events.recv() => match event {
                Ok(Event::Act { act, generation }) => {
                    send(&handle, generation, &act.trigger, &act.line).await;
                }
                Ok(_) | Err(RecvError::Lagged(_)) => {}
                Err(RecvError::Closed) => return,
            },
            news = heard.recv(), if open => match news {
                Ok(news) if news.by.eq_ignore_ascii_case(&character) => {}
                Ok(news) => read_again(&handle, &dir, &character, Some(&news)),
                Err(RecvError::Lagged(_)) => read_again(&handle, &dir, &character, None),
                Err(RecvError::Closed) => open = false,
            },
        }
    }
}

/// Read the file again into `character`'s session, and say why and what is
/// on: who changed it when that is known.
fn read_again(
    handle: &SessionHandle,
    dir: &std::path::Path,
    character: &str,
    changed: Option<&Changed>,
) {
    let why = changed.map_or_else(
        || "the file changed".to_owned(),
        |changed| format!("from {}: {}", changed.by, changed.done),
    );
    let (kind, said) = match reload(handle, dir, character) {
        Ok(reloaded) => (
            NoticeKind::Info,
            format!("{why}. {} on.", counted(reloaded.count)),
        ),
        Err(e) => (
            NoticeKind::Warn,
            format!("{why}; {e}. {}", super::load::still_on(handle)),
        ),
    };
    handle.say(Notice::line(kind, format!("Triggers: {said}")));
}

#[cfg(test)]
mod tests {
    use std::{fs, io};

    use cena_platform::ReplaySource;
    use cena_session::Session;

    use super::*;
    use crate::commands::Commands;

    /// Everything said to the player so far, as text.
    fn told(events: &mut broadcast::Receiver<Event>) -> Vec<String> {
        std::iter::from_fn(|| events.try_recv().ok())
            .filter_map(|event| match event {
                Event::Notice(notice) => Some(notice.lines().join(" ")),
                _ => None,
            })
            .collect()
    }

    fn folder(test: &str) -> io::Result<PathBuf> {
        let dir =
            std::env::temp_dir().join(format!("cena-bin-follow-{test}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir)?;
        Ok(dir)
    }

    /// A change typed at one character reaches the other, which says who
    /// made it; the one that typed it is not told twice.
    #[tokio::test(flavor = "current_thread", start_paused = true)]
    async fn a_change_reaches_every_other_character_and_names_who_made_it() {
        let dir = folder("reaches").unwrap();
        let changes = Changes::new();
        let nisugi = Session::new(ReplaySource::from_bytes(b""));
        let dicate = Session::new(ReplaySource::from_bytes(b""));
        let (_, mut nisugi_told) = nisugi.subscribe();
        let (_, mut dicate_told) = dicate.subscribe();
        let commands = Commands::default();
        super::super::command(
            &nisugi.handle(),
            &commands,
            dir.clone(),
            "Nisugi".to_owned(),
            changes.clone(),
        );
        for (session, name) in [(&nisugi, "Nisugi"), (&dicate, "Dicate")] {
            let (_, events) = session.subscribe();
            tokio::spawn(run(
                events,
                changes.follow(),
                session.handle(),
                dir.clone(),
                name.to_owned(),
            ));
        }

        assert!(
            commands
                .route("trigger add stunned You are stunned")
                .is_some()
        );
        tokio::time::sleep(std::time::Duration::from_secs(1)).await;
        assert_eq!(dicate.handle().triggers().triggers().len(), 1);
        assert_eq!(
            told(&mut dicate_told),
            [
                "Triggers: from Nisugi: `stunned` added, making \"You are stunned\" bold. 1 trigger on."
            ]
        );
        let own = told(&mut nisugi_told);
        assert_eq!(own.len(), 1, "told once, by its own command: {own:?}");

        // A file changed by hand is read again by everyone at one reload.
        fs::write(cena_behavior::triggers::path(&dir), "").unwrap();
        assert!(commands.route("trigger reload").is_some());
        tokio::time::sleep(std::time::Duration::from_secs(1)).await;
        assert_eq!(dicate.handle().triggers().triggers().len(), 0);
        assert_eq!(
            told(&mut dicate_told),
            ["Triggers: from Nisugi: reloaded. 0 triggers on."]
        );
        let _ = fs::remove_dir_all(&dir);
    }
}
