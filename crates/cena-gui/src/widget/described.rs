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
//!
//! **Its links are the story's** (the author, 2026-09-29: *"links are not
//! rendered in the room window, they should be"*): the game's own, in what
//! is here and the exits, and each creature, object and player drawn apart
//! a link to it by its id. A click opens its menu, and an object is carried
//! from it with the drag key, as in the story; only in the window's own
//! character's widget, whose ids are its to send.

use cena_session::{RoomItem, Snapshot};
use cena_ui::{RunLink, StyledRun};
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

/// The room in `snapshot`, its `parts`; its links live when `own`, the
/// window's own character's. The link clicked, if one was.
pub(super) fn room(
    ui: &mut egui::Ui,
    snapshot: Option<&Snapshot>,
    parts: RoomParts,
    (own, tags): (bool, Option<cena_session::targetid::Style>),
) -> Option<super::Clicked> {
    let Some(snapshot) = snapshot else {
        ui.weak("Room unknown");
        return None;
    };
    let mut clicked = None;
    let mut said = |ui: &mut egui::Ui, runs: &[StyledRun]| {
        let job = text::job(runs, ui.style());
        if own && runs.iter().any(|run| run.link.is_some()) {
            clicked = clicked
                .take()
                .or_else(|| text::linked(ui, job, runs).map(super::Clicked::from));
        } else {
            ui.label(job);
        }
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
        let seen = tagged(seen, snapshot, tags);
        prose.extend(styled(&seen, true));
    }
    if !prose.is_empty() {
        said(ui, &prose);
    }
    if parts.apart && seen.is_some() {
        if parts.objects {
            listed(&mut said, ui, "You also see", &room.objects, (OBJECT, None));
        }
        if parts.creatures {
            listed(
                &mut said,
                ui,
                "Creatures",
                &room.creatures,
                (CREATURE, tags.map(|style| (snapshot, style))),
            );
        }
    }
    if parts.players && !room.players.is_empty() {
        said(ui, &players(snapshot));
    }
    if parts.exits {
        match (room.component("room exits"), &room.exits) {
            (Some(exits), _) => said(ui, &styled(exits, false)),
            (None, Some(exits)) if exits.is_empty() => {
                ui.label("Obvious exits: none");
            }
            (None, Some(exits)) => said(ui, &ways_out(exits)),
            (None, None) => {
                ui.weak("Exits unknown");
            }
        }
    }
    clicked
}

/// Who else is here, each a link to them, painted as the triggers paint a
/// name.
fn players(snapshot: &Snapshot) -> Vec<StyledRun> {
    let mut runs = vec![plain("Also here: ")];
    for (at, player) in snapshot.state.room.players.iter().enumerate() {
        if at > 0 {
            runs.push(plain(", "));
        }
        runs.extend(player_runs(snapshot, player));
    }
    runs
}

/// A player in the room, a link to them, painted as the triggers paint a
/// name.
pub(super) fn player_runs(snapshot: &Snapshot, player: &RoomItem) -> Vec<StyledRun> {
    let link = Some(object(player));
    match cena_ui::room_player(&player.text, &snapshot.triggers, &snapshot.state) {
        Some(painted) => painted
            .into_iter()
            .map(|run| StyledRun {
                link: link.clone(),
                ..run
            })
            .collect(),
        None => vec![StyledRun {
            text: player.text.clone(),
            color: Some(colour(PLAYER)),
            link,
            ..StyledRun::default()
        }],
    }
}

/// What a creature's tag needs: the snapshot its mark is in, and the look
/// the player chose (`.targetid`).
pub(super) type Tags<'a> = Option<(&'a Snapshot, cena_session::targetid::Style)>;

