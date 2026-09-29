//! The characters' log windows (`plan/25` step 8): opened from a play
//! window's *Log* or a hub card's, each its own viewport, their reads handed
//! to [`Sessions::read_log`](crate::Sessions) off this thread.

use std::sync::Arc;

use super::{App, TITLE};
use crate::logs::Logs;
use crate::sessions::Seat;

impl App {
    /// Open `seat`'s log window, making it the first time.
    pub(super) fn open_log(&mut self, seat: &Arc<Seat>) {
        self.logs
            .entry(seat.id.0)
            .or_insert_with(|| Logs::new(&seat.name))
            .open = true;
    }

    /// Draw each open log window, and hand on what each asked to read. A
    /// window whose character left the table goes with it.
    pub(super) fn log_windows(&mut self, context: &egui::Context, seats: &[Arc<Seat>]) {
        self.logs
            .retain(|session, _| seats.iter().any(|seat| seat.id.0 == *session));
        for seat in seats {
            let Some(logs) = self.logs.get_mut(&seat.id.0).filter(|logs| logs.open) else {
                continue;
            };
            let mut closed = false;
            context.show_viewport_immediate(
                egui::ViewportId::from_hash_of(("log", seat.id.0)),
                egui::ViewportBuilder::default()
                    .with_title(format!("Log — {} — {TITLE}", seat.name))
                    .with_inner_size([1000.0, 640.0]),
                |ui, _class| {
                    closed |= ui.input(|input| input.viewport().close_requested());
                    logs.show(ui);
                },
            );
            if closed {
                logs.open = false;
            }
            // Taken after every pass has drawn: what any pass asked is kept
            // on the window until here (see `App::play`).
            for ask in logs.take_asks() {
                self.sessions.read_log(seat, ask, logs.inbox());
            }
        }
    }
}
