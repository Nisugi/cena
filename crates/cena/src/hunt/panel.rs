//! A character's hunt, shown in its play window's Hunt pane (`plan/47`
//! step 8): the hunt desk's reports, turn by turn, handed to the window as
//! the view it draws. The window cannot see `cena-behavior`, and the desk
//! cannot see `cena-ui`; the binary sees both, so the one line between them
//! is here.

use cena_behavior::hunt::{Desk, Status};
use cena_session::SessionId;
use cena_ui::HuntView;
use tokio::sync::watch;

/// Show `desk`'s runs, session `id`'s, in `window`'s Hunt pane, when there
/// is a window.
pub(super) fn show(desk: &Desk, id: SessionId, window: Option<&cena_gui::Sessions>) {
    if let Some(window) = window {
        tokio::spawn(forward(desk.reports(), id, window.clone()));
    }
}

/// Hand `reports`, session `id`'s hunt desk's, to `gui` as they come, until
/// the desk is gone.
async fn forward(
    mut reports: watch::Receiver<Option<Status>>,
    id: SessionId,
    gui: cena_gui::Sessions,
) {
    loop {
        let view = reports.borrow_and_update().as_ref().map(view);
        gui.hunt(id, view);
        if reports.changed().await.is_err() {
            return;
        }
    }
}

/// A hunt's report as the window draws it.
fn view(status: &Status) -> HuntView {
    HuntView {
        running: status.profile.clone(),
        phase: status.phase.clone(),
        doing: status.doing.clone(),
        target: status.target.clone(),
        waiting: status.waiting.clone(),
    }
}
