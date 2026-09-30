//! Widgets of what the character is and has (`plan/49` Stage B step 1):
//! stance, encumbrance, mind and the next level as bars; level, training
//! points, the experience numbers, the prepared spell, society, resources
//! and objectives as lines. Each reads a fact the model already holds
//! (`plan/49` §3), and says so when the game has not told it yet.

use std::fmt::Write as _;

use cena_session::GameState;

use crate::bar::{self, Amount, Bar};
use crate::theme::{self, T};

/// A bar the game words, as a vital is drawn (the author, 2026-09-29:
/// *"It's basically a progress bar right? So it should have all the same
/// things"*): `label`, the game's `words` for its state, and how full,
/// each said as `look` asks, filling what it is given as `look` fills.
fn gauge(
    ui: &mut egui::Ui,
    (label, words): (&str, Option<&str>),
    percent: Option<u32>,
    token: T,
    look: Option<&bar::Look>,
) {
    let amount = percent.map(|percent| Amount {
        percent,
        current: None,
        max: None,
    });
    let color = theme::color(ui.ctx(), token);
    let drawn = super::draw::as_looks(ui, Bar::new(label, amount).fill(color).state(words), look);
    ui.add(drawn.fitted(ui));
}

/// The game's words for a state without the percent it adds to some:
/// stance's `defensive (100%)` is `defensive`, the percent the bar's own.
fn worded(text: &str) -> &str {
    match text.rsplit_once(" (") {
        Some((words, rest)) if rest.ends_with("%)") => words,
        _ => text,
    }
}

/// Stance as a bar: what the game calls it, and how much of it guards.
pub(super) fn stance(
    ui: &mut egui::Ui,
    state: Option<&GameState>,
    named: &dyn Fn(&str) -> String,
    look: Option<&bar::Look>,
) {
    let character = state.map(|state| &state.character);
    let text = character.and_then(|c| c.stance.as_deref()).map(worded);
    let percent = character.and_then(|c| c.stance_percent);
    gauge(ui, (&named("Stance"), text), percent, T::Stance, look);
}

/// Encumbrance as a bar.
pub(super) fn encumbrance(
    ui: &mut egui::Ui,
    state: Option<&GameState>,
    named: &dyn Fn(&str) -> String,
    look: Option<&bar::Look>,
) {
    let character = state.map(|state| &state.character);
    let text = character.and_then(|c| c.encumbrance.as_deref()).map(worded);
    let percent = character.and_then(|c| c.encumbrance_percent);
    gauge(
        ui,
        (&named("Encumbrance"), text),
        percent,
        T::Encumbrance,
        look,
    );
}

/// The mind as a bar: how full of experience, in the game's words.
pub(super) fn mind(
    ui: &mut egui::Ui,
    state: Option<&GameState>,
    named: &dyn Fn(&str) -> String,
    look: Option<&bar::Look>,
) {
    let experience = state.map(|state| &state.character.experience);
    let text = experience.and_then(|e| e.mind_state.as_deref()).map(worded);
    let percent = experience.and_then(|e| e.mind_percent);
    gauge(ui, (&named("Mind"), text), percent, T::Mind, look);
}

/// How near the next level, as a bar.
pub(super) fn next_level(
    ui: &mut egui::Ui,
    state: Option<&GameState>,
    named: &dyn Fn(&str) -> String,
    look: Option<&bar::Look>,
) {
    let experience = state.map(|state| &state.character.experience);
    let text = experience.and_then(|e| e.next_level.as_deref()).map(worded);
    let percent = experience.and_then(|e| e.next_level_percent);
    gauge(ui, (&named("Next level"), text), percent, T::Level, look);
}

/// The level, as the game words it.
pub(super) fn level(state: Option<&GameState>) -> String {
    state
        .and_then(|state| state.character.experience.level.clone())
        .unwrap_or_else(|| "Level ?".to_owned())
}

