//! The room as the game describes it, in one widget: the author, 2026-09-28,
//! with the room window as Wrayth draws it --
//!
//! ```text
//! [Rawknuckle's, Watering Hole] (7503251)
//! Low eaves, stained black with smoke, ...  You also see a tree hawk-eagle
//! that is flying around, a raw-boned halfling tavernkeeper and a gaunt
//! masked artificer.
//! Also here: Regyy
//! Obvious exits: east, out
//! ```
//!
//! -- *"So it should display just like this."* Each part can be turned off
//! on the widget's own page ([`RoomParts`]), and the creatures stand apart
//! from the objects when asked: *"take the objects and break it up into
//! creatures / objects based on the pushBold wrapping it"*, each on its own
//! line.

use cena_session::{RoomItem, Snapshot};
use cena_ui::StyledRun;
use egui::{Color32, RichText};
use serde::{Deserialize, Serialize};

use crate::text::{self, AMBER, CREATURE, OBJECT, PLAYER};

/// Which parts of the room the Room widget shows, and whether the creatures
/// stand apart from the objects.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[expect(
    clippy::struct_excessive_bools,
    reason = "a page of switches: each part on or off on its own, as the player picks"
)]
pub(crate) struct RoomParts {
    /// Its name, and the game's number for it.
    pub(crate) title: bool,
    /// What it looks like.
    pub(crate) description: bool,
    /// What else is here: the game's *You also see*.
    pub(crate) objects: bool,
    /// The creatures, the bold ones among what is here; drawn on their own
    /// only when `apart`.
    pub(crate) creatures: bool,
    /// Who else is here.
    pub(crate) players: bool,
    /// The ways out.
    pub(crate) exits: bool,
    /// The creatures and the objects each on a line of their own, rather
    /// than the game's sentence run on after the description.
    pub(crate) apart: bool,
}

impl Default for RoomParts {
    /// Every part, as the game joins them.
    fn default() -> Self {
        Self {
            title: true,
            description: true,
            objects: true,
            creatures: true,
            players: true,
            exits: true,
            apart: false,
        }
    }
}

/// The room in `snapshot`, its `parts`.
pub(super) fn room(ui: &mut egui::Ui, snapshot: Option<&Snapshot>, parts: RoomParts) {
    let Some(snapshot) = snapshot else {
        ui.weak("Room unknown");
        return;
    };
    let room = &snapshot.state.room;
    if parts.title {
        let title = room.title.as_deref().map_or("Room unknown", bare);
        let number = room
            .id
            .as_deref()
            .map_or_else(String::new, |id| format!(" ({id})"));
        ui.label(
            RichText::new(format!("[{title}]{number}"))
                .color(AMBER)
                .strong(),
        );
    }
    let mut prose: Vec<StyledRun> = Vec::new();
    if parts.description
        && let Some(description) = &room.description
    {
        prose.extend(styled(description, false));
    }
    let seen = room.component("room objs");
    if parts.objects
        && !parts.apart
        && let Some(seen) = seen
    {
        if !prose.is_empty() {
            prose.push(plain("  "));
        }
        prose.extend(styled(seen, true));
    }
    if !prose.is_empty() {
        ui.label(text::job(&prose, ui.style()));
    }
    if parts.apart && seen.is_some() {
        if parts.objects {
            listed(ui, "You also see", &room.objects, OBJECT);
        }
        if parts.creatures {
            listed(ui, "Creatures", &room.creatures, CREATURE);
        }
    }
    if parts.players && !room.players.is_empty() {
        ui.horizontal_wrapped(|ui| {
            ui.label("Also here:");
            for player in &room.players {
                match cena_ui::room_player(&player.text, &snapshot.triggers, &snapshot.state) {
                    Some(runs) => ui.label(text::job(&runs, ui.style())),
                    None => ui.colored_label(PLAYER, &player.text),
                };
            }
        });
    }
    if parts.exits {
        match (room.component("room exits"), &room.exits) {
            (Some(said), _) => {
                ui.label(text::job(&styled(said, false), ui.style()));
            }
            (None, Some(exits)) if exits.is_empty() => {
                ui.label("Obvious exits: none");
            }
            (None, Some(exits)) => {
                ui.label(format!("Obvious exits: {}", exits.join(", ")));
            }
            (None, None) => {
                ui.weak("Exits unknown");
            }
        }
    }
}

