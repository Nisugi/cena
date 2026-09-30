//! Widgets of the room as a picture (`plan/49` Stage B step 3): a compass
//! of its ways out, and who is fighting in it -- friends and foes with their
//! statuses and health, as Saga's Combat panel shows them.

use cena_session::{CreatureInstance, GameState};
use egui::{Align2, Color32, FontId, Rect, Sense, Stroke, StrokeKind, Vec2, pos2, vec2};

use crate::bar::{Amount, Bar, Says};
use crate::theme::{self, T, readable_on};

/// A direction on the compass: its word, what the rose calls it, and where
/// it sits, in cells from the top left of a four-by-three grid.
const DIRECTIONS: [(&str, &str, (u8, u8)); 11] = [
    ("northwest", "NW", (0, 0)),
    ("north", "N", (1, 0)),
    ("northeast", "NE", (2, 0)),
    ("west", "W", (0, 1)),
    ("out", "OUT", (1, 1)),
    ("east", "E", (2, 1)),
    ("southwest", "SW", (0, 2)),
    ("south", "S", (1, 2)),
    ("southeast", "SE", (2, 2)),
    ("up", "UP", (3, 0)),
    ("down", "DN", (3, 2)),
];

/// The room's ways out as a compass rose, each lit when the room has it.
/// A click on a lit one goes that way, when `goes` -- never for a compass
/// following another character. The direction clicked, if one was.
pub(super) fn compass(ui: &mut egui::Ui, state: Option<&GameState>, goes: bool) -> Option<String> {
    let exits = state.and_then(|state| state.room.exits.as_ref());
    let size = ui.available_size();
    let cell = (size.x / 4.0).min(size.y / 3.0).max(12.0);
    let (area, response) = ui.allocate_exact_size(vec2(cell * 4.0, cell * 3.0), Sense::hover());
    if exits.is_none() {
        response.widget_info(|| {
            egui::WidgetInfo::labeled(egui::WidgetType::Label, true, "Exits unknown")
        });
    }
    let mut went = None;
    for (word, short, (column, row)) in DIRECTIONS {
        let rect = Rect::from_min_size(
            pos2(
                area.min.x + f32::from(column) * cell,
                area.min.y + f32::from(row) * cell,
            ),
            Vec2::splat(cell),
        )
        .shrink(2.0);
        let open = exits.is_some_and(|exits| exits.iter().any(|exit| exit == word));
        let sense = if open && goes {
            Sense::click()
        } else {
            Sense::hover()
        };
        let direction = ui.interact(rect, response.id.with(word), sense);
        direction.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, open, word));
        let painter = ui.painter();
        let text = if open {
            let accent = theme::color(ui.ctx(), T::Accent);
            painter.rect_filled(rect, theme::corner(ui.ctx()), accent);
            readable_on(accent)
        } else {
            painter.rect_stroke(
                rect,
                4.0,
                Stroke::new(1.0, ui.visuals().weak_text_color()),
                StrokeKind::Inside,
            );
            ui.visuals().weak_text_color()
        };
        painter.text(
            rect.center(),
            Align2::CENTER_CENTER,
            short,
            FontId::proportional((cell * 0.32).clamp(8.0, 14.0)),
            text,
        );
        if direction.clicked() {
            went = Some(word.to_owned());
        }
    }
    went
}

/// One creature as the combat list shows it: read off the model's creature,
/// so the list is drawn from what it says and nothing else.
#[derive(Clone, Debug, PartialEq)]
pub(super) struct Fighter {
    /// Its name.
    pub(super) name: String,
    /// Whether it is on the character's side.
    pub(super) friend: bool,
    /// Whether it is dead.
    pub(super) dead: bool,
    /// Its statuses, in words.
    pub(super) statuses: Vec<String>,
    /// Its health, 0-100, when the server states it.
    pub(super) health: Option<u32>,
}

impl Fighter {
    /// `creature` as the list shows it at `now`. A creature the game has
    /// not said is friendly is a foe.
    fn of(creature: &CreatureInstance, now: Option<u32>) -> Self {
        let health = creature.hp_is_stated().then(|| {
            // Rounded and clamped to 0-100 first, so the cast cannot truncate.
            #[allow(
                clippy::cast_possible_truncation,
                clippy::cast_sign_loss,
                reason = "rounded and clamped to 0-100 in the same expression"
            )]
            let percent = creature.hp_percent().clamp(0.0, 100.0).round() as u32;
            percent
        });
        Self {
            name: creature.name.clone(),
            friend: creature.hostile() == Some(false),
            dead: creature.dead(),
            statuses: creature
                .statuses(now)
                .into_iter()
                .map(|status| status.as_str().replace('_', " "))
                .collect(),
            health,
        }
    }
}

/// Who is fighting here, as Saga's Combat panel shows it: the character
/// and its friends, then its foes.
pub(super) fn combat(ui: &mut egui::Ui, state: Option<&GameState>) {
    let Some(state) = state else {
        ui.weak("Combat unknown");
        return;
    };
    let now = state.game_time_now();
    let fighters: Vec<Fighter> = state
        .creatures()
        .in_room()
        .map(|creature| Fighter::of(creature, now))
        .collect();
    fighting(ui, state.character.stance.as_deref(), &fighters);
}

/// The combat list: *FRIENDLY*, the character with its `stance` and its
/// friends; *FOES*, the rest, or that there are none.
pub(super) fn fighting(ui: &mut egui::Ui, stance: Option<&str>, fighters: &[Fighter]) {
    ui.label(egui::RichText::new("FRIENDLY").small().weak());
    let player = theme::color(ui.ctx(), T::Player);
    ui.horizontal(|ui| {
        ui.colored_label(player, "You");
        if let Some(stance) = stance {
            ui.weak(stance);
        }
    });
    for friend in fighters.iter().filter(|fighter| fighter.friend) {
        fighter(ui, friend, player);
    }
    ui.label(egui::RichText::new("FOES").small().weak());
    let mut foes = fighters.iter().filter(|fighter| !fighter.friend).peekable();
    if foes.peek().is_none() {
        ui.weak("No foes.");
    }
    for foe in foes {
        fighter(ui, foe, theme::color(ui.ctx(), T::Creature));
    }
}

/// One fighter: its name, whether it is dead, its statuses, and its health
/// as a thin bar when stated.
fn fighter(ui: &mut egui::Ui, fighter: &Fighter, color: Color32) {
    ui.horizontal_wrapped(|ui| {
        ui.colored_label(color, &fighter.name);
        if fighter.dead {
            ui.weak("dead");
        }
        if !fighter.statuses.is_empty() {
            ui.weak(fighter.statuses.join(", "));
        }
    });
    if let Some(percent) = fighter.health {
        let bar = Bar::new(
            &fighter.name,
            Some(Amount {
                percent,
                current: None,
                max: None,
            }),
        )
        .fill(theme::color(ui.ctx(), T::Health))
        .size([ui.available_width(), 6.0])
        .says(Says {
            label: false,
            numbers: false,
            percent: false,
            words: false,
        });
        ui.add(bar);
    }
}
