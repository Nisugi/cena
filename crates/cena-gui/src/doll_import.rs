//! A player's `VellumFE` dolls brought into Hydra's `dolls` folder
//! (`plan/55` step 6): `;doll import <folder>`.
//!
//! Each PNG is copied, pictures and their overlays alike. A picture whose
//! calibration `VellumFE` kept beside it, `<picture>.toml`, and not in the
//! picture, has it written into the copy (`calibration.rs`), as it is: the
//! author's rule is that a doll's calibration lives in its picture. The
//! player's own files are never changed, and a file already in Hydra's
//! folder is left as it is.

use std::path::{Path, PathBuf};

use crate::calibration;
use crate::doll_art;

/// What an import brought.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct DollsImported {
    /// The folder the dolls came from.
    pub from: PathBuf,
    /// Doll pictures copied.
    pub pictures: usize,
    /// Overlays copied.
    pub overlays: usize,
    /// Pictures whose calibration was written into them from the `.toml`
    /// beside them.
    pub calibrated: usize,
    /// Files left alone: already in Hydra's folder.
    pub kept: Vec<String>,
}

/// Where `VellumFE` keeps its dolls under `folder`: its own
/// `global/images/dolls` when `folder` is `VellumFE`'s home, or `folder`
/// itself.
fn dolls_under(folder: &Path) -> PathBuf {
    let pool = folder.join("global").join("images").join("dolls");
    if pool.is_dir() { pool } else { folder.to_path_buf() }
}

/// Copy the dolls under `folder` into `into`, Hydra's `dolls` folder.
///
/// # Errors
///
/// Why nothing was brought: `folder` cannot be read, or holds no PNG, or
/// `into` cannot be made.
pub fn import_dolls(folder: &Path, into: &Path) -> Result<DollsImported, String> {
    let from = dolls_under(folder);
    let mut pngs: Vec<PathBuf> = std::fs::read_dir(&from)
        .map_err(|e| format!("{}: {e}", from.display()))?
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|path| {
            path.extension()
                .is_some_and(|x| x.eq_ignore_ascii_case("png"))
        })
        .collect();
    if pngs.is_empty() {
        return Err(format!("{} has no pictures in it.", from.display()));
    }
    pngs.sort();
    std::fs::create_dir_all(into).map_err(|e| format!("{}: {e}", into.display()))?;
    let mut brought = DollsImported {
        from: from.clone(),
        ..DollsImported::default()
    };
    for png in pngs {
        let Some(name) = png.file_name() else {
            continue;
        };
        let to = into.join(name);
        if to.exists() {
            brought.kept.push(name.to_string_lossy().into_owned());
            continue;
        }
        let bytes = std::fs::read(&png).map_err(|e| format!("{}: {e}", png.display()))?;
        let beside = png.with_extension("toml");
        let written = match std::fs::read_to_string(&beside) {
            Ok(text) if calibration::embedded(&bytes).is_none() => {
                calibration::with_chunk(&bytes, &text).map(|bytes| (bytes, true))
            }
            _ => None,
        };
        let (bytes, calibrated) = written.unwrap_or((bytes, false));
        cena_session::store::save_bytes(into, &to, &bytes)
            .map_err(|e| format!("{}: {e}", to.display()))?;
        if doll_art::is_layer(&png) {
            brought.overlays += 1;
        } else {
            brought.pictures += 1;
            brought.calibrated += usize::from(calibrated);
        }
    }
    Ok(brought)
}

#[cfg(test)]
mod tests {
    use super::import_dolls;

    /// `VellumFE`'s home, its dolls under `global/images/dolls`: a picture
    /// calibrated beside it, an overlay, and a picture already in Hydra's
    /// folder. The calibration goes into the copy; the player's own files
    /// are as they were; the one already there is left alone.
    #[test]
    fn vellums_dolls_come_over_their_calibration_in_the_picture() {
        let root = std::env::temp_dir().join(format!("cena-doll-import-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        let pool = root.join("vellum").join("global").join("images").join("dolls");
        let hydra = root.join("hydra").join("dolls");
        std::fs::create_dir_all(&pool).unwrap();
        std::fs::create_dir_all(&hydra).unwrap();
        let png = |path: &std::path::Path| image::RgbaImage::new(2, 2).save(path).unwrap();
        png(&pool.join("ranger.png"));
        std::fs::write(pool.join("ranger.toml"), "[anchors]\nhead = [0.5, 0.25]\n").unwrap();
        png(&pool.join("ranger_chest_injury2.png"));
        png(&pool.join("mine.png"));
        png(&hydra.join("mine.png"));
        let before = std::fs::read(pool.join("ranger.png")).unwrap();

        let brought = import_dolls(&root.join("vellum"), &hydra).unwrap();
        assert_eq!(
            (brought.pictures, brought.overlays, brought.calibrated),
            (1, 1, 1)
        );
        assert_eq!(brought.kept, ["mine.png"]);
        let copy = crate::calibration::Calibration::of_picture(&hydra.join("ranger.png"));
        assert_eq!(copy.anchors.get("head"), Some(&(0.5, 0.25)));
        assert!(!hydra.join("ranger.toml").exists(), "no file beside it");
        assert_eq!(std::fs::read(pool.join("ranger.png")).unwrap(), before);
        let _ = std::fs::remove_dir_all(&root);
    }
}
