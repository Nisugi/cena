//! The Game state widget. The author asked for it on 2026-09-27: *"a
//! `GameState` panel. Where you can see in real time everything in
//! `GameState`?"*, and *"Talk about troubleshooting!!"*
//!
//! It shows everything the model holds for the character, as the model
//! prints itself (`GameState`'s `Debug`). So no field is left off, and a
//! field added to the model shows here with no change to this file. It is a
//! tree, closed below its top; a filter finds any line and shows where it
//! is; and *Copy* takes the whole print for a report.
//!
//! The committed fixtures, folded together, print about 600 KB: 15,676
//! lines, formatted in 6 ms in a debug build. So the print is taken at most
//! twice a second, not every frame.

use std::sync::Arc;
use std::time::{Duration, Instant};

use cena_session::GameState;
use egui::{Id, RichText};

/// How often the print is taken again: often enough to watch, rarely
/// enough to cost nothing.
const EVERY: Duration = Duration::from_millis(500);

/// The most lines the filter lists.
const FOUND: usize = 500;

/// One line of the print: a value, or a branch that opens onto its lines.
#[derive(Debug, PartialEq, Eq)]
pub(super) enum Line {
    /// A value, as printed.
    Leaf(String),
    /// A struct, list or wrapper, by the line that opens it.
    Branch(String, Vec<Line>),
}

/// The print, as text and as a tree, and when it was taken.
struct Printed {
    at: Instant,
    text: String,
    tree: Vec<Line>,
}

/// `{:#?}`'s print as a tree. A line ending `{`, `[` or `(` opens a
/// branch, and one starting `}`, `]` or `)` closes it. A branch holding one
/// value is that value on one line: `id: Some("8213304")`, not three lines.
pub(super) fn tree(text: &str) -> Vec<Line> {
    // The open branches: each's label, its bracket, and its lines so far.
    let mut open: Vec<(String, char, Vec<Line>)> = vec![(String::new(), ' ', Vec::new())];
    for line in text.lines().map(str::trim) {
        if line.starts_with(['}', ']', ')']) {
            close(&mut open, line.ends_with(','));
        } else if let Some(bracket) = line.chars().last().filter(|c| matches!(c, '{' | '[' | '(')) {
            let label = line[..line.len() - 1].trim_end().to_owned();
            open.push((label, bracket, Vec::new()));
        } else if let Some((_, _, lines)) = open.last_mut() {
            lines.push(Line::Leaf(line.to_owned()));
        }
    }
    // A print cut short leaves branches open; they close where it stops.
    while open.len() > 1 {
        close(&mut open, false);
    }
    open.pop().map(|(_, _, lines)| lines).unwrap_or_default()
}

/// Close the innermost open branch into its parent.
fn close(open: &mut Vec<(String, char, Vec<Line>)>, comma: bool) {
    if open.len() < 2 {
        return;
    }
    let Some((label, bracket, mut lines)) = open.pop() else {
        return;
    };
    let line = match (lines.len(), lines.first()) {
        (1, Some(Line::Leaf(only))) => {
            let closing = match bracket {
                '{' => '}',
                '[' => ']',
                _ => ')',
            };
            let only = only.trim_end_matches(',');
            let comma = if comma { "," } else { "" };
            let text = if bracket == '{' {
                format!("{label} {{ {only} }}{comma}")
            } else {
                format!("{label}{bracket}{only}{closing}{comma}")
            };
            lines.clear();
            Line::Leaf(text)
        }
        _ => Line::Branch(label, lines),
    };
    if let Some((_, _, parent)) = open.last_mut() {
        parent.push(line);
    }
}

/// Every line whose text holds `needle` (ignoring case), each with the
/// branches it is under, at most [`FOUND`].
pub(super) fn found(lines: &[Line], needle: &str) -> Vec<String> {
    fn walk<'a>(lines: &'a [Line], needle: &str, under: &mut Vec<&'a str>, out: &mut Vec<String>) {
        for line in lines {
            if out.len() >= FOUND {
                return;
            }
            let (text, inside) = match line {
                Line::Leaf(text) => (text.as_str(), None),
                Line::Branch(label, lines) => (label.as_str(), Some(lines)),
            };
            if text.to_lowercase().contains(needle) {
                let mut path = under.join(" › ");
                if !path.is_empty() {
                    path.push_str(" › ");
                }
                out.push(format!("{path}{text}"));
            }
            if let Some(inside) = inside {
                under.push(text);
                walk(inside, needle, under, out);
                under.pop();
            }
        }
    }
    let mut out = Vec::new();
    walk(lines, &needle.to_lowercase(), &mut Vec::new(), &mut out);
    out
}

/// The widget: the state's print, a filter, and Copy.
pub(super) fn game_state(ui: &mut egui::Ui, state: Option<&GameState>, who: Option<&str>, id: Id) {
    if let Some(who) = who {
        ui.weak(who);
    }
    let Some(state) = state else {
        ui.weak("Nothing seen yet.");
        return;
    };
    let now = Instant::now();
    let kept = ui
        .data(|data| data.get_temp::<Arc<Printed>>(id))
        .filter(|printed| now.saturating_duration_since(printed.at) < EVERY);
    let printed = kept.unwrap_or_else(|| {
        let text = format!("{state:#?}");
        let printed = Arc::new(Printed {
            at: now,
            tree: tree(&text),
            text,
        });
        ui.data_mut(|data| data.insert_temp(id, Arc::clone(&printed)));
        printed
    });
    // Drawn again while open, so what it shows is never older than EVERY.
    ui.ctx().request_repaint_after(EVERY);
    let filter_id = id.with("filter");
    let mut filter = ui.data(|data| data.get_temp::<String>(filter_id).unwrap_or_default());
    ui.horizontal(|ui| {
        ui.add(
            egui::TextEdit::singleline(&mut filter)
                .hint_text("Find in the game state...")
                .desired_width((ui.available_width() - 60.0).max(80.0)),
        );
        if ui
            .button("Copy")
            .on_hover_text("The whole game state, as text, for a report")
            .clicked()
        {
            ui.ctx().copy_text(printed.text.clone());
        }
    });
    ui.data_mut(|data| data.insert_temp(filter_id, filter.clone()));
    egui::ScrollArea::both()
        .id_salt(id.with("scroll"))
        .auto_shrink(false)
        .show(ui, |ui| {
            let needle = filter.trim();
            if needle.is_empty() {
                // The print's one top branch is `GameState { .. }`: its fields
                // are the tree's top.
                let top = match printed.tree.as_slice() {
                    [Line::Branch(_, fields)] => fields.as_slice(),
                    all => all,
                };
                draw(ui, top, id.with("tree"));
                return;
            }
            let found = found(&printed.tree, needle);
            if found.is_empty() {
                ui.weak("Nothing matches.");
            }
            for line in found {
                ui.label(RichText::new(line).monospace());
            }
        });
}

/// `lines`, each branch closed until opened.
fn draw(ui: &mut egui::Ui, lines: &[Line], id: Id) {
    for (at, line) in lines.iter().enumerate() {
        match line {
            Line::Leaf(text) => {
                ui.label(RichText::new(text).monospace());
            }
            Line::Branch(label, inside) => {
                let id = id.with(at);
                egui::CollapsingHeader::new(RichText::new(label).monospace())
                    .id_salt(id)
                    .show(ui, |ui| draw(ui, inside, id));
            }
        }
    }
}

#[cfg(test)]
mod tests;
