//! `;trigger import <path>`: a Wrayth settings file into the triggers file
//! (`plan/45` Stage 4), then what could not come. The reading and the merge
//! are `cena_behavior::triggers::wrayth` and `edit::import`; this says what
//! they did.

use std::fs;
use std::path::Path;

use cena_behavior::triggers::edit::{self, Merged};
use cena_behavior::triggers::wrayth;
use cena_session::{NoticeKind, SessionHandle};

use super::{Changes, Said, change, counted_as};

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
    let origin = format!("Wrayth: {named}");
    let brought = match fs::read_to_string(path)
        .map_err(|e| e.to_string())
        .and_then(|xml| wrayth::read(&xml, &origin))
    {
        Ok(brought) => brought,
        Err(why) => return vec![(NoticeKind::Error, format!("{}: {why}", path.display()))],
    };
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
                "{} play a sound: from the path Wrayth wrote when it is there, otherwise by \
                 its file name from {}.",
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
    said
}

/// What an import brought, in a sentence.
fn imported(file: &str, brought: &wrayth::Import, merged: &Merged) -> String {
    let [strings, names, ignores] = brought.counts;
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
