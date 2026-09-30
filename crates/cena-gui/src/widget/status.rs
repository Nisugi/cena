//! Widgets of the character's condition (`plan/49` Stage B step 2): each
//! status indicator its own widget, as the author asked (*"individual
//! things"*, §1 row 1), and the four lists of effects the game keeps --
//! Active Spells, Buffs, Debuffs, Cooldowns -- each a widget, with Saga's
//! panel of them a preset of the four stacked as tabs.

use cena_session::GameState;
use cena_session::world::Pulse;
use egui::{Align2, FontId, Sense, Stroke, StrokeKind, Vec2};
use serde::{Deserialize, Serialize};

use crate::bar::{Amount, Bar, Says};
use crate::theme::{self, T, readable_on};
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

    /// The palette's token for it when on: the dangers warm, the postures
    /// and the rest cool.
    fn token(self) -> T {
        match self {
            Indicator::Stunned | Indicator::Webbed | Indicator::Bound | Indicator::Calmed => {
                T::Stunned
            }
            Indicator::Bleeding | Indicator::Dead | Indicator::Cutthroat => T::Bleeding,
            Indicator::Poisoned | Indicator::Diseased | Indicator::Thorned => T::Poisoned,
            Indicator::Hidden | Indicator::Invisible => T::Hidden,
            Indicator::Silenced | Indicator::Sleeping => T::Silenced,
            Indicator::Standing
            | Indicator::Kneeling
            | Indicator::Sitting
            | Indicator::Prone
            | Indicator::Joined => T::Posture,
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

    /// The palette's token its bars fill with.
    fn token(self) -> T {
        match self {
            Category::ActiveSpells => T::ActiveSpells,
            Category::Buffs => T::Buffs,
            Category::Debuffs => T::Debuffs,
            Category::Cooldowns => T::Cooldowns,
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
    let lit = theme::color(ui.ctx(), indicator.token());
    let painter = ui.painter();
    let rect = rect.shrink(1.0);
    let (text, color) = match known {
        Some(true) => {
            painter.rect_filled(rect, theme::corner(ui.ctx()), lit);
            (name.to_owned(), readable_on(lit))
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
                .fill(theme::color(ui.ctx(), category.token()))
                .size([ui.available_width(), 16.0])
                .says(Says {
                    label: true,
                    numbers: false,
                    percent: false,
                    words: false,
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

/// When the next pulse comes, as a bar that fills toward it, drawn as
/// `look` says.
pub(super) fn pulse(
    ui: &mut egui::Ui,
    state: Option<&GameState>,
    name: &str,
    look: Option<&crate::bar::Look>,
) {
    let now = state.and_then(GameState::game_time_now);
    if look.is_some_and(|look| look.clock) {
        let pulse = state.and_then(|state| state.world.pulse.as_ref());
        let said = match pulse
            .zip(now)
            .and_then(|(pulse, now)| pulse_clock(pulse, now))
        {
            Some(left) => format!("{name} {left}s"),
            None => format!("{name} ?"),
        };
        let pulse = theme::color(ui.ctx(), T::Pulse);
        super::draw::line(ui, egui::RichText::new(said).color(pulse));
        return;
    }
    let (label, percent) = pulse_said(
        state.and_then(|state| state.world.pulse.as_ref()),
        now,
        name,
    );
    let amount = percent.map(|percent| Amount {
        percent,
        current: None,
        max: None,
    });
    let pulse = theme::color(ui.ctx(), T::Pulse);
    let drawn = super::draw::as_looks(ui, Bar::new(&label, amount).fill(pulse), look);
    ui.add(drawn.fitted(ui));
}

/// What the pulse bar says at server second `now`, and how full it is:
/// *"Next pulse in 12-41s"*, *"Next mana pulse in ..."* when it will be one,
/// *"Pulse due"* once it may come any second, and `name` asking before the
/// first pulse or the game's clock.
pub(super) fn pulse_said(
    pulse: Option<&Pulse>,
    now: Option<u32>,
    name: &str,
) -> (String, Option<u32>) {
    let due = pulse
        .zip(now)
        .and_then(|(pulse, now)| pulse.due(now).map(|due| (pulse, due)));
    match due {
        None => (format!("{name} ?"), None),
        Some((_, (0, _))) => ("Pulse due".to_owned(), Some(100)),
        Some((pulse, (least, most))) => {
            let kind = if pulse.mana { "mana pulse" } else { "pulse" };
            let passed = pulse.min.saturating_sub(least);
            (
                format!("Next {kind} in {least}-{most}s"),
                Some(passed * 100 / pulse.min.max(1)),
            )
        }
    }
}

/// The pulse as a clock at server second `now`: seconds to its earliest,
/// then below zero, how late it is, until it comes and the clock starts
/// again (the author, 2026-09-30: *"it would say 20 and be counting down
/// then after 20 seconds it would be 0, -1, -2, -3, until the pulse"*).
pub(super) fn pulse_clock(pulse: &Pulse, now: u32) -> Option<i64> {
    let at = pulse.at?;
    Some(i64::from(pulse.min) - i64::from(now.saturating_sub(at)))
}

/// The world events under way, each with where and how long it has left.
pub(super) fn world_events(ui: &mut egui::Ui, state: Option<&GameState>) {
    let Some(state) = state else {
        ui.weak("World events unknown");
        return;
    };
    let now = state.game_time_now();
    let events: Vec<(Option<&str>, &str, Option<u32>)> = state
        .world
        .events(now)
        .map(|event| {
            (
                event.realm.as_deref(),
                event.text.as_str(),
                now.zip(event.expires_at)
                    .map(|(now, at)| at.saturating_sub(now)),
            )
        })
        .collect();
    events_listed(ui, &events);
}

/// `events` -- where, what, and seconds left when the game gave an expiry
/// -- a line each, or that there are none.
pub(super) fn events_listed(ui: &mut egui::Ui, events: &[(Option<&str>, &str, Option<u32>)]) {
    if events.is_empty() {
        ui.weak("No world events.");
    }
    for (realm, text, left) in events {
        let mut line = match realm {
            Some(realm) => format!("{realm}: {text}"),
            None => (*text).to_owned(),
        };
        if let Some(left) = left {
            line = format!("{line} ({} left)", clock(*left));
        }
        ui.label(line);
    }
}
