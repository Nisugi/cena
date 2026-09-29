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

/// A picture as last read, and when its file was last looked at.
#[derive(Clone)]
struct Kept {
    texture: Option<egui::TextureHandle>,
    /// The file's modification time when read; `None` when there was no
    /// file to read.
    modified: Option<SystemTime>,
    looked: Instant,
}

/// The picture at `path` as a texture, or `None` when it cannot be read.
pub(crate) fn picture(context: &egui::Context, path: &str) -> Option<egui::TextureHandle> {
    picture_at(context, path, Instant::now())
}

/// [`picture`], at `now`.
fn picture_at(context: &egui::Context, path: &str, now: Instant) -> Option<egui::TextureHandle> {
    let id = egui::Id::new(("picture", path));
    let kept = context.data(|data| data.get_temp::<Kept>(id));
    if let Some(kept) = &kept
        && now.saturating_duration_since(kept.looked) < LOOK_AGAIN
    {
        return kept.texture.clone();
    }
    let modified = std::fs::metadata(path).and_then(|m| m.modified()).ok();
    let texture = match kept {
        Some(kept) if kept.modified == modified => kept.texture,
        _ => modified.and_then(|_| read(context, path)),
    };
    let fresh = Kept {
        texture: texture.clone(),
        modified,
        looked: now,
    };
    context.data_mut(|data| data.insert_temp(id, fresh));
    texture
}

/// A PNG, or any picture the `image` crate reads, as a texture.
fn read(context: &egui::Context, path: &str) -> Option<egui::TextureHandle> {
    let bytes = std::fs::read(Path::new(path)).ok()?;
    let image = image::load_from_memory(&bytes).ok()?.to_rgba8();
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
