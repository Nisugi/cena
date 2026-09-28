//! A link clicked in a line of the game's (the author, 2026-09-28: *"links
//! using the preset highlights, clickable with their menus popping up"*),
//! acted on as `VellumFE` acts on one (`frontend/gui/app.rs`,
//! `resolve_link_dispatch`; `core/app_core/state/menus.rs`):
//!
//! - a command link sends its command, as if typed;
//! - an object link whose `coord=` names a command sends that command;
//! - any other object link asks the game for the object's menu,
//!   `_menu #<id> <n>`, sent without an echo, and the menu the game answers
//!   with `id=<n>` pops up where the click was, labelled by the model's
//!   dictionary (`cena_ui::object_menu`); an entry chosen sends its command.
//!   Only an answer that came after the request, on its connection: a
//!   reconnect closes the request, and a window reopened, its numbers
//!   begun again, takes no answer the story kept from before (the crate
//!   review of 2026-09-28, R13);
//! - a web address opens in the browser.

use cena_ui::{MenuGroup, RunLink, link_command, object_menu};
use egui::{Id, Order, Pos2};

use super::{Asked, Play, PlayView};
use crate::story::Story;

/// An object's menu, asked of the game, until it is chosen from or closed.
#[derive(Debug)]
pub(super) struct Asking {
    /// The request's number, which the game's answer carries as its `id`.
    number: u32,
    /// The object's id.
    exist: String,
    /// The object's noun, which a menu command's `@` becomes.
    noun: String,
    /// Where the click was, where the menu opens.
    at: Pos2,
    /// The connection it was asked on: another, and it is closed.
    generation: Option<cena_session::Generation>,
    /// The answers the story had heard when it was asked: only one after
    /// them is its.
    after: Option<u64>,
}

impl Play {
    /// Act on `link`, clicked at `at`, the character as `snapshot` has it,
    /// its story `story`.
    pub(super) fn clicked(
        &mut self,
        context: &egui::Context,
        (link, at): (RunLink, Pos2),
        (snapshot, story): (Option<&cena_session::Snapshot>, &Story),
    ) {
        let state = snapshot.map(|snapshot| &snapshot.state);
        match link {
            RunLink::Command { command } => self.out = Some(Asked::Send(command)),
            RunLink::Url { href } => context.open_url(egui::OpenUrl::new_tab(href)),
            RunLink::Object { exist, noun, coord } => {
                if let Some(command) = coord
                    .as_deref()
                    .and_then(|coord| link_command(coord, (&exist, &noun), state))
                {
                    self.out = Some(Asked::Send(command));
                    return;
                }
                self.menus_asked = self.menus_asked.wrapping_add(1);
                let number = self.menus_asked;
                self.out = Some(Asked::Quietly(format!("_menu #{exist} {number}")));
                self.asking = Some(Asking {
                    number,
                    exist,
                    noun,
                    at,
                    generation: snapshot.map(|snapshot| snapshot.generation),
                    after: story.menu.as_ref().map(|(count, _)| *count),
                });
            }
        }
    }

    /// The object's menu, once the game has answered the last asked, over
    /// everything where the click was: an entry chosen sends its command.
    /// Escape, or a press anywhere else, closes it.
    pub(super) fn object_menu(&mut self, context: &egui::Context, view: &PlayView<'_>) {
        let Some(asking) = &self.asking else {
            return;
        };
        if context.input(|input| input.key_pressed(egui::Key::Escape)) {
            self.asking = None;
            return;
        }
        if view.snapshot.map(|snapshot| snapshot.generation) != asking.generation {
            self.asking = None;
            return;
        }
        let number = asking.number.to_string();
        let answer = view
            .story
            .menu
            .as_ref()
            .filter(|(count, menu)| Some(*count) > asking.after && menu.id == number);
        let Some((_, menu)) = answer else {
            return;
        };
        let state = view.snapshot.map(|snapshot| &snapshot.state);
        let groups = object_menu(menu, (&asking.exist, &asking.noun), state);
        let mut chosen = None;
        let shown = egui::Area::new(Id::new(("object-menu", self.session)))
            .order(Order::Foreground)
            .fixed_pos(asking.at)
            .show(context, |ui| {
                egui::Frame::menu(ui.style()).show(ui, |ui| {
                    ui.set_min_width(180.0);
                    ui.weak(&asking.noun);
                    entries(ui, &groups, &mut chosen);
                });
            });
        let elsewhere = context.input(|input| {
            input.pointer.primary_pressed()
                && input
                    .pointer
                    .interact_pos()
                    .is_some_and(|at| !shown.response.rect.contains(at))
        });
        if let Some(command) = chosen {
            self.out = Some(Asked::Send(command));
            self.asking = None;
        } else if elsewhere {
            self.asking = None;
        }
    }
}

/// `groups`' entries: a top-level category's in place, after a line from
/// the one before; one a level down under its name, closed until opened.
/// An entry that cannot be sent is shown, greyed.
fn entries(ui: &mut egui::Ui, groups: &[MenuGroup], chosen: &mut Option<String>) {
    for (index, group) in groups.iter().enumerate() {
        let list = |ui: &mut egui::Ui, chosen: &mut Option<String>| {
            for entry in &group.entries {
                let button = egui::Button::new(&entry.label).frame(false);
                if ui.add_enabled(entry.command.is_some(), button).clicked() {
                    chosen.clone_from(&entry.command);
                }
            }
        };
        if group.under.is_empty() {
            if index > 0 {
                ui.separator();
            }
            list(ui, chosen);
        } else {
            egui::CollapsingHeader::new(group.under.join(" › "))
                .id_salt(("object-menu-group", index))
                .default_open(false)
                .show(ui, |ui| list(ui, chosen));
        }
    }
}
