//! What is in the room, one kind to a widget: its creatures, its objects,
//! its players. The author, 2026-09-30: *"the objects window should not say
//! also here: inside the window, it should just list the items. It's
//! settings should have an option to list them horizontally separated by
//! commas or vertically"*, and the same of the players and the creatures.
//!
//! Each name is a link, as in the story and the Room widget (`described.rs`):
//! a click opens its menu, and an object is carried from it with the drag
//! key; only in the window's own character's widget, whose ids are its to
//! send.

use cena_ui::StyledRun;
use serde::{Deserialize, Serialize};

use super::Clicked;
use super::described::plain;
use crate::text;

/// How a room list is laid out.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum Listing {
    /// On a line, a comma between each, wrapped as the widget is wide.
    #[default]
    Across,
    /// One to a line.
    Down,
}

impl Listing {
    /// How the settings menu writes it, and names it.
    pub(crate) const CHOICES: [(Listing, &'static str, &'static str); 2] = [
        (Listing::Across, "across", "Across, with commas"),
        (Listing::Down, "down", "Down, one a line"),
    ];

    /// As the settings menu writes it.
    pub(crate) fn value(self) -> &'static str {
        Self::CHOICES
            .iter()
            .find(|(listing, ..)| *listing == self)
            .map_or("across", |(_, value, _)| value)
    }

    /// From the settings menu's `value`.
    pub(crate) fn of(value: &str) -> Option<Self> {
        Self::CHOICES
            .iter()
            .find(|(_, written, _)| *written == value)
            .map(|(listing, ..)| *listing)
    }
}

/// `items`, each its runs, laid out as `listing` says: `unknown` until the
/// game has said, `none` when it said there are none. Their links live when
/// `own`. The link clicked, if one was.
pub(super) fn list(
    ui: &mut egui::Ui,
    items: Option<Vec<Vec<StyledRun>>>,
    listing: Listing,
    own: bool,
) -> Option<Clicked> {
    let Some(items) = items else {
        ui.weak("unknown");
        return None;
    };
    if items.is_empty() {
        ui.weak("none");
        return None;
    }
    let lines: Vec<Vec<StyledRun>> = match listing {
        Listing::Down => items,
        Listing::Across => {
            let mut line = Vec::new();
            for (at, item) in items.into_iter().enumerate() {
                if at > 0 {
                    line.push(plain(", "));
                }
                line.extend(item);
            }
            vec![line]
        }
    };
    let mut clicked = None;
    for runs in &lines {
        let job = text::job(runs, ui.style());
        if own && runs.iter().any(|run| run.link.is_some()) {
            clicked = clicked
                .take()
                .or_else(|| text::linked(ui, job, runs).map(Clicked::from));
        } else {
            ui.label(job);
        }
    }
    clicked
}

#[cfg(test)]
mod tests {
    use cena_ui::StyledRun;
    use egui_kittest::Harness;
    use egui_kittest::kittest::Queryable as _;

    use super::{Listing, list};

    /// Two names, drawn as `listing` lays them out.
    fn drawn(items: Option<Vec<Vec<StyledRun>>>, listing: Listing) -> Harness<'static> {
        let mut harness = Harness::builder()
            .with_size((300.0, 120.0))
            .build_ui(move |ui| {
                let _ = list(ui, items.clone(), listing, true);
            });
        harness.run();
        harness
    }

    /// Across, the names on one line with commas between; down, one to a
    /// line; no label either way (the author, 2026-09-30). Unknown and none
    /// say so.
    #[test]
    fn a_room_list_goes_across_or_down() {
        let name = |text: &str| {
            vec![StyledRun {
                text: text.to_owned(),
                ..StyledRun::default()
            }]
        };
        let two = || Some(vec![name("a kobold"), name("a grey rat")]);
        let across = drawn(two(), Listing::Across);
        assert!(across.query_by_label("a kobold, a grey rat").is_some());
        let down = drawn(two(), Listing::Down);
        assert!(down.query_by_label("a kobold").is_some());
        assert!(down.query_by_label("a grey rat").is_some());
        assert!(
            drawn(None, Listing::Across)
                .query_by_label("unknown")
                .is_some()
        );
        assert!(
            drawn(Some(Vec::new()), Listing::Down)
                .query_by_label("none")
                .is_some()
        );
    }
}
