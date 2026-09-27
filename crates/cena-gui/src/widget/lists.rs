//! Widgets that are lists of what the character has (`plan/49` Stage B step
//! 5): the spellbook, the reserve (Saga's R1-R3), and the containers the
//! game has shown. Each is drawn from a plain list read off the model, so
//! the list is what it says and nothing else.

use cena_session::GameState;
use egui::RichText;

use crate::text::OBJECT;

/// The spells the game lists for the character: number, name, circle.
pub(super) fn spellbook(ui: &mut egui::Ui, state: Option<&GameState>) {
    let Some(state) = state else {
        ui.weak("Spells unknown");
        return;
    };
    let spells: Vec<(u32, &str, Option<&str>)> = state
        .known_spells
        .iter()
        .map(|(number, spell)| (number, spell.name.as_str(), spell.circle.as_deref()))
        .collect();
    spells_listed(ui, &spells);
}

/// `spells` under their circles, as the game's Spells window lists them.
pub(super) fn spells_listed(ui: &mut egui::Ui, spells: &[(u32, &str, Option<&str>)]) {
    if spells.is_empty() {
        ui.weak("No spells listed");
        return;
    }
    let mut circle = None;
    for (number, name, this) in spells {
        if *this != circle {
            circle = *this;
            if let Some(circle) = circle {
                ui.label(RichText::new(circle.to_uppercase()).small().weak());
            }
        }
        ui.label(format!("{number} {name}"));
    }
}

/// What the character keeps in reserve, numbered as Saga's R1-R3.
pub(super) fn reserve(ui: &mut egui::Ui, state: Option<&GameState>) {
    let items: Option<Vec<&str>> = state.and_then(|state| {
        state
            .reserve
            .items()
            .map(|items| items.iter().map(|item| item.text.as_str()).collect())
    });
    reserved(ui, items.as_deref());
}

/// `items` numbered from R1; `None` until the game has listed the reserve.
pub(super) fn reserved(ui: &mut egui::Ui, items: Option<&[&str]>) {
    match items {
        None => {
            ui.weak("Reserve unknown");
        }
        Some([]) => {
            ui.weak("Nothing in reserve");
        }
        Some(items) => {
            for (at, item) in items.iter().enumerate() {
                ui.label(format!("R{}: {item}", at + 1));
            }
        }
    }
}

/// Every container the game has shown, each with what it holds.
pub(super) fn containers(ui: &mut egui::Ui, state: Option<&GameState>) {
    let Some(state) = state else {
        ui.weak("Containers unknown");
        return;
    };
    let containers: Vec<(String, Vec<&str>)> = state
        .inventory
        .containers()
        .map(|(id, container)| {
            (
                container.title.clone().unwrap_or_else(|| id.to_owned()),
                container
                    .items
                    .iter()
                    .map(|item| item.text.as_str())
                    .collect(),
            )
        })
        .collect();
    containers_listed(ui, &containers);
}

/// `containers`, each a heading that opens to what it holds.
pub(super) fn containers_listed(ui: &mut egui::Ui, containers: &[(String, Vec<&str>)]) {
    if containers.is_empty() {
        ui.weak("No container seen yet");
        return;
    }
    for (title, items) in containers {
        egui::CollapsingHeader::new(format!("{title} ({})", items.len()))
            .id_salt(("container", title))
            .default_open(false)
            .show(ui, |ui| {
                if items.is_empty() {
                    ui.weak("empty");
                }
                for item in items {
                    ui.colored_label(OBJECT, *item);
                }
            });
    }
}
