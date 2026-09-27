//! Hydra's GUI (`plan/47`): egui, the primary frontend.
//!
//! It sits beside `cena-web`, never above it: it reads what a session knows
//! and draws it, and asks the binary to act. A trigger's look was laid in the
//! session before any of this sees the text (`plan/45` §0), so nothing here
//! colours anything itself.
//!
//! Built in the order `plan/47` §5 gives. So far: the [`Hub`], drawn from the
//! same [`SessionCard`](cena_ui::SessionCard)s the web hub sends.

mod hub;

pub use hub::{Hub, Tab};
