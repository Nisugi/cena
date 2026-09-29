//! What the keys' actions do in a play window (`plan/52` steps 3 and 4):
//! the command input's, and the window in use's. Moved out of `play.rs` at
//! its cap.

use super::{Play, draw};

impl Play {
    /// The window in use, by its placed id: the widget last clicked in,
    /// while it is here; the story until one is.
    pub(crate) fn in_use(&self) -> Option<u32> {
        let placed = self.layout.as_ref()?.placed();
        self.in_use
            .filter(|id| placed.iter().any(|placed| placed.id == *id))
            .or_else(|| {
                placed
                    .iter()
                    .find(|placed| placed.widget.is_story())
                    .map(|placed| placed.id)
            })
    }

    /// Whether the command input has the keyboard, or nothing does, with
    /// nothing open that a key would close or move in first: the object's
    /// menu, a widget's, the list of widgets to add, a drop-down or a menu
    /// of the bar. What an action on the input waits for
    /// ([`Action::on_input`](crate::keys::Action::on_input)).
    pub(crate) fn typing(&self, context: &egui::Context) -> bool {
        let focused = context.memory(egui::Memory::focused);
        focused.is_none_or(|id| id == self.input_id())
            && self.asking.is_none()
            && self.menu.is_none()
            && self.adding.is_none()
            && !egui::Popup::is_any_open(context)
    }

    /// Do `action`, one of those on the command input or sending from it
    /// (`plan/52` step 3), or on the window in use (step 4); the line to
    /// send, if it sends one.
    pub(crate) fn act(
        &mut self,
        context: &egui::Context,
        action: crate::keys::Action,
    ) -> Option<String> {
        use crate::keys::Action;
        use crate::widget::Scroll;
        let scroll = |scroll| {
            if let Some(placed) = self.in_use() {
                crate::widget::ask_scroll(context, draw::widget_id(self.session, placed), scroll);
            }
            None
        };
        let typed = |back: usize| {
            self.history
                .len()
                .checked_sub(back)
                .and_then(|at| self.history.get(at).cloned())
        };
        match action {
            Action::SendOrRepeat if self.input.trim().is_empty() => typed(1),
            Action::SendOrRepeat => self.enter(),
            Action::RepeatLast => typed(1),
            Action::RepeatSecondLast => typed(2),
            Action::HistoryBack => {
                self.walk(true);
                None
            }
            Action::HistoryForward => {
                self.walk(false);
                None
            }
            Action::ClearInput => {
                self.input.clear();
                self.back = None;
                None
            }
            Action::ScrollPageUp => scroll(Scroll::PageUp),
            Action::ScrollPageDown => scroll(Scroll::PageDown),
            Action::ScrollLineUp => scroll(Scroll::LineUp),
            Action::ScrollLineDown => scroll(Scroll::LineDown),
            Action::ScrollTop => scroll(Scroll::Top),
            Action::ScrollBottom => scroll(Scroll::Bottom),
            Action::NextWindow => self.next_window(true),
            Action::PreviousWindow => self.next_window(false),
            Action::NextTab => self.turn_tab(true),
            Action::PreviousTab => self.turn_tab(false),
            Action::NextUnreadTab => self.unread_tab(),
            Action::Stop | Action::Settings | Action::Set(_) => None,
        }
    }

    /// The window after the one in use made the one in use, or the one
    /// before (`forward` or not), round the windows showing (step 5).
    fn next_window(&mut self, forward: bool) -> Option<String> {
        let showing = self.layout.as_ref()?.showing();
        let at = self
            .in_use()
            .and_then(|id| showing.iter().position(|shown| *shown == id))
            .unwrap_or(0);
        let count = showing.len();
        let next = if forward {
            (at + 1) % count.max(1)
        } else {
            (at + count - 1) % count.max(1)
        };
        self.in_use = showing.get(next).copied();
        None
    }

    /// The next tab of the window in use's stack shown, or the one before;
    /// it is the window in use now.
    fn turn_tab(&mut self, forward: bool) -> Option<String> {
        let in_use = self.in_use()?;
        let shown = self.layout.as_mut()?.turn_tab(in_use, forward)?;
        self.in_use = Some(shown);
        self.save();
        None
    }

    /// The first tab with lines unread shown, and in use.
    fn unread_tab(&mut self) -> Option<String> {
        let tab = *self.unread_tabs.first()?;
        if self.layout.as_mut()?.show_tab(tab) {
            self.in_use = Some(tab);
            self.save();
        }
        None
    }
}