/// A creature or an object in the room, in `color`, a link to it, its
/// tag after it when `tags` (`.targetid`), and what it is doing when the
/// game says (`dead`, `lying down`).
pub(super) fn item_runs(item: &RoomItem, (color, tags): (Color32, Tags<'_>)) -> Vec<StyledRun> {
    let mut runs = vec![StyledRun {
        text: item.text.clone(),
        color: Some(colour(color)),
        link: Some(object(item)),
        ..StyledRun::default()
    }];
    let marked = tags.and_then(|(snapshot, style)| {
        let creature = snapshot.state.creatures().get(item.id.parse().ok()?)?;
        let (mark, health) = cena_session::targetid::of(creature)?;
        Some(cena_session::targetid::tag(mark, style, Some(health)))
    });
    if let Some(tag) = marked {
        runs.push(plain(&format!(" ({tag})")));
    }
    if let Some(status) = &item.status {
        runs.push(plain(&format!(" ({status})")));
    }
    runs
}

/// The ways out the model holds, when the game sent no line of them, each
/// a link that goes there.
fn ways_out(exits: &[String]) -> Vec<StyledRun> {
    let mut runs = vec![plain("Obvious exits: ")];
    for (at, exit) in exits.iter().enumerate() {
        if at > 0 {
            runs.push(plain(", "));
        }
        runs.push(StyledRun {
            text: exit.clone(),
            link: Some(RunLink::Command {
                command: exit.clone(),
            }),
            ..StyledRun::default()
        });
    }
    runs
}

/// A link to `item` by its id, as the game's own link to it is.
fn object(item: &RoomItem) -> RunLink {
    RunLink::Object {
        exist: item.id.clone(),
        noun: item.noun.clone(),
        coord: None,
    }
}

/// `color` as a run's colour.
fn colour(color: Color32) -> String {
    crate::menu::hex([color.r(), color.g(), color.b()])
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

/// `runs` with each creature's tag after its name, as `style` makes it
/// from the marks in `snapshot`, when there is a style (`.targetid`).
fn tagged<'a>(
    runs: &'a cena_session::Runs,
    snapshot: &Snapshot,
    style: Option<cena_session::targetid::Style>,
) -> std::borrow::Cow<'a, cena_session::Runs> {
    let line = cena_session::Line::new("", runs.clone());
    let creature = |id| cena_session::targetid::of(snapshot.state.creatures().get(id)?);
    match style.and_then(|style| cena_session::targetid::tagged(&line, style, creature)) {
        Some(line) => std::borrow::Cow::Owned(line.runs),
        None => std::borrow::Cow::Borrowed(runs),
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
                link: run.link.as_ref().and_then(RunLink::of),
                ..StyledRun::default()
            }
        })
        .collect()
}

/// Words with no style of their own.
pub(super) fn plain(text: &str) -> StyledRun {
    StyledRun {
        text: text.to_owned(),
        ..StyledRun::default()
    }
}

