//! Where each window was, kept: the hub, every play window, the settings
//! menu and every log window open where the player last left them, at the
//! size they left them (the author, 2026-09-29: *"next time I open it the
//! size and position have reset"*).
//!
//! One file in the data folder, `windows.json`, a place per window by a key:
//! `hub`, `settings`, `play:GS3:nisugi`, `log:GS3:nisugi`. A play window is
//! kept by game and name, as its layout is, so it comes back after a restart
//! as well as after a close.
//!
//! # A window is placed once, as it opens
//!
//! egui sends a window its builder's position and size again whenever the
//! builder changes. So the place a window opened with is the one its builder
//! keeps for as long as it stays open ([`Placements::builder`]), and where
//! the player moves it is only noted ([`Placements::note`]). Were the
//! builder to follow the window, every drag would be answered with a move to
//! where it already was, a frame late.
//!
//! # Written a moment after it settles
//!
//! A drag is many frames. The file is written once the place has been still
//! for [`SETTLE`], and when a window closes; not on every frame.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

/// The file, in the data folder.
pub(crate) const FILE: &str = "windows.json";

/// How long a place must stay still before it is written.
pub(crate) const SETTLE: Duration = Duration::from_secs(1);

/// Where one window was, in egui points: its outer corner, its inner size,
/// and whether it was maximised.
#[derive(Clone, Copy, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub(crate) struct Place {
    /// The outer top-left corner, across all monitors.
    pub(crate) x: f32,
    /// The outer top-left corner, across all monitors.
    pub(crate) y: f32,
    /// The inner width.
    pub(crate) width: f32,
    /// The inner height.
    pub(crate) height: f32,
    /// Maximised: the corner and size are where it goes when it is not.
    #[serde(default)]
    pub(crate) maximized: bool,
}

/// Every window's place, kept.
#[derive(Debug, Default)]
pub(crate) struct Placements {
    /// Where they are written; `None`, and they are only held.
    file: Option<PathBuf>,
    kept: BTreeMap<String, Place>,
    /// The place each open window opened with, fixed until it closes.
    opened: BTreeMap<String, Option<Place>>,
    /// A place changed and is not yet written: when.
    changed: Option<Instant>,
}

/// The key a character's window is kept by: `play:GS3:nisugi`.
#[must_use]
pub(crate) fn key(kind: &str, game: &str, name: &str) -> String {
    format!("{kind}:{game}:{}", name.to_lowercase())
}

impl Placements {
    /// The places kept in `data`'s file. A missing or unreadable file keeps
    /// nothing, and every window opens at its default.
    #[must_use]
    pub(crate) fn load(data: &Path) -> Self {
        let file = data.join(FILE);
        let kept = std::fs::read_to_string(&file)
            .ok()
            .and_then(|text| serde_json::from_str(&text).ok())
            .unwrap_or_default();
        Self {
            file: Some(file),
            kept,
            ..Self::default()
        }
    }

    /// The builder for the window `key`: where it was last, or `size` at the
    /// system's choice of place the first time. Fixed while it stays open.
    pub(crate) fn builder(&mut self, key: &str, size: [f32; 2]) -> egui::ViewportBuilder {
        let place = *self
            .opened
            .entry(key.to_owned())
            .or_insert_with(|| self.kept.get(key).copied());
        let builder = egui::ViewportBuilder::default();
        match place {
            Some(place) => builder
                .with_position([place.x, place.y])
                .with_inner_size([place.width, place.height])
                .with_maximized(place.maximized),
            None => builder.with_inner_size(size),
        }
    }

    /// Where the window `key` is now, as its viewport says: noted, and
    /// written once it settles. A minimised window, or one whose system
    /// will not say where it is (Wayland), changes nothing.
    pub(crate) fn note(&mut self, key: &str, seen: &egui::ViewportInfo, now: Instant) {
        if seen.minimized == Some(true) {
            return;
        }
        let maximized = seen.maximized == Some(true);
        // Maximised, the window fills the screen: keep where it goes back
        // to, and that it was maximised.
        let place = if let (true, Some(before)) = (maximized, self.kept.get(key)) {
            Place {
                maximized: true,
                ..*before
            }
        } else {
            let (Some(outer), Some(inner)) = (seen.outer_rect, seen.inner_rect) else {
                return;
            };
            Place {
                x: outer.min.x,
                y: outer.min.y,
                width: inner.width(),
                height: inner.height(),
                maximized,
            }
        };
        if self.kept.get(key) != Some(&place) {
            self.kept.insert(key.to_owned(), place);
            self.changed = Some(now);
        }
    }

    /// The window `key` closed: it opens where it was next time, and its
    /// place is written now.
    pub(crate) fn closed(&mut self, key: &str) {
        self.opened.remove(key);
        if self.changed.is_some() {
            self.save();
        }
    }

