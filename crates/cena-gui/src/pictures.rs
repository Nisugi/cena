//! Pictures the window draws from the player's files: a bar's images, a
//! doll's picture and its overlays (`plan/55` §2d).
//!
//! One cache for all of them, kept by egui per path. The bar's own cache
//! read a file once and kept the answer for the rest of the run: a picture
//! the player changed stayed as it was, and one that failed to load -- not
//! there yet, or half copied -- never loaded (`plan/55` §1a). This one looks
//! at the file's modification time, **at most once a second** per picture,
//! and reads it again only when that changed. A draw costs a stat a second
//! at most, never a read of an unchanged file.

use std::path::Path;
use std::time::{Duration, Instant, SystemTime};

/// How often a kept picture's file is looked at again.
const LOOK_AGAIN: Duration = Duration::from_secs(1);

/// What was last read from a file, and when the file was last looked at.
#[derive(Clone)]
struct Kept<T> {
    read: Option<T>,
    /// The file's modification time when read; `None` when there was no
    /// file to read.
    modified: Option<SystemTime>,
    looked: Instant,
}

/// The picture at `path` as a texture, or `None` when it cannot be read.
pub(crate) fn picture(context: &egui::Context, path: &str) -> Option<egui::TextureHandle> {
    picture_at(context, path, Instant::now())
}

/// The doll calibration of the picture at `path` (`calibration.rs`), kept
/// as a picture is.
pub(crate) fn calibration(context: &egui::Context, path: &str) -> crate::calibration::Calibration {
    kept_at(context, "calibration", path, Instant::now(), |path| {
        Some(crate::calibration::Calibration::of_picture(Path::new(path)))
    })
    .unwrap_or_default()
}

/// [`picture`], at `now`.
fn picture_at(context: &egui::Context, path: &str, now: Instant) -> Option<egui::TextureHandle> {
    kept_at(context, "picture", path, now, |path| read(context, path))
}

/// What `read` makes of the file at `path`, kept by egui under `kind` and
/// read again only when the file's modification time has changed, looked
/// at no oftener than [`LOOK_AGAIN`].
fn kept_at<T: Clone + Send + Sync + 'static>(
    context: &egui::Context,
    kind: &'static str,
    path: &str,
    now: Instant,
    read: impl FnOnce(&str) -> Option<T>,
) -> Option<T> {
    kept_watching_at(context, (kind, path), path, now, read)
}

/// What `read` makes of `watched`, kept by egui under `kind` and `key` and
/// read again when `watched`'s modification time has changed: a folder's,
/// for what is in it (`doll_art.rs`).
pub(crate) fn kept_watching<T: Clone + Send + Sync + 'static>(
    context: &egui::Context,
    kind: &'static str,
    key: &str,
    watched: &str,
    read: impl FnOnce(&str) -> Option<T>,
) -> Option<T> {
    kept_watching_at(context, (kind, key), watched, Instant::now(), read)
}

fn kept_watching_at<T: Clone + Send + Sync + 'static>(
    context: &egui::Context,
    (kind, key): (&'static str, &str),
    path: &str,
    now: Instant,
    read: impl FnOnce(&str) -> Option<T>,
) -> Option<T> {
    let id = egui::Id::new((kind, key));
    let kept = context.data(|data| data.get_temp::<Kept<T>>(id));
    if let Some(kept) = &kept
        && now.saturating_duration_since(kept.looked) < LOOK_AGAIN
    {
        return kept.read.clone();
    }
    let modified = std::fs::metadata(path).and_then(|m| m.modified()).ok();
    let fresh = match kept {
        Some(kept) if kept.modified == modified => kept.read,
        _ => modified.and_then(|_| read(path)),
    };
    let kept = Kept {
        read: fresh.clone(),
        modified,
        looked: now,
    };
    context.data_mut(|data| data.insert_temp(id, kept));
    fresh
}

/// Whether the picture at `path` has been read: for a test that a picture
/// not shown is never decoded (`plan/55` §2d).
#[cfg(test)]
pub(crate) fn was_read(context: &egui::Context, path: &str) -> bool {
    let id = egui::Id::new(("picture", path));
    context.data(|data| data.get_temp::<Kept<egui::TextureHandle>>(id).is_some())
}

