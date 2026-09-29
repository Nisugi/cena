//! The Find bar (`plan/52` step 6): over the window in use when it is a
//! story or a stream, over the story when it is not, looking through its
//! lines as they are typed. Ctrl+F opens it and takes the keyboard to it;
//! F3, and Enter in it, go back to the line found before, Shift+F3 and
//! Shift+Enter forward; Escape, or its ×, closes it. The widget does the
//! finding (`crate::widget::find`), asked each frame by its id.

use egui::{Id, Order, Rect, pos2};

use super::{Play, draw};
use crate::widget::find::{self, Finding};

/// The Find bar, while it is open.
#[derive(Debug)]
pub(super) struct FindBar {
    /// What is looked for, as typed.
    query: String,
    /// Which line found is current, from the newest.
    current: usize,
    /// The widget looked through, by placed id.
    target: u32,
    /// The bar's field is to take the keyboard.
    focus: bool,
    /// The widget is to be brought to the current line.
    go: bool,
}

/// How wide the bar's field is.
const FIELD: f32 = 160.0;

impl Play {
    /// Open the Find bar over the window in use, or the story, or take the
    /// keyboard back to it.
    pub(super) fn open_find(&mut self) {
        if let Some(bar) = &mut self.find {
            bar.focus = true;
            return;
        }
        let Some(layout) = self.layout.as_ref() else {
            return;
        };
        let placed = layout.placed();
        let lines = |id: u32| {
            placed
                .iter()
                .any(|placed| placed.id == id && placed.widget.has_lines())
        };
        let target = self.in_use().filter(|id| lines(*id)).or_else(|| {
            placed
                .iter()
                .find(|placed| placed.widget.is_story())
                .map(|placed| placed.id)
        });
        if let Some(target) = target {
            self.find = Some(FindBar {
                query: String::new(),
                current: 0,
                target,
                focus: true,
                go: false,
            });
        }
    }

    /// Go to the line found before the current one (`older`), or after.
    pub(super) fn step_find(&mut self, context: &egui::Context, older: bool) {
        let session = self.session;
        let Some(bar) = &mut self.find else {
            return;
        };
        let found = find::found(context, draw::widget_id(session, bar.target)).unwrap_or(0);
        bar.current = if older {
            (bar.current + 1).min(found.saturating_sub(1))
        } else {
            bar.current.saturating_sub(1)
        };
        bar.go = true;
    }

    /// Tell the widget looked through what to find this frame, before it
    /// is drawn.
    pub(super) fn ask_find(&mut self, context: &egui::Context) {
        let session = self.session;
        if let Some(bar) = &mut self.find {
            let finding = Finding {
                query: bar.query.clone(),
                current: bar.current,
                go: std::mem::take(&mut bar.go),
            };
            find::ask(context, draw::widget_id(session, bar.target), Some(finding));
        }
    }

    /// The bar, over the top right of the widget it looks through.
    pub(super) fn find_bar(&mut self, context: &egui::Context, area: Rect) {
        let session = self.session;
        let Some(bar) = &mut self.find else {
            return;
        };
        let widget = draw::widget_id(session, bar.target);
        let over = self
            .shown_rects
            .iter()
            .find(|(id, _)| *id == bar.target)
            .map_or(area, |(_, rect)| *rect);
        let found = find::found(context, widget);
        let mut closed = false;
        let mut stepped = None;
        let at = pos2(
            (over.right() - FIELD - 170.0).max(over.left()),
            over.top() + 2.0,
        );
        egui::Area::new(Id::new(("play-find", session)))
            .order(Order::Foreground)
            .fixed_pos(at)
            .show(context, |ui| {
                egui::Frame::popup(ui.style()).show(ui, |ui| {
                    ui.horizontal(|ui| {
                        let label = ui.label("Find");
                        let field = ui
                            .add(
                                egui::TextEdit::singleline(&mut bar.query)
                                    .id(Id::new(("play-find-field", session)))
                                    .desired_width(FIELD),
                            )
                            .labelled_by(label.id);
                        if std::mem::take(&mut bar.focus) {
                            field.request_focus();
                        }
                        if field.changed() {
                            bar.current = 0;
                            bar.go = true;
                        }
                        let (enter, escape, shift) = ui.input(|input| {
                            (
                                input.key_pressed(egui::Key::Enter),
                                input.key_pressed(egui::Key::Escape),
                                input.modifiers.shift,
                            )
                        });
                        if field.lost_focus() && enter {
                            stepped = Some(!shift);
                            field.request_focus();
                        }
                        closed |= field.lost_focus() && escape;
                        match found {
                            Some(0) if !bar.query.trim().is_empty() => {
                                ui.weak("none");
                            }
                            Some(all) if all > 0 => {
                                ui.weak(format!("{} of {all}", bar.current + 1));
                            }
                            _ => {}
                        }
                        if ui
                            .small_button("⬆")
                            .on_hover_text("Found before (F3)")
                            .clicked()
                        {
                            stepped = Some(true);
                        }
                        if ui
                            .small_button("⬇")
                            .on_hover_text("Found after (Shift+F3)")
                            .clicked()
                        {
                            stepped = Some(false);
                        }
                        closed |= ui
                            .small_button("×")
                            .on_hover_text("Close (Escape)")
                            .clicked();
                    });
                });
            });
        if let Some(older) = stepped {
            self.step_find(context, older);
        }
        if closed {
            find::ask(context, widget, None);
            self.find = None;
        }
    }
}
