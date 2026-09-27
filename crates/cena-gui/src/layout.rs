//! A play window's layout (`plan/47` step 6): where each pane sits inside
//! it, and the grid their edges snap to, kept by character name -- a
//! `SessionId` starts again at 0 every run (`plan/23` §D1a), a name does not.
//!
//! The author's model (`plan/28` §7d): *"free rects, with the grid as a snap
//! target"*, the pitch adjustable, since a fixed one was the complaint about
//! Saga (§7d.1). A rect is kept from the pane area's top left, so a window
//! moved across the screen keeps its layout.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use egui::{Rect, Vec2, pos2};
use serde::{Deserialize, Serialize};

/// A pane of a play window.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub(crate) enum Pane {
    /// The game's text.
    Story,
    /// The four vitals.
    Vitals,
    /// The room the character stands in.
    Room,
    /// Hydra's own messages.
    Hydra,
    /// What the hunt is doing (`plan/47` step 8).
    Hunt,
}

impl Pane {
    /// Every pane, in the order they are drawn.
    pub(crate) const ALL: [Pane; 5] = [
        Pane::Story,
        Pane::Vitals,
        Pane::Hunt,
        Pane::Room,
        Pane::Hydra,
    ];

    /// Its title bar's words.
    pub(crate) fn title(self) -> &'static str {
        match self {
            Pane::Story => "Story",
            Pane::Vitals => "Vitals",
            Pane::Room => "Room",
            Pane::Hydra => "Hydra",
            Pane::Hunt => "Hunt",
        }
    }
}

/// The smallest a pane can be made.
pub(crate) const SMALLEST: Vec2 = Vec2::new(120.0, 60.0);

/// The grid a new layout starts with, in points.
pub(crate) const GRID: f32 = 10.0;

/// The version of the layout file this build writes and reads.
const VERSION: u32 = 1;

/// Where a play window's panes sit.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub(crate) struct Layout {
    /// This file's shape; a file of another is not read.
    version: u32,
    /// The grid's pitch in points; 0 for none.
    pub(crate) grid: f32,
    /// Each pane's rect from the pane area's top left: x, y, width, height.
    panes: BTreeMap<Pane, [f32; 4]>,
}

impl Layout {
    /// The first layout, for a pane area `area` across: the story on the
    /// left, and down the right the vitals, the hunt, the room and Hydra's
    /// messages, their edges on the grid.
    pub(crate) fn fitted(area: Vec2) -> Self {
        let on_grid = |value: f32| (value / GRID).round() * GRID;
        let split = on_grid(area.x * 0.66).max(SMALLEST.x);
        let side = (area.x - split).max(SMALLEST.x);
        let vitals = 130.0;
        let hunt = 110.0;
        let room = on_grid(((area.y - vitals - hunt) * 0.5).max(SMALLEST.y));
        let hydra = (area.y - vitals - hunt - room).max(SMALLEST.y);
        let mut layout = Self {
            version: VERSION,
            grid: GRID,
            panes: BTreeMap::new(),
        };
        layout.set(Pane::Story, rect(0.0, 0.0, split, area.y.max(SMALLEST.y)));
        layout.set(Pane::Vitals, rect(split, 0.0, side, vitals));
        layout.set(Pane::Hunt, rect(split, vitals, side, hunt));
        layout.set(Pane::Room, rect(split, vitals + hunt, side, room));
        layout.set(Pane::Hydra, rect(split, vitals + hunt + room, side, hydra));
        layout
    }

    /// Give each pane this layout never placed -- one saved before the pane
    /// existed -- the place `fitted` gives it.
    pub(crate) fn fill_from(&mut self, fitted: &Layout) {
        for pane in Pane::ALL {
            if !self.panes.contains_key(&pane) {
                self.set(pane, fitted.rect(pane));
            }
        }
    }

    /// Whether every pane has a place.
    pub(crate) fn complete(&self) -> bool {
        Pane::ALL.iter().all(|pane| self.panes.contains_key(pane))
    }

    /// Where `pane` sits, from the pane area's top left; a pane this layout
    /// never placed is given a small place at the top left.
    pub(crate) fn rect(&self, pane: Pane) -> Rect {
        self.panes.get(&pane).map_or_else(
            || rect(0.0, 0.0, SMALLEST.x * 2.0, SMALLEST.y * 2.0),
            |&[x, y, width, height]| rect(x, y, width, height),
        )
    }

    /// Put `pane` at `at`.
    pub(crate) fn set(&mut self, pane: Pane, at: Rect) {
        self.panes
            .insert(pane, [at.min.x, at.min.y, at.width(), at.height()]);
    }