/// A PNG, or any picture the `image` crate reads, as a texture.
fn read(context: &egui::Context, path: &str) -> Option<egui::TextureHandle> {
    let bytes = std::fs::read(Path::new(path)).ok()?;
    let mut image = image::load_from_memory(&bytes).ok()?;
    // **No larger than a texture can be** (the crate review of 2026-10-01,
    // GU-C-1): egui asserts it and the GPU refuses it, either way on the
    // window thread, which every character's session shares.
    let most = u32::try_from(context.input(|input| input.max_texture_side)).unwrap_or(u32::MAX);
    if image.width() > most || image.height() > most {
        image = image.resize(most, most, image::imageops::FilterType::Triangle);
    }
    let image = image.to_rgba8();
    let size = [
        usize::try_from(image.width()).ok()?,
        usize::try_from(image.height()).ok()?,
    ];
    let pixels = egui::ColorImage::from_rgba_unmultiplied(size, image.as_raw());
    Some(context.load_texture(path, pixels, egui::TextureOptions::LINEAR))
}

#[cfg(test)]
mod tests {
    use super::{LOOK_AGAIN, picture_at};
    use std::time::{Duration, Instant, SystemTime};

    /// A `width` by 1 PNG at `path`, stamped `modified`.
    fn png(path: &std::path::Path, width: u32, modified: SystemTime) -> Option<()> {
        image::RgbaImage::new(width, 1).save(path).ok()?;
        std::fs::File::options()
            .write(true)
            .open(path)
            .ok()?
            .set_modified(modified)
            .ok()
    }

    fn folder(name: &str) -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!("cena-pictures-{}-{name}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let _ = std::fs::create_dir_all(&dir);
        dir
    }

    #[test]
    fn a_changed_picture_is_read_again_and_an_unchanged_one_kept() {
        let dir = folder("changed");
        let path = dir.join("a.png");
        let then = SystemTime::UNIX_EPOCH + Duration::from_secs(1_000_000);
        png(&path, 2, then).unwrap();
        let (context, start) = (egui::Context::default(), Instant::now());
        let at = path.to_string_lossy().into_owned();

        let first = picture_at(&context, &at, start).unwrap();
        let later = start + LOOK_AGAIN * 2;
        let kept = picture_at(&context, &at, later).unwrap();
        assert_eq!(kept.id(), first.id(), "unchanged: the same texture");

        png(&path, 5, then + Duration::from_mins(1)).unwrap();
        let soon = picture_at(&context, &at, later + Duration::from_millis(10)).unwrap();
        assert_eq!(soon.size()[0], 2, "not looked at again within the second");
        let again = picture_at(&context, &at, later + LOOK_AGAIN * 2).unwrap();
        assert_eq!(again.size()[0], 5, "changed: read again");
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// The crate review of 2026-10-01, GU-C-1: a picture wider than the
    /// largest texture the window can hold killed the window thread, and with
    /// it every character; its path is saved, so again at every start. It is
    /// shrunk to fit.
    #[test]
    fn a_picture_too_big_for_a_texture_is_shrunk_to_fit() {
        let dir = folder("big");
        let path = dir.join("big.png");
        let context = egui::Context::default();
        let most = context.input(|input| input.max_texture_side);
        let wide = u32::try_from(most).unwrap() + 1000;
        png(&path, wide, SystemTime::now()).unwrap();
        let at = path.to_string_lossy().into_owned();
        let read = picture_at(&context, &at, Instant::now()).expect("read");
        assert!(read.size()[0] <= most, "{:?} within {most}", read.size());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_picture_that_was_not_there_is_read_once_it_is() {
        let dir = folder("missing");
        let path = dir.join("late.png");
        let (context, start) = (egui::Context::default(), Instant::now());
        let at = path.to_string_lossy().into_owned();
        assert!(picture_at(&context, &at, start).is_none());
        png(&path, 3, SystemTime::now()).unwrap();
        let read = picture_at(&context, &at, start + LOOK_AGAIN * 2);
        assert_eq!(read.map(|t| t.size()[0]), Some(3));
        let _ = std::fs::remove_dir_all(&dir);
    }
}
