//! *What would it have caught?* (`plan/54` step 4): the form run over a
//! character's player log (`plan/25`), the lines it matches.
//!
//! The form's own rule alone, through the real matcher, over the days kept,
//! newest first: a regex that would fire on half a hunt is seen before it is
//! saved. Blocking file I/O, so it runs where `Sessions::check_log` puts
//! it, off the window's thread, the writer flushed
//! first when the character is playing.

use std::path::Path;
use std::sync::{Arc, Mutex};

use cena_session::player_log::reader::{self, Entry, MAX_HITS};
use cena_session::trigger::{Matcher, Trigger};
use cena_ui::triggers::Form;

/// What to check: whose log, how many days back, and the form.
#[derive(Clone, Debug)]
pub(crate) struct Ask {
    /// The character, as the roster names it.
    pub(crate) character: String,
    /// How many of the newest days kept.
    pub(crate) days: usize,
    /// The trigger as the form has it.
    pub(crate) form: Form,
}

/// What the form caught.
#[derive(Clone, Debug, Default)]
pub(crate) struct Caught {
    /// The lines it matched, newest first.
    pub(crate) lines: Vec<Entry>,
    /// It stopped at [`MAX_HITS`] with more to find.
    pub(crate) more: bool,
    /// The days read.
    pub(crate) days: usize,
    /// Why nothing could be checked.
    pub(crate) why: Option<String>,
}

/// Where a check's answer lands.
pub(crate) type Inbox = Arc<Mutex<Option<Caught>>>;

/// Run `ask` over the log under `root`.
pub(crate) fn run(root: &Path, ask: &Ask) -> Caught {
    let failed = |why: String| Caught {
        why: Some(why),
        ..Caught::default()
    };
    let rule = match ask.form.rule() {
        Ok(rule) => rule,
        Err(why) => return failed(format!("the trigger as it stands would be refused: {why}")),
    };
    let matcher = match Matcher::new(vec![Trigger {
        name: ask.form.name.clone(),
        rule,
    }]) {
        Ok(matcher) => matcher,
        Err(why) => return failed(why),
    };
    let days = match reader::days(root, &ask.character) {
        Ok(days) => days,
        Err(why) => return failed(why.to_string()),
    };
    let mut caught = Caught::default();
    for day in days.iter().take(ask.days) {
        caught.days += 1;
        let lines = match reader::read_day(root, &ask.character, day) {
            Ok(lines) => lines,
            Err(why) => return failed(why.to_string()),
        };
        for entry in lines.into_iter().rev() {
            if matcher.hits(stream(&entry.stream), &entry.text).is_empty() {
                continue;
            }
            if caught.lines.len() == MAX_HITS {
                caught.more = true;
                return caught;
            }
            caught.lines.push(entry);
        }
    }
    caught
}

/// The stream a logged line came from, as the matcher names it: the tag's
/// source, `main` being the story's empty id (`plan/25` step 2b).
fn stream(tag: &str) -> &str {
    match tag.split('/').next().unwrap_or(tag) {
        "main" => "",
        source => source,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_form_is_run_over_the_days_kept_newest_first() {
        let root = std::env::temp_dir().join(format!("cena-gui-catch-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        for (day, body) in [
            (
                "2026-09-20",
                "[06:00:00.000][main] a rock\n[06:00:01.000][thoughts] a rock thought\n",
            ),
            (
                "2026-09-21",
                "[07:00:00.000][main/combat] you swing at a rock\n[07:00:01.000][main] nothing\n",
            ),
        ] {
            let path = cena_session::player_log::writer::day_path(&root, "Nisugi", day);
            std::fs::create_dir_all(path.parent().expect("a parent")).expect("mkdir");
            std::fs::write(path, body).expect("write");
        }
        let form = Form {
            name: "rock".to_owned(),
            text: "rock".to_owned(),
            stream: "main".to_owned(),
            squelch: true,
            ..Form::default()
        };
        let ask = |days| Ask {
            character: "Nisugi".to_owned(),
            days,
            form: form.clone(),
        };
        let caught = run(&root, &ask(2));
        let words: Vec<&str> = caught.lines.iter().map(|e| e.text.as_str()).collect();
        assert_eq!(
            words,
            ["you swing at a rock", "a rock"],
            "the story only, newest first"
        );
        assert_eq!(caught.days, 2);
        assert_eq!(run(&root, &ask(1)).lines.len(), 1, "one day back");

        let bad = Ask {
            form: Form {
                text: "(".to_owned(),
                regex: true,
                ..form.clone()
            },
            ..ask(1)
        };
        assert!(run(&root, &bad).why.is_some());
        let _ = std::fs::remove_dir_all(&root);
    }
}
