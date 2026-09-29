//! The trigger editor's state, and its list and chosen trigger drawn.

use std::collections::BTreeSet;

use cena_ui::triggers::{Book, Change, Entry, Switch};

/// The trigger editor.
#[derive(Debug, Default)]
pub(crate) struct Editor {
    /// Whether it is showing.
    pub(crate) open: bool,
    /// What the list is narrowed to: words in a name, a category or what a
    /// trigger does.
    pub(super) search: String,
    /// The trigger shown on the right, by name.
    chosen: Option<String>,
    /// A trigger the player asked to remove, waiting on *Remove* again.
    removing: Option<String>,
    /// Categories folded shut.
    folded: BTreeSet<String>,
}

impl Editor {
    /// Open it, and whether it was shut: a window opening asks for the file.
    pub(crate) fn open(&mut self) -> bool {
        !std::mem::replace(&mut self.open, true)
    }

    /// Draw it over `book` (`None` until the binary has answered), with the
    /// binary's last answer; what the player asked to change.
    pub(crate) fn show(
        &mut self,
        ui: &mut egui::Ui,
        book: Option<&Book>,
        said: Option<&str>,
    ) -> Vec<Change> {
        let mut asked = Vec::new();
        let Some(book) = book else {
            ui.spinner();
            ui.label("Reading the triggers file...");
            return asked;
        };
        ui.horizontal(|ui| {
            ui.add(
                egui::TextEdit::singleline(&mut self.search)
                    .hint_text("Search names, categories, what they do")
                    .desired_width(280.0),
            );
            ui.separator();
            ui.label("Everywhere:");
            for (kind, on) in &book.kinds {
                let mut now = *on;
                if ui
                    .checkbox(&mut now, kind.as_str())
                    .on_hover_text(format!("Every trigger's {kind}, on or off"))
                    .changed()
                {
                    asked.push(Change::Switch(Switch::Every(kind.clone()), now));
                }
            }
        });
        if let Some(problem) = &book.problem {
            ui.colored_label(ui.visuals().warn_fg_color, problem.as_str());
        }
        if let Some(said) = said {
            ui.label(said);
        }
        ui.separator();
        let height = ui.available_height();
        ui.horizontal_top(|ui| {
            ui.vertical(|ui| {
                ui.set_width(380.0);
                egui::ScrollArea::vertical()
                    .id_salt("trigger-list")
                    .max_height(height)
                    .auto_shrink(false)
                    .show(ui, |ui| self.list(ui, book, &mut asked));
            });
            ui.separator();
            ui.vertical(|ui| self.chosen(ui, book, &mut asked));
        });
        asked
    }

    /// Every trigger the search keeps, by category, each with its switch.
    fn list(&mut self, ui: &mut egui::Ui, book: &Book, asked: &mut Vec<Change>) {
        if book.triggers.is_empty() {
            ui.label("No triggers yet.");
            return;
        }
        let search = self.search.to_lowercase();
        let kept = |entry: &Entry| {
            search.is_empty()
                || [&entry.name, &entry.category, &entry.summary]
                    .iter()
                    .any(|text| text.to_lowercase().contains(&search))
        };
        let mut at = 0;
        while let Some(first) = book.triggers.get(at) {
            let category = first.category.clone();
            let group: Vec<&Entry> = book.triggers[at..]
                .iter()
                .take_while(|entry| entry.category == category)
                .collect();
            at += group.len();
            let shown: Vec<&Entry> = group.iter().copied().filter(|e| kept(e)).collect();
            if shown.is_empty() {
                continue;
            }
            self.category(ui, book, &category, group.len(), asked);
            if self.folded.contains(&category) && search.is_empty() {
                continue;
            }
            for entry in shown {
                self.row(ui, entry, asked);
            }
        }
    }

