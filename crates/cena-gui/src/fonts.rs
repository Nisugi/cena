//! The fonts a theme may name (`plan/57` step 5): every `.ttf` and `.otf`
//! in the `fonts` folder of the data folder, loaded once into egui beside
//! its own, each as a family named by its file's stem. A theme names one
//! by that stem; one it names that is not here is egui's own.
//!
//! A file that is not a font is refused by its first bytes, since egui
//! stops on one it cannot parse; a font that is broken past its header
//! still would, so a font goes into the folder whole.

use std::collections::BTreeSet;
use std::path::Path;
use std::sync::Arc;

use egui::{FontData, FontDefinitions, FontFamily};

/// Where the families loaded are kept in egui's data.
fn key() -> egui::Id {
    egui::Id::new("theme-fonts")
}

/// The families loaded, by stem.
pub(crate) fn loaded(ctx: &egui::Context) -> BTreeSet<String> {
    ctx.data(|data| data.get_temp::<Arc<BTreeSet<String>>>(key()))
        .map(|set| (*set).clone())
        .unwrap_or_default()
}

/// Load every font in `folder` into `ctx` beside egui's own; the stems
/// loaded, in order, and a note for each file refused.
pub(crate) fn load(ctx: &egui::Context, folder: &Path) -> (Vec<String>, Vec<String>) {
    let mut definitions = FontDefinitions::default();
    let (mut stems, mut notes) = (Vec::new(), Vec::new());
    let mut files: Vec<_> = std::fs::read_dir(folder)
        .into_iter()
        .flatten()
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|path| {
            path.extension()
                .and_then(|ext| ext.to_str())
                .is_some_and(|ext| {
                    ext.eq_ignore_ascii_case("ttf") || ext.eq_ignore_ascii_case("otf")
                })
        })
        .collect();
    files.sort();
    for path in files {
        let Some(stem) = path.file_stem().and_then(|s| s.to_str()) else {
            continue;
        };
        match std::fs::read(&path) {
            Ok(bytes) if is_font(&bytes) => {
                definitions
                    .font_data
                    .insert(stem.to_owned(), Arc::new(FontData::from_owned(bytes)));
                definitions
                    .families
                    .insert(FontFamily::Name(stem.into()), vec![stem.to_owned()]);
                stems.push(stem.to_owned());
            }
            Ok(_) => notes.push(format!(
                "{} is not a TrueType or OpenType font",
                path.display()
            )),
            Err(why) => notes.push(format!("{} cannot be read: {why}", path.display())),
        }
    }
    ctx.set_fonts(definitions);
    ctx.data_mut(|data| {
        data.insert_temp(
            key(),
            Arc::new(stems.iter().cloned().collect::<BTreeSet<_>>()),
        );
    });
    (stems, notes)
}

/// Whether `bytes` begin as a TrueType or OpenType font does.
fn is_font(bytes: &[u8]) -> bool {
    bytes.len() >= 1024
        && matches!(
            &bytes[..4],
            [0x00, 0x01, 0x00, 0x00] | b"OTTO" | b"true" | b"ttcf"
        )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_folder_of_fonts_is_loaded_and_what_is_not_a_font_refused() {
        let dir = std::env::temp_dir().join(format!("cena-fonts-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("notes.ttf"), "not a font").unwrap();
        std::fs::write(dir.join("readme.txt"), "ignored").unwrap();
        let ctx = egui::Context::default();
        let (stems, notes) = load(&ctx, &dir);
        assert!(stems.is_empty());
        assert_eq!(notes.len(), 1, "{notes:?}");
        assert!(notes[0].contains("notes.ttf"));
        assert!(loaded(&ctx).is_empty());
        let (stems, notes) = load(&ctx, &dir.join("missing"));
        assert!(stems.is_empty() && notes.is_empty(), "no folder, nothing");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_font_is_known_by_its_first_bytes() {
        let mut truetype = vec![0x00, 0x01, 0x00, 0x00];
        truetype.resize(2048, 0);
        assert!(is_font(&truetype));
        let mut opentype = b"OTTO".to_vec();
        opentype.resize(2048, 0);
        assert!(is_font(&opentype));
        assert!(!is_font(b"OTTO"), "too short to be one");
        assert!(!is_font(&vec![b'x'; 4096]));
    }
}
