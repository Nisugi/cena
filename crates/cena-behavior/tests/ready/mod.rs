//! Wait for a session to be `Ready` before a behavior starts.
//!
//! A module rather than a test target (`tests/<name>/mod.rs` is not one).
//!
//! `run` holds `Syncing` until the first prompt after `<endSetup/>`
//! (`cena-session`'s `actor/readiness.rs`), and a behavior's command before
//! then is refused. A real behavior starts on a session somebody is already
//! playing; these harnesses start theirs the moment the actor is spawned, so
//! they wait for the same thing a player would have.
//!
//! A facade (`plan/05` Rule 4.4): the wait is in `wait.rs`.

mod wait;

pub use wait::until_ready;