/// `items` on one line after `label`, in `color`, each a link to it, drawn
/// by `said`; nothing when there are none.
fn listed(
    said: &mut impl FnMut(&mut egui::Ui, &[StyledRun]),
    ui: &mut egui::Ui,
    label: &str,
    items: &[RoomItem],
    look: (Color32, Tags<'_>),
) {
    if items.is_empty() {
        return;
    }
    let mut runs = vec![plain(&format!("{label}: "))];
    for (at, item) in items.iter().enumerate() {
        if at > 0 {
            runs.push(plain(", "));
        }
        runs.extend(item_runs(item, look));
    }
    said(ui, &runs);
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
            .build_ui(move |ui| {
                let _ = room(ui, Some(&snapshot), parts, (true, None));
            });
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
            "Also here: Regyy",
            "Obvious exits: east, out",
        ] {
            assert!(harness.query_by_label(said).is_some(), "{said}");
        }
        assert!(harness.query_by_label_contains("Creatures:").is_none());
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
        assert!(harness.query_by_label_contains("Also here").is_none());
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
        assert!(
            harness
                .query_by_label(
                    "You also see: a raw-boned halfling tavernkeeper, a gaunt masked artificer"
                )
                .is_some()
        );
        assert!(harness.query_by_label(CREATURES).is_some());
        let without = drawn(RoomParts {
            apart: true,
            creatures: false,
            ..RoomParts::default()
        });
        assert!(without.query_by_label_contains("Creatures").is_none());
        assert!(without.query_by_label_contains("You also see:").is_some());
        let without = drawn(RoomParts {
            apart: true,
            objects: false,
            ..RoomParts::default()
        });
        assert!(without.query_by_label_contains("You also see:").is_none());
        assert!(without.query_by_label(CREATURES).is_some());
    }

    /// The creatures' line, apart.
    const CREATURES: &str = "Creatures: a tree hawk-eagle";

    /// With `.targetid` on, each creature drawn apart has its tag after its
    /// name, the mark the registry handed it, and an object none (the
    /// author, 2026-09-30).
    #[test]
    fn a_creature_apart_has_its_tag() {
        let mut snapshot = watering_hole();
        // The hawk-eagle, registered by its room link and its status, as
        // the game shows it: the first creature here, so its letter is `A`.
        snapshot.state.apply(&Frame::Component {
            id: "room objs".to_owned(),
            body: Runs {
                runs: vec![Run {
                    text: "a tree hawk-eagle".to_owned(),
                    style: Style {
                        bold_depth: 1,
                        ..Style::default()
                    },
                    link: Some(cena_session::Link {
                        kind: cena_session::LinkKind::Exist {
                            id: "17".to_owned(),
                            noun: "hawk-eagle".to_owned(),
                        },
                        text: "a tree hawk-eagle".to_owned(),
                        coord: None,
                    }),
                    inner_link: None,
                }],
            },
        });
        snapshot.state.apply(&Frame::CreatureStatus {
            id: "17".to_owned(),
            attrs: vec![
                ("exist".to_owned(), "17".to_owned()),
                ("hostile".to_owned(), "1".to_owned()),
            ],
        });
        let style = cena_session::targetid::Style::default();
        let mark = snapshot
            .state
            .creatures()
            .get(17)
            .and_then(cena_session::CreatureInstance::mark)
            .expect("registered");
        assert_eq!(mark.unique, 'A');
        let parts = RoomParts {
            apart: true,
            ..RoomParts::default()
        };
        let mut harness = Harness::builder()
            .with_size((420.0, 260.0))
            .build_ui(move |ui| {
                let _ = room(ui, Some(&snapshot), parts, (true, Some(style)));
            });
        harness.run();
        let tagged = format!(
            "Creatures: a tree hawk-eagle ({})",
            cena_session::targetid::tag(mark, style, None)
        );
        assert!(harness.query_by_label(&tagged).is_some(), "{tagged}");
    }

    #[test]
    fn a_creature_in_the_room_is_a_link() {
        let clicked = |own: bool| {
            let snapshot = watering_hole();
            let parts = RoomParts {
                apart: true,
                ..RoomParts::default()
            };
            let mut harness = Harness::builder().with_size((420.0, 260.0)).build_ui_state(
                move |ui, clicked: &mut Option<crate::widget::Clicked>| {
                    if let Some(now) = room(ui, Some(&snapshot), parts, (own, None)) {
                        *clicked = Some(now);
                    }
                },
                None,
            );
            harness.run();
            let at = harness.get_by_label(CREATURES).rect().right_center() - egui::vec2(8.0, 0.0);
            let button = |pressed| egui::Event::PointerButton {
                pos: at,
                button: egui::PointerButton::Primary,
                pressed,
                modifiers: egui::Modifiers::NONE,
            };
            harness.hover_at(at);
            harness.step();
            harness.event(button(true));
            harness.step();
            harness.event(button(false));
            harness.run();
            harness.state().clone()
        };
        let Some(crate::widget::Clicked::Link(link, _)) = clicked(true) else {
            panic!("a link clicked");
        };
        assert_eq!(
            link,
            RunLink::Object {
                exist: "17".to_owned(),
                noun: "hawk-eagle".to_owned(),
                coord: None,
            }
        );
        assert_eq!(clicked(false), None, "another's room only shows");
    }

    /// `parts` of the room, rendered and compared with the committed image
    /// `name`.
    fn rendered(name: &str, parts: RoomParts) {
        let snapshot = watering_hole();
        let mut harness = Harness::builder()
            .with_size((420.0, 220.0))
            .wgpu()
            .build_ui(move |ui| {
                let _ = room(ui, Some(&snapshot), parts, (true, None));
            });
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
