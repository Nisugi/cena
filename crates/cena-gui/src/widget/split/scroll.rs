//! A key's scroll (`plan/52` step 4): asked of a widget by its id, and done
//! as it is next drawn, by the split or by any widget that scrolls. Moved
//! out of `split.rs` at its cap.

use egui::{Id, vec2};

/// A scroll a key asked of the window in use (`plan/52` step 4).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Scroll {
    /// Back a page, to older lines.
    PageUp,
    /// Forward a page.
    PageDown,
    /// Back a line.
    LineUp,
    /// Forward a line.
    LineDown,
    /// To the oldest line kept.
    Top,
    /// To the newest line.
    Bottom,
}

/// Where a widget's asked scroll is kept until it is drawn.
pub(super) const ASKED: &str = "keyed-scroll";

/// Ask the widget whose id is `id` to scroll, as it is next drawn.
pub(crate) fn ask(context: &egui::Context, id: Id, scroll: Scroll) {
    context.data_mut(|data| data.insert_temp(id.with(ASKED), scroll));
}

/// The scroll asked of the widget whose id is `id`, taken.
pub(crate) fn asked(ui: &egui::Ui, id: Id) -> Option<Scroll> {
    ui.data_mut(|data| {
        let asked = data.get_temp::<Scroll>(id.with(ASKED));
        data.remove::<Scroll>(id.with(ASKED));
        asked
    })
}

/// A line's height and a page's, in `ui`'s text, for a pane `height` tall:
/// a page keeps a line of the last in sight.
pub(super) fn steps(ui: &egui::Ui, height: f32) -> (f32, f32) {
    let line = ui.text_style_height(&egui::TextStyle::Body) + ui.spacing().item_spacing.y;
    (line, (height - line).max(line))
}

/// A body of lines, not split, scrolled as a key asked: inside its
/// scrolling area, before its lines are added (`Top` and the steps), or
/// after (`Bottom`).
pub(crate) fn keyed(ui: &mut egui::Ui, scroll: Option<Scroll>, add: impl FnOnce(&mut egui::Ui)) {
    let (line, page) = steps(ui, ui.clip_rect().height());
    let by = match scroll {
        Some(Scroll::PageUp) => page,
        Some(Scroll::PageDown) => -page,
        Some(Scroll::LineUp) => line,
        Some(Scroll::LineDown) => -line,
        Some(Scroll::Top | Scroll::Bottom) | None => 0.0,
    };
    if scroll == Some(Scroll::Top) {
        ui.scroll_to_cursor(Some(egui::Align::TOP));
    } else if by != 0.0 {
        ui.scroll_with_delta_animation(vec2(0.0, by), egui::style::ScrollAnimation::none());
    }
    add(ui);
    if scroll == Some(Scroll::Bottom) {
        ui.scroll_to_cursor(Some(egui::Align::BOTTOM));
    }
}
