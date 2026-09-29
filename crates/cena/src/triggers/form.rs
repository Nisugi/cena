//! What the trigger editor's form offers from the binary's side
//! (`plan/54` step 2): the guard words a trigger may use, and the sounds
//! folder's files. The form and a trigger's table are turned into each other
//! by `cena_ui::triggers::Form` itself.

use std::path::Path;

/// The words the form offers for a condition and *only if*: the guard words,
/// less those a trigger cannot use (`plan/45` §6b: they read what a routine
/// sent, or the map, which a session has not got).
pub(crate) fn guard_words() -> Vec<String> {
    const REFUSED: [&str; 5] = ["once", "once_here", "every <n>", "splashy", "nomagic"];
    cena_session::guard::vocabulary()
        .into_iter()
        .filter(|word| !REFUSED.contains(&word.as_str()))
        .collect()
}

/// The sound files in `folder`, by name, sorted.
pub(crate) fn sounds(folder: &Path) -> Vec<String> {
    let mut names: Vec<String> = std::fs::read_dir(folder)
        .map(|entries| {
            entries
                .filter_map(Result::ok)
                .map(|entry| entry.path())
                .filter(|path| {
                    path.extension().and_then(|x| x.to_str()).is_some_and(|x| {
                        ["wav", "mp3", "ogg", "flac"].contains(&x.to_ascii_lowercase().as_str())
                    })
                })
                .filter_map(|path| path.file_name()?.to_str().map(str::to_owned))
                .collect()
        })
        .unwrap_or_default();
    names.sort();
    names
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_trigger_cannot_offer_words_it_cannot_use() {
        let words = guard_words();
        assert!(words.contains(&"stunned".to_owned()));
        assert!(words.contains(&"health_at_least <n>".to_owned()));
        assert!(!words.contains(&"once".to_owned()));
        assert!(!words.contains(&"splashy".to_owned()));
    }
}