    /// A category's heading: fold it, count it, switch it.
    fn category(
        &mut self,
        ui: &mut egui::Ui,
        book: &Book,
        category: &str,
        count: usize,
        asked: &mut Vec<Change>,
    ) {
        ui.horizontal(|ui| {
            let folded = self.folded.contains(category);
            if ui.small_button(if folded { "▸" } else { "▾" }).clicked() {
                if folded {
                    self.folded.remove(category);
                } else {
                    self.folded.insert(category.to_owned());
                }
            }
            let shown = if category.is_empty() {
                "(no category)"
            } else {
                category
            };
            ui.strong(format!("{shown} ({count})"));
            if !category.is_empty() {
                let on = book
                    .categories
                    .iter()
                    .find(|(name, _)| name == category)
                    .is_none_or(|(_, on)| *on);
                let mut now = on;
                if ui
                    .checkbox(&mut now, "on")
                    .on_hover_text("Every trigger in this category, on or off")
                    .changed()
                {
                    asked.push(Change::Switch(Switch::Category(category.to_owned()), now));
                }
            }
        });
    }

    /// One trigger: its switch, its name, what it does, and what is wrong.
    fn row(&mut self, ui: &mut egui::Ui, entry: &Entry, asked: &mut Vec<Change>) {
        ui.horizontal(|ui| {
            ui.add_space(16.0);
            let mut on = entry.enabled;
            if ui.checkbox(&mut on, "").changed() {
                asked.push(Change::Switch(Switch::Trigger(entry.name.clone()), on));
            }
            let chosen = self.chosen.as_deref() == Some(entry.name.as_str());
            if ui.selectable_label(chosen, entry.name.as_str()).clicked() {
                self.chosen = Some(entry.name.clone());
                self.removing = None;
            }
            if entry.refused.is_some() {
                ui.colored_label(ui.visuals().error_fg_color, "refused");
            } else if entry.held.is_some() {
                ui.colored_label(ui.visuals().warn_fg_color, "send waits");
            }
        });
        ui.horizontal(|ui| {
            ui.add_space(40.0);
            ui.label(egui::RichText::new(&entry.summary).weak().small());
        });
    }

    /// The chosen trigger: what it is, and what can be done to it.
    fn chosen(&mut self, ui: &mut egui::Ui, book: &Book, asked: &mut Vec<Change>) {
        let Some(entry) = self
            .chosen
            .as_ref()
            .and_then(|name| book.triggers.iter().find(|entry| entry.name == *name))
        else {
            ui.label("Choose a trigger on the left.");
            return;
        };
        ui.heading(entry.name.as_str());
        egui::Grid::new("trigger-chosen")
            .num_columns(2)
            .show(ui, |ui| {
                ui.label("Category");
                ui.label(if entry.category.is_empty() {
                    "(none)"
                } else {
                    entry.category.as_str()
                });
                ui.end_row();
                ui.label("Does");
                ui.label(entry.summary.as_str());
                ui.end_row();
                if let Some(origin) = &entry.origin {
                    ui.label("From");
                    ui.label(origin.as_str());
                    ui.end_row();
                }
            });
        if let Some(why) = &entry.refused {
            ui.colored_label(
                ui.visuals().error_fg_color,
                format!("Refused, so it does nothing: {why}"),
            );
        }
        if let Some(line) = &entry.held {
            ui.colored_label(
                ui.visuals().warn_fg_color,
                format!("It came from elsewhere, and sends \"{line}\" only once you approve it."),
            );
            if ui.button("Approve this command").clicked() {
                asked.push(Change::Approve(entry.name.clone()));
            }
        }
        ui.separator();
        if self.removing.as_deref() == Some(entry.name.as_str()) {
            ui.horizontal(|ui| {
                ui.label(format!("Remove `{}` from the file?", entry.name));
                if ui.button("Remove").clicked() {
                    asked.push(Change::Remove(entry.name.clone()));
                    self.removing = None;
                    self.chosen = None;
                }
                if ui.button("Keep it").clicked() {
                    self.removing = None;
                }
            });
        } else if ui.button("Remove...").clicked() {
            self.removing = Some(entry.name.clone());
        }
        ui.add_space(8.0);
        ui.label(
            egui::RichText::new(format!(
                "Kept in {}. Editing a trigger's words and responses is the next step (plan/54 step 2); until then, ;trigger set.",
                book.file
            ))
            .weak()
            .small(),
        );
    }
}
