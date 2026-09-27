//! Hydra's GUI (`plan/47`): egui, the primary frontend.
//!
//! It sits beside `cena-web`, never above it: it reads what a session knows
//! and draws it, and asks the binary to act. A trigger's look was laid in the
//! session before any of this sees the text (`plan/45` §0), so nothing here
//! colours anything itself.
//!
//! Built in the order `plan/47` §5 gives. So far: the [`App`], a window the
//! binary opens and [`run`]s on the main thread, showing the [`Hub`] over the
//! [`Sessions`] the binary attached, each followed by its own feed.

mod app;
mod feed;
mod hub;
mod sessions;

pub use app::{App, TITLE, run};
pub use hub::{Hub, Tab};
pub use sessions::Sessions;
