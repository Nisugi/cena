//! A doll picture's overlays: the **Doll plus** (`plan/55` §2a, step 5).
//!
//! `VellumFE`'s naming, kept so art made for either client works in both
//! (`reference/VellumFE/src/config/pool.rs:306-392`): beside the picture
//! `<picture>.png`, an overlay is `<picture>_<part>_<level>.png`, the level
//! one of `healthy`, `injury1`-`3` and `scar1`-`3`, the part the game's id
//! in any case. An overlay is the picture's own size and drawn over all of
//! it, not at the part's anchor. A picture with no overlays of its own
//! borrows its group's, the name before its last `_`: `nisugi_bow` uses
//! `nisugi_*`'s.
//!
//! Where a part has an overlay for what it shows, the overlay is drawn in
//! place of its dot; where it has none, the dot stays (`plan/55` §2a).
//! Only what is shown is read (`pictures.rs`): the folder's listing, kept
//! until the folder changes, and an overlay when its part shows its level.

use std::collections::BTreeMap;
use std::path::Path;
use std::sync::Arc;

use crate::widget::doll::PARTS;

/// The levels an overlay is drawn for, as `VellumFE` names them in a file
/// (`reference/VellumFE/src/config/skins.rs:644-669`): whole, three wounds,
/// three scars.
pub(crate) const LEVELS: [&str; 7] = [
    "healthy", "injury1", "injury2", "injury3", "scar1", "scar2", "scar3",
];

/// A picture's overlays: by part id and level (an index into [`LEVELS`]),
/// each file's path.
pub(crate) type Layers = BTreeMap<(&'static str, usize), String>;

/// The overlay a file's name makes it: its picture's name, its part and its
/// level; `None` for a file that is not one.
fn overlay_of(stem: &str) -> Option<(&str, &'static str, usize)> {
    let mut pieces = stem.rsplitn(3, '_');
    let (level, part, picture) = (pieces.next()?, pieces.next()?, pieces.next()?);
    let level = LEVELS
        .iter()
        .position(|known| known.eq_ignore_ascii_case(level))?;
    let part = PARTS
        .iter()
        .find(|known| known.id.eq_ignore_ascii_case(part))?
        .id;
    Some((picture, part, level))
}

/// Whether the file at `path` is one of a doll picture's overlays rather
/// than a picture of its own.
pub(crate) fn is_layer(path: &Path) -> bool {
    path.file_stem()
        .and_then(|stem| stem.to_str())
        .and_then(overlay_of)
        .is_some()
}

/// The overlays of the picture named `stem` among the files `names` in
/// `folder`: its own, or, with none, its group's.
fn layers_of(stem: &str, names: &[String], folder: &Path) -> Layers {
    let of = |picture: &str| -> Layers {
        names
            .iter()
            .filter_map(|name| {
                let file = Path::new(name);
                let png = file
                    .extension()
                    .is_some_and(|x| x.eq_ignore_ascii_case("png"));
                let (whose, part, level) = overlay_of(file.file_stem()?.to_str()?)?;
                (png && whose.eq_ignore_ascii_case(picture)).then(|| {
                    (
                        (part, level),
                        folder.join(name).to_string_lossy().into_owned(),
                    )
                })
            })
            .collect()
    };
    let own = of(stem);
    match stem.rsplit_once('_') {
        Some((group, _)) if own.is_empty() => of(group),
        _ => own,
    }
}

/// The overlays of the picture at `picture`, kept as a picture is and
/// looked for again when its folder changes.
pub(crate) fn layers(context: &egui::Context, picture: &str) -> Arc<Layers> {
    let path = Path::new(picture);
    let (Some(folder), Some(stem)) = (path.parent(), path.file_stem().and_then(|s| s.to_str()))
    else {
        return Arc::default();
    };
    let watched = folder.to_string_lossy();
    crate::pictures::kept_watching(context, "layers", picture, &watched, |_| {
        let names: Vec<String> = std::fs::read_dir(folder)
            .ok()?
            .filter_map(Result::ok)
            .filter_map(|entry| entry.file_name().to_str().map(str::to_owned))
            .collect();
        Some(Arc::new(layers_of(stem, &names, folder)))
    })
    .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::{is_layer, layers_of};
    use std::path::Path;

    #[test]
    fn an_overlay_file_is_told_from_a_picture() {
        let is = |name: &str| is_layer(Path::new(name));
        assert!(is("nisugi_chest_injury2.png"));
        assert!(is("dwarf_ranger_leftArm_scar1.png"));
        assert!(is("nisugi_NSYS_Healthy.png"), "in any case");
        assert!(
            !is("nisugi_bow.png"),
            "a picture of the group's, not an overlay"
        );
        assert!(!is("dwarf_ranger.png"));
        assert!(!is("chest_injury2.png"), "no picture before the part");
    }

    /// A picture's own overlays; a picture with none borrows its group's;
    /// one with its own never borrows.
    #[test]
    fn a_picture_has_its_own_overlays_or_its_groups() {
        let names: Vec<String> = [
            "nisugi.png",
            "nisugi_chest_injury2.png",
            "nisugi_leftarm_scar1.png",
            "nisugi_bow.png",
            "nisugi_staff.png",
            "nisugi_staff_head_injury1.png",
            "nisugi_chest_injury2.txt",
        ]
        .map(str::to_owned)
        .to_vec();
        let folder = Path::new("dolls");
        let own = layers_of("nisugi", &names, folder);
        assert_eq!(own.len(), 2);
        assert!(own.contains_key(&("chest", 2)));
        assert!(own.contains_key(&("leftArm", 4)), "the game's spelling");
        let borrowed = layers_of("nisugi_bow", &names, folder);
        assert_eq!(borrowed, own, "the group's");
        let staff = layers_of("nisugi_staff", &names, folder);
        assert_eq!(
            staff.keys().collect::<Vec<_>>(),
            [&("head", 1)],
            "its own only"
        );
    }
}
