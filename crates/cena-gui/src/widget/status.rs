//! Widgets of the character's condition (`plan/49` Stage B step 2): each
//! status indicator its own widget, as the author asked (*"individual
//! things"*, §1 row 1), and the four lists of effects the game keeps --
//! Active Spells, Buffs, Debuffs, Cooldowns -- each a widget, with Saga's
//! panel of them a preset of the four stacked as tabs.

use cena_session::GameState;
use egui::{Align2, Color32, FontId, Sense, Stroke, StrokeKind, Vec2};
use serde::{Deserialize, Serialize};

use crate::bar::{Amount, Bar, Says};
use crate::widget::LINE;

/// One status indicator, as the game names it
/// (`crates/cena-model/src/status.rs`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum Indicator {
    /// Standing.
    Standing,
    /// Kneeling.
    Kneeling,
    /// Sitting.
    Sitting,
    /// Lying down.
    Prone,
    /// Stunned.
    Stunned,
    /// Bleeding.
    Bleeding,
    /// Hidden.
    Hidden,
    /// Invisible.
    Invisible,
    /// Webbed.
    Webbed,
    /// In a group.
    Joined,
    /// Dead.
    Dead,
    /// Bound.
    Bound,
    /// Calmed.
    Calmed,
    /// Throat cut.
    Cutthroat,
    /// Silenced.
    Silenced,
    /// Asleep.
    Sleeping,
    /// Thorned.
    Thorned,
    /// Poisoned.
    Poisoned,
    /// Diseased.
    Diseased,
}

impl Indicator {
    /// Every indicator, in the order the model lists them.
    pub(crate) const ALL: [Indicator; 19] = [
        Indicator::Standing,
        Indicator::Kneeling,
        Indicator::Sitting,
        Indicator::Prone,
        Indicator::Stunned,
        Indicator::Bleeding,
        Indicator::Hidden,
        Indicator::Invisible,
        Indicator::Webbed,
        Indicator::Joined,
        Indicator::Dead,
        Indicator::Bound,
        Indicator::Calmed,
        Indicator::Cutthroat,
        Indicator::Silenced,
        Indicator::Sleeping,
        Indicator::Thorned,
        Indicator::Poisoned,
        Indicator::Diseased,
    ];

    /// The model's id for it (`StatusInfo`), which is its name there.
    fn id(self) -> &'static str {
        match self {
            Indicator::Standing => "standing",
            Indicator::Kneeling => "kneeling",
            Indicator::Sitting => "sitting",
            Indicator::Prone => "prone",
            Indicator::Stunned => "stunned",
            Indicator::Bleeding => "bleeding",
            Indicator::Hidden => "hidden",
            Indicator::Invisible => "invisible",
            Indicator::Webbed => "webbed",
            Indicator::Joined => "joined",
            Indicator::Dead => "dead",
            Indicator::Bound => "bound",
            Indicator::Calmed => "calmed",
            Indicator::Cutthroat => "cutthroat",
            Indicator::Silenced => "silenced",
            Indicator::Sleeping => "sleeping",
            Indicator::Thorned => "thorned",
            Indicator::Poisoned => "poisoned",
            Indicator::Diseased => "diseased",
        }
    }

    /// What a player calls it.
    pub(crate) fn name(self) -> &'static str {
        match self {
            Indicator::Standing => "Standing",
            Indicator::Kneeling => "Kneeling",
            Indicator::Sitting => "Sitting",
            Indicator::Prone => "Prone",
            Indicator::Stunned => "Stunned",
            Indicator::Bleeding => "Bleeding",
            Indicator::Hidden => "Hidden",
            Indicator::Invisible => "Invisible",
            Indicator::Webbed => "Webbed",
            Indicator::Joined => "Grouped",
            Indicator::Dead => "Dead",
            Indicator::Bound => "Bound",
            Indicator::Calmed => "Calmed",
            Indicator::Cutthroat => "Throat cut",
            Indicator::Silenced => "Silenced",
            Indicator::Sleeping => "Asleep",
            Indicator::Thorned => "Thorned",
            Indicator::Poisoned => "Poisoned",
            Indicator::Diseased => "Diseased",
        }
    }

    /// Its colour when on: the dangers warm, the postures and the rest cool.
    fn color(self) -> Color32 {
        match self {
            Indicator::Stunned | Indicator::Webbed | Indicator::Bound | Indicator::Calmed => {
                Color32::from_rgb(0xd8, 0xb4, 0x3a)
            }
            Indicator::Bleeding | Indicator::Dead | Indicator::Cutthroat => {
                Color32::from_rgb(0xcd, 0x4d, 0x4d)
            }
            Indicator::Poisoned | Indicator::Diseased | Indicator::Thorned => {
                Color32::from_rgb(0x6d, 0xa8, 0x3c)
            }
            Indicator::Hidden | Indicator::Invisible => Color32::from_rgb(0x7a, 0x86, 0xa8),
            Indicator::Silenced | Indicator::Sleeping => Color32::from_rgb(0x8e, 0x6b, 0xc9),
            Indicator::Standing
            | Indicator::Kneeling
            | Indicator::Sitting
            | Indicator::Prone
            | Indicator::Joined => Color32::from_rgb(0x4f, 0xa3, 0xa5),
        }
    }
}

