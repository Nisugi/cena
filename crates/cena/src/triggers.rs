//! The one triggers file, given to each character's session (`plan/45`).
//!
//! The join only: the file is `cena_behavior::triggers`, and what a trigger
//! does to a line is the session's ([`SessionHandle::set_triggers`]). This
//! reads the file when a character starts, compiles that character's list,
//! and says what it could not use. A file changed between two starts is read
//! again at the second; `;trigger` will read it on demand.

use std::path::Path;

use cena_behavior::triggers;
use cena_session::trigger::Matcher;
use cena_session::{Notice, NoticeKind, SessionHandle};

/// Give `character`'s session its triggers from the file under `dir`, and
/// say what was left out. No file is no triggers, and nothing is said.
pub(crate) fn open(handle: &SessionHandle, dir: &Path, character: &str) {
    let loaded = match triggers::load(dir) {
        Ok(loaded) => loaded,
        Err(why) => {
            handle.say(Notice::line(
                NoticeKind::Warn,
                format!("Triggers: {why}. None are on."),
            ));
            return;
        }
    };
    for refused in &loaded.refused {
        handle.say(Notice::line(
            NoticeKind::Warn,
            format!("Triggers: {refused} It is left out."),
        ));
    }
    let mine = loaded.triggers.for_character(character);
    let count = mine.len();
    match Matcher::new(mine) {
        Ok(matcher) => {
            handle.set_triggers(matcher);
            if count > 0 {
                let noun = if count == 1 { "trigger" } else { "triggers" };
                handle.say(Notice::line(
                    NoticeKind::Info,
                    format!("Triggers: {count} {noun} on."),
                ));
            }
        }
        Err(why) => handle.say(Notice::line(
            NoticeKind::Warn,
            format!("Triggers: {why}. None are on."),
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use cena_platform::ReplaySource;
    use cena_session::{Event, Session};
    use std::path::PathBuf;

    /// A data directory holding `file` as its triggers file, if any.
    fn dir(test: &str, file: Option<&str>) -> std::io::Result<PathBuf> {
        let dir =
            std::env::temp_dir().join(format!("cena-bin-triggers-{test}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir)?;
        if let Some(file) = file {
            std::fs::write(triggers::path(&dir), file)?;
        }
        Ok(dir)
    }

    /// What `character` is given from `file`, and what it is told.
    fn opened(
        test: &str,
        file: Option<&str>,
        character: &str,
    ) -> std::io::Result<(usize, Vec<String>)> {
        let dir = dir(test, file)?;
        let session = Session::new(ReplaySource::from_bytes(b""));
        let handle = session.handle();
        let (_, mut events) = session.subscribe();
        open(&handle, &dir, character);
        let _ = std::fs::remove_dir_all(&dir);
        let told = std::iter::from_fn(|| events.try_recv().ok())
            .filter_map(|event| match event {
                Event::Notice(notice) => Some(notice.lines().join(" ")),
                _ => None,
            })
            .collect();
        Ok((handle.triggers().triggers().len(), told))
    }

    #[tokio::test(flavor = "current_thread", start_paused = true)]
    async fn a_character_is_given_its_triggers_and_told_what_was_left_out() {
        let file = "[trigger.stunned]\ntext = 'You are stunned'\nlook = { bold = true }\n\
                    [trigger.bad]\ntext = 'x'\nlook = { color = 'red' }\n\
                    [trigger.dicates]\ntext = 'y'\nsquelch = true\ncharacters = ['Dicate']\n";
        let (count, told) = opened("given", Some(file), "Nisugi").unwrap();
        assert_eq!(
            count, 1,
            "stunned; bad is refused and dicates is not Nisugi's"
        );
        assert_eq!(told.len(), 2, "{told:?}");
        assert!(
            told[0].contains("`bad`") && told[0].contains("left out"),
            "{told:?}"
        );
        assert_eq!(told[1], "Triggers: 1 trigger on.");
    }

    #[tokio::test(flavor = "current_thread", start_paused = true)]
    async fn no_file_is_no_triggers_and_nothing_said() {
        let (count, told) = opened("none", None, "Nisugi").unwrap();
        assert_eq!(count, 0);
        assert!(told.is_empty(), "{told:?}");
    }

    #[tokio::test(flavor = "current_thread", start_paused = true)]
    async fn a_file_that_is_not_toml_is_said_and_nothing_is_on() {
        let (count, told) = opened("broken", Some("[trigger.x\n"), "Nisugi").unwrap();
        assert_eq!(count, 0);
        assert_eq!(told.len(), 1, "{told:?}");
        assert!(
            told[0].contains("is not TOML") && told[0].contains("None are on"),
            "{told:?}"
        );
    }
}
