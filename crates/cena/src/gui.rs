//! The windowed run (`plan/47` step 2): the hub on the main thread, the
//! session table on the runtime's workers.
//!
//! What the binary does with no `--headless` or `--web`: no arguments, or a
//! double-click, opens the hub (the author: *"launching the binary with no
//! arguments or double clicking it, will launch our gui hub"*). A character
//! named with `--character` is settled at the terminal first, as a headless
//! run settles it, and started when the window opens.
//!
//! The run ends one of three ways, all through `play::serve`'s one orderly
//! path: the window closes, Ctrl-C, or (step 3) the hub's Shut down. Every
//! character quits, then the window is closed if it is still open.

use crate::{interrupt, play};

/// Open the hub, with `names` started, and run until it closes.
///
/// # Errors
///
/// A named character's login could not be settled, the window could not
/// open, or every login was refused.
pub(crate) fn run(
    runtime: &tokio::runtime::Runtime,
    names: &[String],
) -> Result<(), Box<dyn std::error::Error>> {
    let logins = play::settle(names)?;
    let sessions = cena_gui::Sessions::new(runtime.handle().clone());
    // The Ctrl-C listener is a task, so it is installed inside the runtime.
    let interrupt = runtime.block_on(async { interrupt::on_ctrl_c() });
    let table = runtime.spawn(Box::pin(play::serve(
        logins,
        Some(sessions.clone()),
        interrupt.clone(),
    )));
    let shown = cena_gui::run(sessions);
    // However the window closed, every character quits now.
    interrupt.cancel();
    let served = runtime.block_on(table);
    shown.map_err(|error| format!("the window could not open: {error}"))?;
    served.map_err(|error| format!("the session table failed: {error}"))??;
    Ok(())
}
