//! Presets (`plan/49` Stage A step 7): custom windows already put together,
//! as the author put it -- Hydra's own, such as a row of vitals, and any a
//! player saved. Not special code: a preset is a custom window kept aside.
//!
//! One library every character adds from, in `presets.json` beside the
//! layouts; each layout stays its character's own. **Adding a preset places
//! the character's own copy** (the author, 2026-09-27: *"yep own copy"*):
//! its widgets get new ids, and nothing done to the copy reaches the preset,
//! nor anything saved over the preset reaches a copy already placed.

use std::path::{Path, PathBuf};

use egui::{Rect, Vec2};
use serde::{Deserialize, Serialize};

use super::{CHROME, Custom, Holds, Layout, Placed};
use crate::widget::{Category, Indicator, Widget};

/// The version of the library file this build writes and reads.
const VERSION: u32 = 1;

/// The library's file, in the layouts folder.
const FILE: &str = "presets.json";

/// A custom window kept aside to be placed again.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub(crate) struct Preset {
    /// What the list calls it.
    pub(crate) name: String,
    /// The window, as it was put together.
    custom: Custom,
}

impl Preset {
    /// Custom window `custom` kept as a preset named `name`.
    pub(crate) fn of(name: &str, custom: &Custom) -> Self {
        Self {
            name: name.trim().to_owned(),
            custom: custom.clone(),
        }
    }

    /// The presets Hydra ships: the vitals stacked and in a row, what the
    /// hands hold with the clocks, the room in its parts, and the
    /// experience, as Saga's panel has it (`plan/49` §3).
    pub(crate) fn hydras() -> Vec<Preset> {
        let bars = [
            Widget::Health,
            Widget::Mana,
            Widget::Stamina,
            Widget::Spirit,
        ];
        vec![
            shipped(
                "Vitals",
                &bars.clone().map(|bar| vec![bar]),
                Vec2::new(300.0, 86.0),
            ),
            shipped("Vitals row", &[bars.to_vec()], Vec2::new(800.0, 20.0)),
            shipped(
                "Loadout",
                &[
                    vec![Widget::RightHand],
                    vec![Widget::LeftHand],
                    vec![Widget::Roundtime, Widget::CastTime],
                ],
                Vec2::new(300.0, 66.0),
            ),
            shipped(
                "Room",
                &[
                    vec![Widget::RoomTitle],
                    vec![Widget::RoomDescription],
                    vec![Widget::Creatures],
                    vec![Widget::Objects],
                    vec![Widget::Players],
                    vec![Widget::Exits],
                ],
                Vec2::new(320.0, 240.0),
            ),
            shipped(
                "Experience",
                &[
                    vec![Widget::Level],
                    vec![Widget::Mind],
                    vec![Widget::NextLevel],
                    vec![Widget::TrainingPoints],
                    vec![Widget::ExperienceTotals],
                ],
                Vec2::new(300.0, 160.0),
            ),
            shipped(
                "Indicators",
                &Indicator::ALL
                    .chunks(3)
                    .map(|row| {
                        row.iter()
                            .map(|indicator| Widget::Indicator(*indicator))
                            .collect()
                    })
                    .collect::<Vec<_>>(),
                Vec2::new(300.0, 140.0),
            ),
            Preset {
                name: "Streams".to_owned(),
                custom: Custom::stack(
                    "Streams",
                    ["thoughts", "speech", "logons", "death", "announcements"]
                        .map(|id| Placed {
                            id: 0,
                            widget: Widget::Stream(id.to_owned()),
                        })
                        .to_vec(),
                    0,
                    Vec2::new(320.0, 200.0),
                ),
            },
            Preset {
                name: "Effects".to_owned(),
                custom: Custom::stack(
                    "Effects",
                    Category::ALL
                        .map(|category| Placed {
                            id: 0,
                            widget: Widget::Effects(category),
                        })
                        .to_vec(),
                    0,
                    Vec2::new(300.0, 160.0),
                ),
            },
        ]
    }
}

/// One of Hydra's presets, its widgets in `rows`, its inside `inside`
/// across. The ids are placeholders: placing a preset gives new ones.
fn shipped(name: &str, rows: &[Vec<Widget>], inside: Vec2) -> Preset {
    let rows = rows
        .iter()
        .map(|row| {
            row.iter()
                .map(|widget| Placed {
                    id: 0,
                    widget: widget.clone(),
                })
                .collect()
        })
        .collect();
    Preset {
        name: name.to_owned(),
        custom: Custom::rows(name, rows, inside),
    }
}

impl Layout {
    /// A copy of `preset` in a custom window of its own, its widgets given
    /// ids of their own, placed where a widget added from the list goes; each
    /// following `whose`, when another character, but a story. Its id.
    pub(crate) fn add_preset(&mut self, preset: &Preset, whose: Option<&str>) -> u32 {
        let mut custom = preset.custom.clone();
        for tab in custom
            .cells
            .iter_mut()
            .flat_map(|cell| cell.tabs.iter_mut())
        {
            tab.id = self.next;
            self.next += 1;
            if let Some(whose) = whose.filter(|_| !tab.widget.is_story()) {
                self.follows.insert(tab.id, whose.to_owned());
            }
        }
        let size = custom.inside() + CHROME;
        let corner = self.next_corner();
        self.add(Rect::from_min_size(corner, size), Holds::Custom(custom))
    }
}

/// The presets a player saved, shared by every character.
#[derive(Debug, Default)]
pub(crate) struct Library {
    /// Where the file is kept; `None`, and nothing is.
    dir: Option<PathBuf>,
    /// The presets, in the order they were first saved.
    presets: Vec<Preset>,
    /// Why the file could not be written, until it can.
    pub(crate) unsaved: Option<String>,
}

/// The library file's shape.
#[derive(Serialize, Deserialize)]
struct File {
    version: u32,
    presets: Vec<Preset>,
}

impl Library {
    /// The library kept in `dir`, the layouts folder: empty when there is
    /// none yet, or it cannot be read.
    pub(crate) fn load(dir: Option<PathBuf>) -> Self {
        let presets = dir
            .as_deref()
            .and_then(|dir| std::fs::read_to_string(dir.join(FILE)).ok())
            .and_then(|text| serde_json::from_str::<File>(&text).ok())
            .filter(|file| file.version == VERSION)
            .map(|file| file.presets)
            .unwrap_or_default();
        Self {
            dir,
            presets,
            unsaved: None,
        }
    }

    /// The presets saved.
    pub(crate) fn presets(&self) -> &[Preset] {
        &self.presets
    }

    /// Keep `preset`, over one of the same name.
    pub(crate) fn keep(&mut self, preset: Preset) {
        match self
            .presets
            .iter_mut()
            .find(|kept| kept.name.eq_ignore_ascii_case(&preset.name))
        {
            Some(kept) => *kept = preset,
            None => self.presets.push(preset),
        }
        self.save();
    }

    /// Forget the preset named `name`.
    pub(crate) fn forget(&mut self, name: &str) {
        self.presets.retain(|kept| kept.name != name);
        self.save();
    }

    fn save(&mut self) {
        let Some(dir) = &self.dir else {
            return;
        };
        self.unsaved = write(dir, &self.presets).err().map(|why| why.to_string());
    }
}

fn write(dir: &Path, presets: &[Preset]) -> std::io::Result<()> {
    std::fs::create_dir_all(dir)?;
    let file = File {
        version: VERSION,
        presets: presets.to_vec(),
    };
    let text = serde_json::to_string_pretty(&file).map_err(std::io::Error::other)?;
    std::fs::write(dir.join(FILE), text)
}

#[cfg(test)]
mod tests;
