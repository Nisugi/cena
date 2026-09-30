//! Widgets that are lists of what the character has (`plan/49` Stage B step
//! 5): the spellbook, the reserve (Saga's R1-R3), and the containers the
//! game has shown. Each is drawn from a plain list read off the model, so
//! the list is what it says and nothing else.

use cena_session::GameState;
use egui::RichText;

use crate::theme::{self, T};

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

/// One container as the widget lists it: its title, the id an object
/// carried onto it goes into, and what it holds, each its words and id.
pub(super) type Listed<'a> = (String, String, Vec<(&'a str, &'a str)>);

/// Every container the game has shown, each with what it holds; an object
/// carried and let go on one goes into it, `_drag #<item> #<container>`,
/// and on one of its items, into that. On the window's `own` character's,
/// each item is carried from it with the drag key held (`carry.rs`).
pub(super) fn containers(
    ui: &mut egui::Ui,
    state: Option<&GameState>,
    own: bool,
) -> Option<String> {
    let Some(state) = state else {
        ui.weak("Containers unknown");
        return None;
    };
    let containers: Vec<Listed<'_>> = state
        .inventory
        .containers()
        .map(|(id, container)| {
            (
                container.title.clone().unwrap_or_else(|| id.to_owned()),
                // The container object's id, which the window's is not
                // always: `stow` is a name (`Container::target`).
                container.target.clone().unwrap_or_else(|| id.to_owned()),
                container
                    .items
                    .iter()
                    .map(|item| (item.text.as_str(), item.id.as_str()))
                    .collect(),
            )
        })
        .collect();
    containers_listed(ui, &containers, own)
}

/// `containers`, each a heading that opens to what it holds; what is
/// carried and let go on one, or on an item in it, put into it; on the
/// `own` character's, each item carried from it.
pub(super) fn containers_listed(
    ui: &mut egui::Ui,
    containers: &[Listed<'_>],
    own: bool,
) -> Option<String> {
    if containers.is_empty() {
        ui.weak("No container seen yet");
        return None;
    }
    let mut put = None;
    for (title, target, items) in containers {
        let shown = egui::CollapsingHeader::new(format!("{title} ({})", items.len()))
            .id_salt(("container", title))
            .default_open(false)
            .show(ui, |ui| {
                if items.is_empty() {
                    ui.weak("empty");
                }
                for (words, item) in items {
                    let object = theme::color(ui.ctx(), T::Object);
                    let label = egui::Label::new(egui::RichText::new(*words).color(object))
                        .selectable(false);
                    if !own {
                        ui.add(label);
                        continue;
                    }
                    let response = ui.add(label.sense(crate::carry::sense(ui)));
                    crate::carry::source(
                        &response,
                        crate::carry::Carried {
                            exist: (*item).to_owned(),
                            name: (*words).to_owned(),
                        },
                    );
                    if put.is_none() {
                        put = crate::carry::dropped(&response, &format!("#{item}"), Some(item));
                    }
                }
            });
        let onto = format!("#{target}");
        put = put
            .or_else(|| crate::carry::dropped(&shown.header_response, &onto, Some(target)))
            .or_else(|| {
                shown
                    .body_response
                    .as_ref()
                    .and_then(|body| crate::carry::dropped(body, &onto, Some(target)))
            });
    }
    put
}
