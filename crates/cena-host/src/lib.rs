//! cena-host
//!
//! The table of sessions one Hydra runs: many heads, one body (`plan/29`
//! step 3).
//!
//! A session (`cena-session`) is one character, and knows nothing of any
//! other. This crate is what knows there are several: it adds and removes
//! them while Hydra runs (author, 2026-09-23), gives each its own
//! [`SessionId`](cena_session::SessionId), refuses a second session on an
//! account that already has one, and stops them all in order.
//!
//! # Why a crate
//!
//! The table has three callers -- the command line, the web hub (`plan/29`
//! step 5) and the GUI launcher the author named as the intended entry
//! point -- and `cena-web` cannot depend on the binary. Putting it in
//! `cena-session` was the alternative, and was declined: that crate's job is
//! one character, and a table of them is a different job.
//!
//! # Built to sit behind a shared lock
//!
//! Frontends will share one table. [`Host::take`] removes a session from it
//! at once, and the slow part -- `quit`, then waiting for the game to close
//! -- is [`Hosted::stop`], awaited with the lock released. So no frontend
//! waits on another session's goodbye.

mod table;

pub use table::{AddError, Host, Hosted, QUIT_TIMEOUT, Who, stop_all};