/// One of the game's lists of effects, as it names its dialogs
/// (`crates/cena-model/src/effects.rs`, `EFFECT_DIALOGS`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum Category {
    /// Active Spells.
    ActiveSpells,
    /// Buffs.
    Buffs,
    /// Debuffs.
    Debuffs,
    /// Cooldowns.
    Cooldowns,
}

impl Category {
    /// Every list, in the order the game's dialogs name them.
    pub(crate) const ALL: [Category; 4] = [
        Category::ActiveSpells,
        Category::Buffs,
        Category::Debuffs,
        Category::Cooldowns,
    ];

    /// Its name, which is the game's dialog's too.
    pub(crate) fn name(self) -> &'static str {
        match self {
            Category::ActiveSpells => "Active Spells",
            Category::Buffs => "Buffs",
            Category::Debuffs => "Debuffs",
            Category::Cooldowns => "Cooldowns",
        }
    }

    /// Its bars' fill.
    fn color(self) -> Color32 {
        match self {
            Category::ActiveSpells => Color32::from_rgb(0x47, 0x84, 0xd9),
            Category::Buffs => Color32::from_rgb(0x55, 0xb8, 0x6c),
            Category::Debuffs => Color32::from_rgb(0xcd, 0x4d, 0x4d),
            Category::Cooldowns => Color32::from_rgb(0x7a, 0x86, 0xa8),
        }
    }
}

/// An indicator, filling what it is given: lit in its colour when on, dim
/// when off, and asking when the game has not said. A screen reader hears
/// which: *"Stunned: yes"*.
pub(super) fn indicator(
    ui: &mut egui::Ui,
    indicator: Indicator,
    state: Option<&GameState>,
    name: &str,
) {
    let known = state.and_then(|state| {
        state
            .status
            .is_known(indicator.id())
            .then(|| state.status.get(indicator.id()))
    });
    let size = Vec2::new(
        ui.available_width(),
        LINE.min(ui.available_height().max(LINE)),
    );
    let (rect, response) = ui.allocate_exact_size(size, Sense::hover());
    let said = match known {
        Some(true) => "yes",
        Some(false) => "no",
        None => "unknown",
    };
    response.widget_info(|| {
        egui::WidgetInfo::labeled(egui::WidgetType::Label, true, format!("{name}: {said}"))
    });
    let painter = ui.painter();
    let rect = rect.shrink(1.0);
    let (text, color) = match known {
        Some(true) => {
            painter.rect_filled(rect, 4.0, indicator.color());
            (name.to_owned(), Color32::BLACK)
        }
        Some(false) => {
            painter.rect_stroke(
                rect,
                4.0,
                Stroke::new(1.0, ui.visuals().weak_text_color()),
                StrokeKind::Inside,
            );
            (name.to_owned(), ui.visuals().weak_text_color())
        }
        None => (format!("{name} ?"), ui.visuals().weak_text_color()),
    };
    painter.text(
        rect.center(),
        Align2::CENTER_CENTER,
        text,
        FontId::proportional(12.0),
        color,
    );
}

/// One list of effects: a bar each, its time left beside its name, full as
/// the game last said; or that there are none.
pub(super) fn effects(ui: &mut egui::Ui, category: Category, state: Option<&GameState>) {
    let Some(state) = state else {
        ui.weak(format!("{} unknown", category.name()));
        return;
    };
    let now = state.game_time_now();
    let mut any = false;
    for (id, effect) in state.effects.in_category(category.name()) {
        any = true;
        let left = now.and_then(|now| state.effects.remaining(id, now));
        let label = match left {
            Some(seconds) => format!("{} {}", effect.text, clock(seconds)),
            None => effect.text.clone(),
        };
        let amount = Amount {
            percent: effect.percent,
            current: None,
            max: None,
        };
        ui.add(
            Bar::new(&label, Some(amount))
                .fill(category.color())
                .size([ui.available_width(), 16.0])
                .says(Says {
                    label: true,
                    numbers: false,
                    percent: false,
                }),
        );
    }
    if !any {
        ui.weak("None.");
    }
}

/// Seconds as a clock: `1:59`, or `1:02:03` past the hour.
pub(super) fn clock(seconds: u32) -> String {
    let (hours, minutes, seconds) = (seconds / 3600, seconds / 60 % 60, seconds % 60);
    if hours > 0 {
        format!("{hours}:{minutes:02}:{seconds:02}")
    } else {
        format!("{minutes}:{seconds:02}")
    }
}