/// The room's name without the number the game adds to it when the player
/// has room numbers shown: `Rawknuckle's, Watering Hole - 7503251`.
fn bare(title: &str) -> &str {
    match title.rsplit_once(" - ") {
        Some((name, number))
            if !number.is_empty() && number.chars().all(|c| c.is_ascii_digit()) =>
        {
            name
        }
        _ => title,
    }
}

/// A component's runs as the window draws them; `creatures`, its bold runs
/// in the creatures' colour, as the game's bold marks them.
fn styled(runs: &cena_session::Runs, creatures: bool) -> Vec<StyledRun> {
    runs.runs
        .iter()
        .map(|run| {
            let bold = run.style.bold_depth > 0;
            StyledRun {
                text: run.text.clone(),
                bold,
                monospace: run.style.mono,
                preset: if creatures && bold {
                    Some("monsterbold".to_owned())
                } else {
                    run.style.preset.clone()
                },
                ..StyledRun::default()
            }
        })
        .collect()
}

/// Words with no style of their own.
fn plain(text: &str) -> StyledRun {
    StyledRun {
        text: text.to_owned(),
        ..StyledRun::default()
    }
}

/// `items` on one line after `label`, in `color`; nothing when there are
/// none.
fn listed(ui: &mut egui::Ui, label: &str, items: &[RoomItem], color: Color32) {
    if items.is_empty() {
        return;
    }
    ui.horizontal_wrapped(|ui| {
        ui.label(format!("{label}:"));
        for (at, item) in items.iter().enumerate() {
            let text = match &item.status {
                Some(status) => format!("{} ({status})", item.text),
                None => item.text.clone(),
            };
            let comma = if at + 1 < items.len() { "," } else { "" };
            ui.colored_label(color, format!("{text}{comma}"));
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use cena_session::{Frame, Run, Runs, Style};
    use egui_kittest::Harness;
    use egui_kittest::kittest::Queryable as _;

    /// Text in runs, each bold (a creature) or not.
    fn runs(parts: &[(&str, bool)]) -> Runs {
        let runs = parts
            .iter()
            .map(|(text, bold)| Run {
                text: (*text).to_owned(),
                style: Style {
                    bold_depth: u16::from(*bold),
                    ..Style::default()
                },
                link: None,
                inner_link: None,
            })
            .collect();
        Runs { runs }
    }

    fn item(text: &str) -> RoomItem {
        RoomItem {
            id: text.len().to_string(),
            noun: text.rsplit(' ').next().unwrap_or(text).to_owned(),
            text: text.to_owned(),
            before: None,
            after: None,
            status: None,
        }
    }

    /// The room of the author's example, as the game sends it.
    fn watering_hole() -> Snapshot {
        let mut snapshot = crate::fixture::snapshot();
        let state = &mut snapshot.state;
        state.apply(&Frame::Component {
            id: "room objs".to_owned(),
            body: runs(&[
                ("You also see ", false),
                ("a tree hawk-eagle", true),
                (" that is flying around, a raw-boned halfling tavernkeeper and a gaunt masked artificer.", false),
            ]),
        });
        state.apply(&Frame::Component {
            id: "room exits".to_owned(),
            body: runs(&[("Obvious exits: east, out", false)]),
        });
        let room = &mut state.room;
        room.id = Some("7503251".to_owned());
        room.title = Some("Rawknuckle's, Watering Hole - 7503251".to_owned());
        room.description = Some(runs(&[(
            "Low eaves, stained black with smoke, do little to keep out the icy chill.",
            false,
        )]));
        room.creatures = vec![item("a tree hawk-eagle")];
        room.objects = vec![
            item("a raw-boned halfling tavernkeeper"),
            item("a gaunt masked artificer"),
        ];
        room.players = vec![item("Regyy")];
        snapshot
    }

    fn drawn(parts: RoomParts) -> Harness<'static, ()> {
        let snapshot = watering_hole();
        let mut harness = Harness::builder()
            .with_size((420.0, 260.0))
            .build_ui(move |ui| room(ui, Some(&snapshot), parts));
        harness.run();
        harness
    }

    const JOINED: &str = "Low eaves, stained black with smoke, do little to keep out the icy chill.  You also see a tree hawk-eagle that is flying around, a raw-boned halfling tavernkeeper and a gaunt masked artificer.";

    /// As the game draws it: its name and number, the description with what
    /// is here run on after it, who else is here, and the ways out.
    #[test]
    fn the_room_is_drawn_as_the_game_joins_it() {
        let harness = drawn(RoomParts::default());
        for said in [
            "[Rawknuckle's, Watering Hole] (7503251)",
            JOINED,
            "Also here:",
            "Regyy",
            "Obvious exits: east, out",
        ] {
            assert!(harness.query_by_label(said).is_some(), "{said}");
        }
        assert!(harness.query_by_label("Creatures:").is_none());
    }

    /// Each part turned off is left out, and nothing else moves.
    #[test]
    fn a_part_turned_off_is_left_out() {
        let harness = drawn(RoomParts {
            title: false,
            description: false,
            players: false,
            ..RoomParts::default()
        });
        assert!(harness.query_by_label_contains("Rawknuckle").is_none());
        assert!(harness.query_by_label("Also here:").is_none());
        assert!(
            harness
                .query_by_label("You also see a tree hawk-eagle that is flying around, a raw-boned halfling tavernkeeper and a gaunt masked artificer.")
                .is_some(),
            "what is here, alone"
        );
        assert!(harness.query_by_label("Obvious exits: east, out").is_some());
    }

    /// Apart, the objects and the creatures each have a line of their own,
    /// split by the game's bold, and each can be left out.
    #[test]
    fn creatures_apart_have_a_line_of_their_own() {
        let harness = drawn(RoomParts {
            apart: true,
            ..RoomParts::default()
        });
        assert!(
            harness
                .query_by_label(
                    "Low eaves, stained black with smoke, do little to keep out the icy chill."
                )
                .is_some(),
            "the description alone"
        );
        assert!(harness.query_by_label("You also see:").is_some());
        assert!(harness.query_by_label("a gaunt masked artificer").is_some());
        assert!(harness.query_by_label("Creatures:").is_some());
        assert!(harness.query_by_label("a tree hawk-eagle").is_some());
        let without = drawn(RoomParts {
            apart: true,
            creatures: false,
            ..RoomParts::default()
        });
        assert!(without.query_by_label("Creatures:").is_none());
        assert!(without.query_by_label("You also see:").is_some());
        let without = drawn(RoomParts {
            apart: true,
            objects: false,
            ..RoomParts::default()
        });
        assert!(without.query_by_label("You also see:").is_none());
        assert!(without.query_by_label("Creatures:").is_some());
    }

    /// `parts` of the room, rendered and compared with the committed image
    /// `name`.
    fn rendered(name: &str, parts: RoomParts) {
        let snapshot = watering_hole();
        let mut harness = Harness::builder()
            .with_size((420.0, 220.0))
            .wgpu()
            .build_ui(move |ui| room(ui, Some(&snapshot), parts));
        harness.run();
        harness.snapshot(name);
    }

    /// The Room widget as a player sees it, joined as the game joins it.
    #[test]
    fn the_room_as_drawn() {
        rendered("room_joined", RoomParts::default());
    }

    /// The same room with its creatures apart from its objects.
    #[test]
    fn the_room_apart_as_drawn() {
        rendered(
            "room_apart",
            RoomParts {
                apart: true,
                ..RoomParts::default()
            },
        );
    }
}
