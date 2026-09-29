//! A text widget's scrollback split (the author, 2026-09-29): *"scroll up
//! and screen splits, top is what you're scrolling, top does not scroll when
//! new lines come in only when you scroll it. bottom sticks to the newest
//! line as it comes in. the split separator is draggable to adjust their
//! proportions. there is a button on the separator line that when clicked
//! scrolls you back to the bottom removing the split."*
//!
//! One pane, stuck to the newest line, until the player scrolls it back:
//! then two, the top where the player left it and the bottom following the
//! newest. The top closes the split too when scrolled back down to the
//! newest, as a split in Mudlet does.
//!
//! **The top holds its place when old lines go.** A story keeps its newest
//! thousand lines, so once full, each new line drops the oldest and the rest
//! move up by its height. The top is kept on its lines, not its offset: the
//! widget marks where each line starts ([`Tops`]), and each frame the top
//! moves down by the height of the lines dropped since the last.
//!
//! **Keys scroll it too** (`plan/52` step 4): a page or a line back opens
//! the split as the wheel does, and scrolls its top; forward scrolls the top
//! down, and to the newest line closes it, as its button does. A key's
//! scroll is asked of a widget by its id ([`ask`]) and done as it is next
//! drawn, by this or by any widget that scrolls ([`asked`]).

use egui::{CursorIcon, Id, Rect, Sense, Stroke, vec2};

use crate::text::AMBER;
#[cfg(test)]
use scroll::ASKED;
use scroll::steps;
pub(crate) use scroll::{Scroll, ask};
pub(super) use scroll::{Tops, asked, keyed};

/// How tall the separator is, to take a drag.
const BAR: f32 = 14.0;

/// The smallest share of the height either pane is left.
const LEAST: f32 = 0.1;

/// A widget's split, kept from frame to frame.
#[derive(Clone, Debug)]
struct Split {
    /// The top pane is open.
    open: bool,
    /// The top pane's share of the height.
    share: f32,
    /// The top pane's offset when last drawn.
    offset: f32,
    /// The top pane is to start at `offset`: it opened this frame, or old
    /// lines went.
    place: bool,
    /// The number of the first line kept when last drawn.
    first: u64,
    /// Where each line started when last drawn.
    ys: Vec<f32>,
    /// Times the split opened or closed: each pane's id takes it, so each
    /// begins afresh, and a fresh pane stuck to the newest line starts
    /// there. An offset set past the end is not pulled back by egui, and
    /// left the pane empty below its lines.
    turns: u64,
    /// The one pane has just come back and is not yet at the newest line:
    /// it takes no wheel until it is. A fresh pane starts at its top and is
    /// taken to its end as its first frame closes, which egui skips on a
    /// frame with a wheel turn in it -- and the rest of the turn that
    /// scrolled the top down to close the split is such a frame. It left
    /// the pane far back, and split it again.
    settling: bool,
    /// The pane the player scrolls -- the one, or the top -- as last drawn:
    /// how tall it is, and its offset at the newest line.
    height: f32,
    end: f32,
}

impl Default for Split {
    fn default() -> Self {
        Self {
            open: false,
            share: 0.6,
            offset: 0.0,
            place: false,
            first: 0,
            ys: Vec::new(),
            turns: 0,
            settling: false,
            height: 0.0,
            end: 0.0,
        }
    }
}

/// A scrolled body of lines, newest at the bottom, split when the player
/// scrolls back. `first` is the number of the first line kept, which grows
/// as old lines are dropped; `wrap` whether lines wrap, or the body scrolls
/// sideways too; `scroll` what a key asked; `add` draws every line, marking
/// each on its [`Tops`], and may be drawn twice a frame, once a pane.
pub(super) fn scrolled(
    ui: &mut egui::Ui,
    id: Id,
    (first, wrap): (u64, bool),
    scroll: Option<Scroll>,
    mut add: impl FnMut(&mut egui::Ui, &mut Tops),
) {
    let state_id = id.with("split");
    let mut split: Split = ui
        .data_mut(|data| data.get_temp(state_id))
        .unwrap_or_default();
    let mut body = |ui: &mut egui::Ui, player: bool| {
        if !wrap {
            ui.style_mut().wrap_mode = Some(egui::TextWrapMode::Extend);
        }
        let mut tops = Tops::at(ui, player);
        add(ui, &mut tops);
        tops.ys
    };
    if split.open {
        both(ui, id, &mut split, (first, wrap, scroll), &mut body);
    } else {
        let mut area = pane(wrap)
            .id_salt(id.with(("one", split.turns)))
            .stick_to_bottom(true);
        // A key back from the newest line: the pane goes there, and splits.
        // Scrolled from inside, at once: an offset given the pane is not
        // the player's scroll to egui, and sticking to the newest line
        // takes it straight back.
        let (line, page) = steps(ui, split.height);
        let back = match scroll {
            Some(Scroll::PageUp) => Some(page),
            Some(Scroll::LineUp) => Some(line),
            Some(Scroll::Top) => Some(split.end),
            _ => None,
        };
        let back = back.filter(|_| split.end > 0.0);
        if back.is_some() {
            split.settling = false;
        }
        if split.settling {
            area = area.scroll_source(egui::scroll_area::ScrollSource::NONE);
        }
        let shown = area.show(ui, |ui| {
            if let Some(back) = back {
                let now = egui::style::ScrollAnimation::none();
                ui.scroll_with_delta_animation(vec2(0.0, back), now);
            }
            body(ui, true)
        });
        let newest = newest(&shown);
        split.height = shown.inner_rect.height();
        split.end = newest;
        let back = shown.state.offset.y < newest - 1.0;
        if split.settling {
            split.settling = back;
        } else if back {
            // Scrolled back: the top pane starts where the player is.
            split.open = true;
            split.offset = shown.state.offset.y;
            split.place = true;
            split.first = first;
            split.ys = shown.inner;
            split.turns += 1;
        }
    }
    ui.data_mut(|data| data.insert_temp(state_id, split));
}

