//! Batches: `;multi` and `;foreach`, commands sent for the player in order
//! (`plan/30` §4 and §7, M6e; the author, Q6: *"we do ;multi not chain, and
//! ;foreach yes"*).
//!
//! Two Lich scripts, ported as one behavior because they are one job at two
//! sizes: multi.lic repeats a list (`reference/scripts/scripts/multi.lic`,
//! 65 lines), and foreach.lic runs a list on each item that matches
//! (`reference/scripts/scripts/foreach.lic`, 2,192 lines). `VellumFE` calls
//! its own foreach "batch item commands" (`src/core/app_core/commands.rs:2500`),
//! which is the word used here.
//!
//! | Piece | Module | What it is |
//! |---|---|---|
//! | the words | [`parse`], [`Command`] | `;multi ...` or `;foreach ...` |
//! | `;multi` | [`multi`] | a comma list, so many times |
//! | `;foreach` | [`foreach`] | its options, filter, targets and commands |
//! | the items | [`pick`] | read off the model, filtered and ordered |
//! | one item's lines | [`build`] | the commands with `item` filled in |
//! | the lines | [`Line`] | what a batch does, one at a time |
//! | the driver | `drive`, `scan` | settle, send, wait; look in each target |
//! | the desk | [`Desk`] | one batch of a kind at a time, per session |
//!
//! Everything but the driver and the desk is pure and tested without a game.
//! **A Hydra command in a batch** (`;sc 401`) is run by the binary, which is
//! the only place that knows every command ([`Hydra`]): the batch gives the
//! authority back while it runs, and waits until what it started is over.

pub mod build;
mod command;
mod desk;
mod drive;
pub mod foreach;
mod line;
pub mod multi;
pub mod pick;
mod scan;

pub use command::{Command, Job, Kind, parse};
pub use desk::{Desk, EVERY};
pub use drive::{BEAT, Halt, REFUSED_CAP, RESENDS, SEND_DEADLINE, SETTLE_CAP};
pub use line::{Hydra, Line, Pool, Ran};