    /// Write what changed if it has been still for [`SETTLE`]; otherwise
    /// how long until it will have been, for the window to wake then.
    pub(crate) fn save_due(&mut self, now: Instant) -> Option<Duration> {
        let changed = self.changed?;
        let left = SETTLE.saturating_sub(now.saturating_duration_since(changed));
        if left.is_zero() {
            self.save();
            None
        } else {
            Some(left)
        }
    }

    /// Write every place kept. A failure is not worth stopping for: the
    /// windows open at their defaults next time, as they did before.
    pub(crate) fn save(&mut self) {
        self.changed = None;
        let Some(file) = &self.file else { return };
        let Some(data) = file.parent() else { return };
        if let Ok(text) = serde_json::to_string_pretty(&self.kept) {
            let _ = cena_session::store::save_text(data, file, &text);
        }
    }

    /// Where the window `key` was last, if anywhere.
    #[cfg(test)]
    pub(crate) fn kept(&self, key: &str) -> Option<Place> {
        self.kept.get(key).copied()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn seen(x: f32, y: f32, w: f32, h: f32) -> egui::ViewportInfo {
        egui::ViewportInfo {
            outer_rect: Some(egui::Rect::from_min_size(
                egui::pos2(x, y),
                egui::vec2(w + 16.0, h + 39.0),
            )),
            inner_rect: Some(egui::Rect::from_min_size(
                egui::pos2(x + 8.0, y + 31.0),
                egui::vec2(w, h),
            )),
            minimized: Some(false),
            maximized: Some(false),
            ..egui::ViewportInfo::default()
        }
    }

    fn folder(name: &str) -> PathBuf {
        let data =
            std::env::temp_dir().join(format!("cena-placement-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&data);
        data
    }

    /// Moved, sized and closed, a window opens there again: after a close,
    /// and after Hydra is started again.
    #[test]
    fn a_window_opens_where_it_was_left() {
        let data = folder("left");
        let key = key("play", "GS3", "Nisugi");
        let now = Instant::now();
        let mut places = Placements::load(&data);
        let first = places.builder(&key, [980.0, 680.0]);
        assert_eq!(first.position, None, "the first time, the system places it");
        assert_eq!(first.inner_size, Some(egui::vec2(980.0, 680.0)));

        places.note(&key, &seen(1200.0, -300.0, 700.0, 900.0), now);
        places.closed(&key);

        let again = places.builder(&key, [980.0, 680.0]);
        assert_eq!(again.position, Some(egui::pos2(1200.0, -300.0)));
        assert_eq!(again.inner_size, Some(egui::vec2(700.0, 900.0)));

        let restarted = Placements::load(&data).builder(&key, [980.0, 680.0]);
        assert_eq!(
            restarted.position,
            Some(egui::pos2(1200.0, -300.0)),
            "written at close"
        );
        let _ = std::fs::remove_dir_all(&data);
    }

    /// While a window stays open its builder does not follow it, or every
    /// drag would be answered with a move a frame late.
    #[test]
    fn an_open_windows_builder_stays_as_it_opened() {
        let key = key("log", "GS3", "Nisugi");
        let now = Instant::now();
        let mut places = Placements::default();
        let opened = places.builder(&key, [1000.0, 640.0]);
        places.note(&key, &seen(10.0, 20.0, 500.0, 400.0), now);
        assert_eq!(places.builder(&key, [1000.0, 640.0]), opened);
        assert_eq!(
            places.kept(&key).map(|p| (p.x, p.y)),
            Some((10.0, 20.0)),
            "but where it went is noted"
        );
    }

    /// Minimised says nothing of where it will come back; maximised keeps
    /// where it goes back to.
    #[test]
    fn minimised_changes_nothing_and_maximised_keeps_where_it_returns() {
        let key = "settings";
        let now = Instant::now();
        let mut places = Placements::default();
        places.note(key, &seen(100.0, 100.0, 760.0, 560.0), now);

        let mut minimised = seen(-32000.0, -32000.0, 160.0, 28.0);
        minimised.minimized = Some(true);
        places.note(key, &minimised, now);
        assert_eq!(places.kept(key).map(|p| p.x), Some(100.0));

        let mut maximised = seen(0.0, 0.0, 2560.0, 1400.0);
        maximised.maximized = Some(true);
        places.note(key, &maximised, now);
        let kept = places.kept(key).expect("kept");
        assert!(kept.maximized);
        assert_eq!((kept.x, kept.width), (100.0, 760.0));
    }

    /// A drag is written once it has been still a moment, not every frame.
    #[test]
    fn a_place_is_written_once_it_settles() {
        let data = folder("settle");
        let now = Instant::now();
        let mut places = Placements::load(&data);
        places.note("hub", &seen(5.0, 5.0, 560.0, 720.0), now);
        assert_eq!(places.save_due(now), Some(SETTLE));
        assert!(!data.join(FILE).exists());
        assert_eq!(places.save_due(now + SETTLE), None);
        assert!(data.join(FILE).exists());
        assert_eq!(
            places.save_due(now + SETTLE * 2),
            None,
            "nothing more to write"
        );
        let _ = std::fs::remove_dir_all(&data);
    }
}