/// Both panes and the separator between them.
fn both(
    ui: &mut egui::Ui,
    id: Id,
    split: &mut Split,
    (first, wrap, scroll): (u64, bool, Option<Scroll>),
    body: &mut impl FnMut(&mut egui::Ui, bool) -> Vec<f32>,
) {
    let whole = ui.available_rect_before_wrap();
    let panes = (whole.height() - BAR).max(0.0);
    let top_height = (panes * split.share).round();
    let top_rect = Rect::from_min_size(whole.min, vec2(whole.width(), top_height));
    let bar_rect = Rect::from_min_size(top_rect.left_bottom(), vec2(whole.width(), BAR));
    let bottom_rect = Rect::from_min_max(bar_rect.left_bottom(), whole.max);

    // A key scrolls the top, never past the newest line.
    let (line, page) = steps(ui, split.height);
    let moved = match scroll {
        Some(Scroll::PageUp) => Some(split.offset - page),
        Some(Scroll::PageDown) => Some(split.offset + page),
        Some(Scroll::LineUp) => Some(split.offset - line),
        Some(Scroll::LineDown) => Some(split.offset + line),
        Some(Scroll::Top) => Some(0.0),
        Some(Scroll::Bottom) | None => None,
    };
    if let Some(to) = moved {
        split.offset = to.clamp(0.0, split.end.max(0.0));
        split.place = true;
    }

    // The top: where the player left it, moved down by the lines dropped.
    let gone = usize::try_from(first.saturating_sub(split.first)).unwrap_or(usize::MAX);
    let shift = if gone == 0 {
        0.0
    } else {
        split.ys.get(gone).copied().unwrap_or(split.offset)
    };
    let mut top = pane(wrap).id_salt(id.with(("top", split.turns)));
    if std::mem::take(&mut split.place) || shift > 0.0 {
        top = top.vertical_scroll_offset((split.offset - shift).max(0.0));
    }
    let shown = ui
        .scope_builder(egui::UiBuilder::new().max_rect(top_rect), |ui| {
            top.show(ui, |ui| body(ui, true))
        })
        .inner;
    let at_newest = shown.state.offset.y >= newest(&shown) - 1.0;
    split.height = shown.inner_rect.height();
    split.end = newest(&shown);
    split.offset = shown.state.offset.y;
    split.first = first;
    split.ys = shown.inner;

    // The separator: dragged to share the height, its button back to the
    // newest line.
    let bar = ui.interact(bar_rect, id.with("bar"), Sense::drag());
    let bar = bar.on_hover_and_drag_cursor(CursorIcon::ResizeVertical);
    if bar.dragged() && panes > 0.0 {
        split.share = (split.share + bar.drag_delta().y / panes).clamp(LEAST, 1.0 - LEAST);
    }
    let line = if bar.hovered() || bar.dragged() {
        Stroke::new(2.0, AMBER)
    } else {
        ui.visuals().widgets.noninteractive.bg_stroke
    };
    ui.painter()
        .hline(bar_rect.x_range(), bar_rect.center().y, line);
    let button = egui::Button::new("⬇ Newest").small();
    let size = vec2(72.0, BAR);
    let back = ui
        .put(Rect::from_center_size(bar_rect.center(), size), button)
        .on_hover_text("Back to the newest line, and one pane again");

    // The bottom: always the newest, which neither the wheel nor a bar
    // takes it from.
    ui.scope_builder(egui::UiBuilder::new().max_rect(bottom_rect), |ui| {
        pane(wrap)
            .id_salt(id.with(("bottom", split.turns)))
            .stick_to_bottom(true)
            .scroll_source(egui::scroll_area::ScrollSource::NONE)
            .show(ui, |ui| body(ui, false));
    });
    ui.advance_cursor_after_rect(whole);

    if back.clicked() || at_newest || scroll == Some(Scroll::Bottom) {
        split.open = false;
        split.turns += 1;
        split.settling = true;
    }
}

/// A pane of lines, as big as it is given.
fn pane(wrap: bool) -> egui::ScrollArea {
    let area = if wrap {
        egui::ScrollArea::vertical()
    } else {
        egui::ScrollArea::both()
    };
    area.min_scrolled_height(0.0).auto_shrink(false)
}

/// The offset a pane is at when its newest line is at its bottom.
fn newest<R>(shown: &egui::scroll_area::ScrollAreaOutput<R>) -> f32 {
    (shown.content_size.y - shown.inner_rect.height()).max(0.0)
}

mod scroll;
#[cfg(test)]
mod tests;
