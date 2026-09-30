//! Find in a story or a stream (`plan/52` step 6): what the play window's
//! Find bar looks for, asked of the widget by its id as a key's scroll is,
//! and how many lines it found, said back.
//!
//! The widget counts its lines that hold what is looked for, as they are
//! drawn, whatever their case; each is marked, the current one more; and
//! when the bar moves to another, the pane the player scrolls is brought to
//! it, which splits the story as scrolling back does. The current one counts
//! from the newest, so the first found is the latest said and *next* goes
//! back through what was.

use egui::Id;

use crate::theme::{self, T};

/// What the Find bar looks for in a widget.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(crate) struct Finding {
    /// What is looked for, as typed.
    pub(crate) query: String,
    /// Which line found is current, from the newest: 0 is the latest.
    pub(crate) current: usize,
    /// The pane the player scrolls is to be brought to the current one.
    pub(crate) go: bool,
}

/// Where a widget's finding is kept, and the count it said back.
const ASKED: &str = "find";
const FOUND: &str = "found";

/// Ask the widget whose id is `id` to find `finding`, from this frame on;
/// `None` stops.
pub(crate) fn ask(context: &egui::Context, id: Id, finding: Option<Finding>) {
    context.data_mut(|data| {
        if let Some(finding) = finding {
            data.insert_temp(id.with(ASKED), finding);
        } else {
            data.remove::<Finding>(id.with(ASKED));
            data.remove::<usize>(id.with(FOUND));
        }
    });
}

/// How many lines the widget whose id is `id` found when last drawn.
pub(crate) fn found(context: &egui::Context, id: Id) -> Option<usize> {
    context.data(|data| data.get_temp(id.with(FOUND)))
}

/// A widget's lines looked through this frame, as they are drawn.
pub(super) struct Seek {
    /// What is looked for, in lower case.
    query: String,
    /// The line found that is current, counted from the oldest.
    current: Option<usize>,
    go: bool,
    /// Lines found so far this frame.
    seen: usize,
}

impl Seek {
    /// What the widget `id` is to find this frame among `lines`, the text of
    /// each it will draw, in order; `None` when it is to find nothing. How
    /// many hold it is said back. `go` is taken: the pane is brought to the
    /// current line once.
    pub(super) fn of<'a>(
        ui: &egui::Ui,
        id: Id,
        lines: impl Iterator<Item = std::borrow::Cow<'a, str>>,
    ) -> Option<Self> {
        let finding = ui.data(|data| data.get_temp::<Finding>(id.with(ASKED)))?;
        let query = finding.query.to_lowercase();
        let found = if query.trim().is_empty() {
            0
        } else {
            lines.filter(|line| Self::holds(&query, line)).count()
        };
        ui.data_mut(|data| data.insert_temp(id.with(FOUND), found));
        if found == 0 {
            return None;
        }
        if finding.go {
            let taken = Finding {
                go: false,
                ..finding.clone()
            };
            ui.data_mut(|data| data.insert_temp(id.with(ASKED), taken));
        }
        let current = found
            .checked_sub(1)
            .map(|last| last.saturating_sub(finding.current.min(last)));
        Some(Self {
            query,
            current,
            go: finding.go,
            seen: 0,
        })
    }

    /// Whether `text`, a line's, holds what is looked for.
    pub(super) fn holds(query: &str, text: &str) -> bool {
        text.to_lowercase().contains(query)
    }

    /// A pane begins: its lines are counted afresh, each pane drawing all.
    pub(super) fn start(&mut self) {
        self.seen = 0;
    }

    /// A line drawn by `draw` into `ui`, whose text is `text`: marked when
    /// it holds what is looked for, the current one more, and the pane
    /// brought to it when `player` -- the pane the player scrolls -- and the
    /// bar moved.
    pub(super) fn line(
        &mut self,
        ui: &mut egui::Ui,
        (text, player): (&str, bool),
        draw: impl FnOnce(&mut egui::Ui),
    ) {
        let top = ui.cursor().top();
        draw(ui);
        if !Self::holds(&self.query, text) {
            return;
        }
        let rect = egui::Rect::from_x_y_ranges(ui.max_rect().x_range(), top..=ui.cursor().top());
        let current = self.current == Some(self.seen);
        let fill =
            theme::color(ui.ctx(), T::Accent).gamma_multiply(if current { 0.35 } else { 0.12 });
        ui.painter()
            .rect_filled(rect, theme::corner(ui.ctx()), fill);
        if current && self.go && player {
            let now = egui::style::ScrollAnimation::none();
            ui.scroll_to_rect_animation(rect, Some(egui::Align::Center), now);
        }
        self.seen += 1;
    }
}
