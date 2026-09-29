//! Drawing a log window: its tabs, the day and its lines, search and export.
//! What it asks to read goes on the window's list for the app to hand on.

use cena_ui::settings::size;

use super::{Ask, Logs, Preset, Shown, Tab, class, open_folder, shown};

impl Logs {
    /// Draw the window into `ui`.
    pub(crate) fn show(&mut self, ui: &mut egui::Ui) {
        self.take_replies();
        ui.horizontal(|ui| {
            for (tab, label) in [
                (Tab::Recent, "Recent"),
                (Tab::Search, "Search"),
                (Tab::Export, "Export"),
            ] {
                ui.selectable_value(&mut self.tab, tab, label);
            }
            if self.waiting > 0 {
                ui.spinner();
            }
        });
        ui.separator();
        match self.tab {
            Tab::Recent => self.recent(ui),
            Tab::Search => self.search(ui),
            Tab::Export => self.export(ui),
        }
    }

    /// A day, whole: the day picker, the filters, the lines.
    fn recent(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            ui.label("Day");
            let chosen = self.day.clone().unwrap_or_default();
            let mut picked = None;
            egui::ComboBox::from_id_salt(("log-day", &self.character))
                .selected_text(self.day_label(&chosen))
                .show_ui(ui, |ui| {
                    for (day, _) in &self.days {
                        if ui
                            .selectable_label(*day == chosen, self.day_label(day))
                            .clicked()
                        {
                            picked = Some(day.clone());
                        }
                    }
                });
            if let Some(day) = picked.filter(|day| *day != chosen) {
                self.asked.push(Ask::Day(day.clone()));
                self.day = Some(day);
                self.lines.clear();
            }
            if ui
                .button("Refresh")
                .on_hover_text("Read the days and this day again")
                .clicked()
            {
                self.asked.push(Ask::Days);
                if let Some(day) = self.day.clone() {
                    self.asked.push(Ask::Day(day));
                }
            }
        });
        self.filters(ui);
        let lines = shown(&self.lines, &self.ticked, self.dedup);
        lines_area(ui, &lines, false, true);
        self.footer(ui);
    }

    /// The presets, *Dedup*, and a box per class the window has seen.
    fn filters(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            for preset in Preset::ALL {
                if ui.button(preset.label()).clicked() {
                    self.preset(preset);
                }
            }
            ui.checkbox(&mut self.dedup, "Dedup")
                .on_hover_text("Show a run of the same line once, with its count. Nothing is left out of an export.");
        });
        ui.horizontal_wrapped(|ui| {
            for (class, on) in &mut self.ticked {
                ui.checkbox(on, class.as_str());
            }
        });
        ui.separator();
    }

    /// Lines holding a text, newest first.
    fn search(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            let typed = ui.add(
                egui::TextEdit::singleline(&mut self.query)
                    .hint_text("Text to find, any case")
                    .desired_width(320.0),
            );
            ui.checkbox(&mut self.regex, "Regex")
                .on_hover_text("A regular expression, where case matters unless it says (?i)");
            let entered = typed.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
            if (ui.button("Search").clicked() || entered) && !self.query.trim().is_empty() {
                self.asked
                    .push(Ask::Search(self.query.trim().to_owned(), self.regex));
            }
        });
        self.filters(ui);
        if let Some(found) = &self.found {
            let lines = shown(&found.entries, &self.ticked, self.dedup);
            let more = if found.more {
                ", and more past the first 1000"
            } else {
                ""
            };
            ui.label(format!("{} found{more}, newest first", lines.len()));
            lines_area(ui, &lines, true, false);
        }
        self.footer(ui);
    }

    /// Days from one to another written to a file.
    fn export(&mut self, ui: &mut egui::Ui) {
        ui.label("Write every line of the days chosen, of the ticked kinds, to one text file.");
        egui::Grid::new(("log-export", &self.character)).show(ui, |ui| {
            for (label, value) in [("From", &mut self.from), ("To", &mut self.to)] {
                ui.label(label);
                egui::ComboBox::from_id_salt(("log-export", label, &self.character))
                    .selected_text(value.as_str())
                    .show_ui(ui, |ui| {
                        for (day, _) in &self.days {
                            ui.selectable_value(value, day.clone(), day);
                        }
                    });
                ui.end_row();
            }
        });
        self.filters(ui);
        if ui.button("Create log file").clicked() && !self.from.is_empty() {
            self.asked.push(Ask::Export(
                self.from.clone(),
                self.to.clone(),
                self.export_streams(),
            ));
        }
        if ui.button("Open exports folder").clicked() {
            self.said = open_folder(&self.folder().join("exports")).err();
        }
        self.footer(ui);
    }

    /// What was said, the disk used, and the folder.
    fn footer(&mut self, ui: &mut egui::Ui) {
        ui.separator();
        ui.horizontal(|ui| {
            if ui.button("Open logs folder").clicked() {
                self.said = open_folder(&self.folder()).err();
            }
            if let Some(usage) = self.usage {
                ui.label(format!(
                    "{} days · {} in all, {} plain, {} archived",
                    usage.days,
                    size(usage.total()),
                    size(usage.plain),
                    size(usage.archived)
                ));
            }
        });
        if let Some(said) = &self.said {
            ui.label(said.as_str());
        }
    }

    /// A day as the picker shows it: with its size, or that it is archived.
    pub(super) fn day_label(&self, day: &str) -> String {
        match self.days.iter().find(|(d, _)| d == day) {
            Some((_, Some(bytes))) => format!("{day} · {}", size(*bytes)),
            Some((_, None)) => format!("{day} · archived"),
            None if day.is_empty() => "none kept yet".to_owned(),
            None => day.to_owned(),
        }
    }
}

/// The lines, as many as fit drawn: `time [class] text`, the day first when
/// they span days; the newest kept in view when `follow`.
fn lines_area(ui: &mut egui::Ui, lines: &[Shown<'_>], with_day: bool, follow: bool) {
    let height = ui.text_style_height(&egui::TextStyle::Monospace) + ui.spacing().item_spacing.y;
    egui::ScrollArea::both()
        .auto_shrink(false)
        .stick_to_bottom(follow)
        .max_height(ui.available_height() - 64.0)
        .show_rows(ui, height, lines.len(), |ui, rows| {
            for (entry, count) in &lines[rows] {
                ui.horizontal(|ui| {
                    let at = entry.at.get(..8).unwrap_or(&entry.at);
                    let when = if with_day {
                        format!("{} {at}", entry.day)
                    } else {
                        at.to_owned()
                    };
                    ui.label(egui::RichText::new(when).monospace().weak());
                    ui.label(
                        egui::RichText::new(class(&entry.stream))
                            .small()
                            .background_color(ui.visuals().faint_bg_color),
                    )
                    .on_hover_text(&entry.stream);
                    let text = if *count > 1 {
                        format!("{}  ×{count}", entry.text)
                    } else {
                        entry.text.clone()
                    };
                    ui.add(egui::Label::new(egui::RichText::new(text).monospace()).extend());
                });
            }
        });
}
