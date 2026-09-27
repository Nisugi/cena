//! What a play window draws: its top bar, and what each of its windows
//! holds -- one widget, framed by its window, or a custom window's widgets,
//! bare in their cells (`plan/49` §2).

use egui::{Color32, Id, UiBuilder};

use super::PlayView;
use crate::layout::Holds;
use crate::text::{AMBER, WRONG};
use crate::widget::Seen;

/// What the top bar was asked this frame.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Top {
    /// Stop was pressed.
    Stop,
    /// The layout was to be fitted afresh.
    Fit,
    /// The grid's pitch was changed.
    Grid,
    /// The keybinds were to be read again.
    ReloadKeys,
}

/// The top bar: who, how connected, the keybinds, the layout's grid, Stop;
/// and any banner. What the hands hold and the clocks are widgets now, in
/// the layout with the rest.
pub(super) fn top(
    ui: &mut egui::Ui,
    view: &PlayView<'_>,
    grid: &mut f32,
    unsaved: Option<&str>,
) -> Option<Top> {
    let mut asked = None;
    ui.horizontal(|ui| {
        ui.strong(view.name);
        ui.label(crate::hub::lifecycle(view.lifecycle));
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if ui
                .button("Stop")
                .on_hover_text("Stop everything Hydra is doing on this character")
                .clicked()
            {
                asked = Some(Top::Stop);
            }
            ui.menu_button("Keys", |ui| {
                for said in view.keys {
                    ui.label(said);
                }
                if ui.button("Read the keybinds again").clicked() {
                    asked = Some(Top::ReloadKeys);
                    ui.close();
                }
            });
            ui.menu_button("Layout", |ui| {
                ui.horizontal(|ui| {
                    ui.label("Grid");
                    let changed = ui
                        .add(egui::DragValue::new(grid).range(0.0..=64.0).suffix(" pt"))
                        .on_hover_text("What the windows' edges snap to; 0 for none. Shift while dragging snaps to nothing.")
                        .changed();
                    if changed {
                        asked = Some(Top::Grid);
                    }
                });
                if ui.button("Lay out afresh").clicked() {
                    asked = Some(Top::Fit);
                    ui.close();
                }
            });
            if let Some(why) = unsaved {
                ui.colored_label(WRONG, format!("Layout not saved: {why}"));
            }
            if let Some(on) = view.numlock {
                ui.weak(if on { "NumLock on" } else { "NumLock off" });
            }
        });
    });
    for alert in view.story.alerts_at(view.now) {
        egui::Frame::new()
            .fill(AMBER)
            .inner_margin(4.0)
            .show(ui, |ui| {
                ui.colored_label(Color32::BLACK, alert);
            });
    }
    asked
}

/// A widget's egui id: its own in every play window, whatever holds it.
pub(super) fn widget_id(session: u32, placed: u32) -> Id {
    Id::new(("play-widget", session, placed))
}

/// What a window holds, filling it: its window is the size its layout says,
/// never its content's. One widget is given everything; a custom window's
/// widgets are drawn bare in their cells, once the cells are kept to the
/// inside it has now (`Custom::fit`). Nothing spills out of its cell: a
/// one-line widget stays one line, and the rest scroll.
pub(super) fn holder(ui: &mut egui::Ui, holds: &mut Holds, seen: &Seen<'_>, session: u32) {
    let inside = ui.available_rect_before_wrap();
    ui.set_min_size(inside.size());
    match holds {
        Holds::One(placed) => {
            placed.widget.draw(ui, seen, widget_id(session, placed.id));
        }
        Holds::Custom(custom) => {
            custom.fit(inside.size());
            for cell in &custom.cells {
                let Some(placed) = cell.shown() else {
                    continue;
                };
                let at = cell
                    .rect()
                    .translate(inside.min.to_vec2())
                    .intersect(inside);
                if !at.is_positive() {
                    continue;
                }
                let mut child =
                    ui.new_child(UiBuilder::new().max_rect(at).id_salt(("cell", placed.id)));
                placed
                    .widget
                    .draw(&mut child, seen, widget_id(session, placed.id));
            }
            ui.advance_cursor_after_rect(inside);
        }
    }
}