/// Physical and mental training points.
pub(super) fn training(state: Option<&GameState>) -> String {
    let experience = state.map(|state| &state.character.experience);
    let points = |value: Option<u32>| value.map_or_else(|| "?".to_owned(), |n| grouped(n.into()));
    format!(
        "PTPs {} · MTPs {}",
        points(experience.and_then(|e| e.physical_training)),
        points(experience.and_then(|e| e.mental_training)),
    )
}

/// The experience numbers the game has told, one a line.
pub(super) fn experience(ui: &mut egui::Ui, state: Option<&GameState>) {
    let Some(experience) = state.map(|state| &state.character.experience) else {
        ui.weak("Experience unknown");
        return;
    };
    let mut lines = Vec::new();
    if let Some(total) = experience.total_experience {
        lines.push(format!("Total {}", grouped(total)));
    }
    if let (Some(field), Some(max)) = (experience.field_experience, experience.field_experience_max)
    {
        lines.push(format!(
            "Field {}/{}",
            grouped(field.into()),
            grouped(max.into())
        ));
    }
    if let Some(ascension) = experience.ascension_experience {
        lines.push(format!("Ascension {}", grouped(ascension)));
    }
    if let Some(long_term) = experience.long_term_experience {
        lines.push(format!("Long-term {}", grouped(long_term.into())));
    }
    if let Some(deeds) = experience.deeds {
        lines.push(format!("Deeds {deeds}"));
    }
    if lines.is_empty() {
        ui.weak("Experience unknown");
    }
    for line in lines {
        ui.label(line);
    }
}

/// The society and its rank.
pub(super) fn society(state: Option<&GameState>) -> String {
    let standing = state.map(|state| &state.character.standing);
    match standing.and_then(|standing| standing.society.as_ref()) {
        None => "Society unknown".to_owned(),
        Some(None) => "Society: none".to_owned(),
        Some(Some(society)) => match standing.and_then(|standing| standing.society_rank) {
            Some(rank) => format!("{society}, rank {rank}"),
            None => society.to_string(),
        },
    }
}

/// The resource the character's profession gathers, and its kin.
pub(super) fn resources(ui: &mut egui::Ui, state: Option<&GameState>) {
    let Some(standing) = state.map(|state| &state.character.standing) else {
        ui.weak("Resources unknown");
        return;
    };
    let mut lines = Vec::new();
    if let (Some(kind), Some(amounts)) = (standing.resource_type, standing.resources.as_ref()) {
        lines.push(format!(
            "{kind}: {}/50,000 this week, {}/200,000 in all",
            grouped(amounts.weekly.into()),
            grouped(amounts.total.into()),
        ));
    }
    if let Some(suffused) = standing.suffused {
        lines.push(format!("Suffused: {}", grouped(suffused.into())));
    }
    if let Some(charges) = standing.covert_arts_charges {
        lines.push(format!("Covert Arts charges: {charges}/200"));
    }
    if let Some(essence) = standing.shadow_essence {
        lines.push(format!("Shadow essence: {essence}/5"));
    }
    if lines.is_empty() {
        ui.weak("Resources unknown");
    }
    for line in lines {
        ui.label(line);
    }
}

/// Every objective the game has stated.
pub(super) fn objectives(ui: &mut egui::Ui, state: Option<&GameState>) {
    let Some(objectives) = state.map(|state| &state.objectives) else {
        ui.weak("Objectives unknown");
        return;
    };
    if objectives.is_empty() {
        ui.weak("No objectives");
    }
    for objective in objectives.all() {
        let name = objective.name.as_deref().unwrap_or(&objective.id);
        let mut line = format!("{name} ({})", objective.kind.to_lowercase());
        if let Some(location) = &objective.location {
            let _ = write!(line, " — {location}");
        }
        if let Some(state) = &objective.state {
            let _ = write!(line, " [{state}]");
        }
        ui.label(line);
    }
}

/// `n` with its thousands grouped by commas, as the game writes them.
pub(super) fn grouped(n: u64) -> String {
    let digits = n.to_string();
    let mut out = String::new();
    for (at, digit) in digits.chars().enumerate() {
        if at > 0 && (digits.len() - at).is_multiple_of(3) {
            out.push(',');
        }
        out.push(digit);
    }
    out
}