    /// `character`'s saved layout in `dir`; `None` when there is none, or it
    /// cannot be read, when a fitted one does.
    pub(crate) fn load(dir: &Path, character: &str) -> Option<Self> {
        let text = std::fs::read_to_string(file(dir, character)).ok()?;
        serde_json::from_str::<Self>(&text)
            .ok()
            .filter(|layout| layout.version == VERSION)
    }

    /// Save this as `character`'s layout in `dir`.
    ///
    /// # Errors
    ///
    /// The folder could not be made or the file written.
    pub(crate) fn save(&self, dir: &Path, character: &str) -> std::io::Result<()> {
        std::fs::create_dir_all(dir)?;
        let text = serde_json::to_string_pretty(self).map_err(std::io::Error::other)?;
        std::fs::write(file(dir, character), text)
    }
}

fn rect(x: f32, y: f32, width: f32, height: f32) -> Rect {
    Rect::from_min_size(pos2(x, y), Vec2::new(width, height))
}

/// `character`'s layout file: its name in lower case, letters and digits
/// only, so the same character is one file on every filesystem (the
/// character store's lesson: one file on NTFS was two on ext4).
fn file(dir: &Path, character: &str) -> PathBuf {
    let name: String = character
        .chars()
        .filter(char::is_ascii_alphanumeric)
        .map(|c| c.to_ascii_lowercase())
        .collect();
    dir.join(format!("{name}.json"))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The first layout fills the area, no pane over another.
    #[test]
    fn the_first_layout_tiles_the_area() {
        let area = Vec2::new(900.0, 600.0);
        let layout = Layout::fitted(area);
        let rects: Vec<Rect> = Pane::ALL.iter().map(|pane| layout.rect(*pane)).collect();
        let covered: f32 = rects.iter().map(|r| r.width() * r.height()).sum();
        assert!((covered - area.x * area.y).abs() < 1.0, "{covered}");
        for (i, a) in rects.iter().enumerate() {
            for b in rects.iter().skip(i + 1) {
                assert!(a.intersect(*b).area() < 0.5, "{a:?} over {b:?}");
            }
        }
        assert!(
            (layout.rect(Pane::Story).width() % GRID).abs() < 0.01,
            "on the grid"
        );
    }

    /// A layout saved before a pane existed gets that pane where a fitted
    /// layout puts it, and keeps every other where the player left it.
    #[test]
    fn a_pane_new_since_the_layout_was_saved_is_fitted_in() {
        let area = Vec2::new(900.0, 600.0);
        let fitted = Layout::fitted(area);
        let mut old = fitted.clone();
        old.panes.remove(&Pane::Hunt);
        old.set(Pane::Room, rect(0.0, 0.0, 200.0, 100.0));
        assert!(!old.complete());
        old.fill_from(&fitted);
        assert!(old.complete());
        assert_eq!(old.rect(Pane::Hunt), fitted.rect(Pane::Hunt));
        assert_eq!(old.rect(Pane::Room), rect(0.0, 0.0, 200.0, 100.0));
    }

    /// One file per character on every filesystem: its name in lower case,
    /// letters and digits only -- asserted here rather than through a load,
    /// which Windows' case-blind files would pass either way.
    #[test]
    fn a_characters_file_is_its_name_in_lower_case() {
        let dir = Path::new("layouts");
        assert_eq!(file(dir, "Ashryn"), dir.join("ashryn.json"));
        assert_eq!(file(dir, "GS3:Ashryn"), dir.join("gs3ashryn.json"));
    }

    #[test]
    fn a_layout_is_kept_by_name_whatever_its_case() {
        let dir = std::env::temp_dir().join(format!("cena-layout-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let mut layout = Layout::fitted(Vec2::new(900.0, 600.0));
        layout.set(Pane::Room, rect(10.0, 20.0, 300.0, 200.0));
        layout.grid = 16.0;
        layout.save(&dir, "Ashryn").expect("saved");
        assert_eq!(Layout::load(&dir, "ASHRYN").as_ref(), Some(&layout));
        assert_eq!(Layout::load(&dir, "Baelor"), None, "never saved");
        std::fs::write(file(&dir, "Lorwyn"), "{ not json").expect("written");
        assert_eq!(Layout::load(&dir, "Lorwyn"), None, "unreadable: fitted");
        let later = serde_json::to_string(&layout)
            .expect("written")
            .replace("\"version\":1", "\"version\":2");
        std::fs::write(file(&dir, "Orsen"), later).expect("written");
        assert_eq!(Layout::load(&dir, "Orsen"), None, "another version: fitted");
        let _ = std::fs::remove_dir_all(&dir);
    }
}
