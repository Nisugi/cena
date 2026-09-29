//! A play window's drawers as drawn (`plan/49` Stage E; the model is
//! `crate::layout`'s `drawers.rs`): each open drawer's backdrop, the edge
//! it is resized by, and the Drawers menu on the top bar.
//!
//! **Layers.** The main area's windows are egui's `Middle` order; a
//! drawer's backdrop is `Foreground`, over them, and its windows and its
//! edge are sublayers of the backdrop, drawn just above it wherever egui
//! raises it (`egui::Context::set_sublayer`). `VellumFE` puts its drawers'
//! windows in `Foreground` too, and sinks the backdrop under them each frame
//! by raising every other layer (`zones.rs`, `render_overlay_backdrop`);
//! a sublayer says the same in one call.
//!
//! **Click-through** is egui's own: a backdrop that hides nothing is not
//! interactable, so egui's hit test passes it by (`Area::interactable`), and
//! a press reaches what is under it. The drawer's windows stay interactable,
//! so a press on one is the window's (`plan/28` §7d.5: *"'No window' means
//! no widget"*).

use egui::{CursorIcon, Id, LayerId, Order, Rect, Sense, Stroke};

use super::Play;
use crate::layout::{CLEAR, Drawers, Mode, SMALLEST, THINNEST, Zone, Zones};
use crate::text::AMBER;

/// How thick a drawer's edge is, to take a press: half in the drawer, half
/// out.
const EDGE: f32 = 6.0;

/// Drawer `zone`'s backdrop layer in play window `session`: its windows
/// are drawn just above it.
pub(super) fn backdrop(session: u32, zone: Zone) -> LayerId {
    LayerId::new(Order::Foreground, Id::new(("play-drawer", session, zone)))
}

/// Drawer `zone`'s edge layer.
fn edge(session: u32, zone: Zone) -> LayerId {
    LayerId::new(
        Order::Foreground,
        Id::new(("play-drawer-edge", session, zone)),
    )
}

impl Play {
    /// Each open drawer's backdrop in play area `area`, with the edge that
    /// resizes it, unless the layout is locked. `true` when a resize ended,
    /// so the layout wants saving.
    pub(super) fn drawers(&mut self, context: &egui::Context, area: Rect) -> bool {
        let session = self.session;
        let Some(layout) = self.layout.as_mut() else {
            return false;
        };
        let zones = Zones::of(&layout.drawers, area.size());
        let mut ended = false;
        for &(zone, rect) in zones.drawers() {
            let Some(drawer) = layout.drawers.get_mut(zone) else {
                continue;
            };
            let at = rect.translate(area.min.to_vec2());
            let layer = backdrop(session, zone);
            let opacity = drawer.shown_opacity();
            // Seen, it takes a press on its bare backdrop. Clear, it only
            // paints: egui's hit test stops at any widget in a layer, even
            // the one an area made that is not interactable.
            if !drawer.clear() {
                egui::Area::new(layer.id)
                    .order(Order::Foreground)
                    .fixed_pos(at.min)
                    .show(context, |ui| {
                        ui.allocate_exact_size(at.size(), Sense::click_and_drag());
                    });
            }
            let visuals = &context.global_style().visuals;
            let painter = context.layer_painter(layer);
            painter.rect_filled(at, 0.0, visuals.panel_fill.gamma_multiply(opacity));
            // The line along its inner edge fades with it: fading only the
            // fill "would leave a hairline floating in space" (`VellumFE`,
            // `zones.rs:1435-1440`).
            let line = Stroke::new(1.5, visuals.window_stroke.color.gamma_multiply(opacity));
            let inner = Zones::edge(zone, at, 0.0);
            painter.line_segment([inner.min, inner.max], line);
            if layout.locked {
                continue;
            }
            let strip = Zones::edge(zone, at, EDGE);
            let edge_layer = edge(session, zone);
            egui::Area::new(edge_layer.id)
                .order(Order::Foreground)
                .fixed_pos(strip.min)
                .show(context, |ui| {
                    let cursor = if Zones::sideways(zone) {
                        CursorIcon::ResizeHorizontal
                    } else {
                        CursorIcon::ResizeVertical
                    };
                    let (_, response) = ui.allocate_exact_size(strip.size(), Sense::drag());
                    let response = response.on_hover_and_drag_cursor(cursor);
                    if response.hovered() || response.dragged() {
                        let middle = Zones::edge(zone, at, 0.0);
                        ui.painter()
                            .line_segment([middle.min, middle.max], Stroke::new(2.0, AMBER));
                    }
                    if response.dragged() {
                        let most = if Zones::sideways(zone) {
                            area.width() - SMALLEST.x
                        } else {
                            area.height() - SMALLEST.y
                        };
                        let grown =
                            drawer.size.max(THINNEST) + Zones::grown(zone, response.drag_delta());
                        drawer.size = grown.clamp(THINNEST, most.max(THINNEST));
                    }
                    ended |= response.drag_stopped();
                });
            context.set_sublayer(layer, edge_layer);
            context.move_to_top(edge_layer);
        }
        ended
    }
}

/// The Drawers menu: each drawer shown or not, pushing or clipping, and a
/// clip drawer's opacity. Whether anything changed.
pub(super) fn menu(ui: &mut egui::Ui, drawers: &mut Drawers) -> bool {
    let mut changed = false;
    for zone in Zone::DRAWERS {
        let Some(drawer) = drawers.get_mut(zone) else {
            continue;
        };
        ui.horizontal(|ui| {
            changed |= ui.checkbox(&mut drawer.open, zone.name()).changed();
            changed |= ui
                .radio_value(&mut drawer.mode, Mode::Push, "Push")
                .on_hover_text("Take its room from the main area, whose windows move over")
                .changed();
            changed |= ui
                .radio_value(&mut drawer.mode, Mode::Clip, "Clip")
                .on_hover_text("Lie over the main area, which stays where it is")
                .changed();
        });
        if drawer.mode == Mode::Clip {
            ui.horizontal(|ui| {
                ui.add_space(24.0);
                changed |= ui
                    .add(egui::Slider::new(&mut drawer.opacity, 0.0..=1.0).text("Opacity").fixed_decimals(2))
                    .on_hover_text(format!(
                        "How much of the main area it hides. Below {CLEAR:.2}, a click on the drawer where no window is reaches what is under it."
                    ))
                    .changed();
            });
        }
    }
    ui.weak("Drag a window into an open drawer to keep it there; drag a drawer's edge to size it.");
    changed
}
