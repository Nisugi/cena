//! A character's log window: the player log read back (`plan/25` step 8).
//!
//! The author, 2026-09-29, showing Lichborne's viewer: *"I notice there is
//! no gui window in the plan"*. One window per character, its own viewport as
//! the settings menu is, opened from the play window's top bar and the hub
//! card. Three tabs: *Recent* (a day, whole), *Search* and *Export*, over
//! `cena_session::player_log`'s reader, archive and export.
//!
//! # Reading is off the window's thread
//!
//! Every read is asked of [`Sessions::read_log`](crate::Sessions), which asks
//! the writer to flush first and reads on a blocking task (`plan/25` §5); the
//! answer lands in the window's inbox and wakes it. This file holds what the
//! window shows and what it asks, and runs a read ([`run`]) where it is told
//! to; `draw.rs` draws it.
//!
//! # Filters, presets and *Dedup* are a reading view
//!
//! A line is shown when its **class** is ticked: the last part of its tag,
//! so `main/combat` is `combat` and `main` is `main` (`plan/25` step 2b: the
//! class is what our definitions made of the line). The presets tick a set
//! of classes, and *Dedup* folds a run of the same line into one with its
//! count. None of it changes what is read or counted (§1, the DR-speech
//! pitfall): an export writes every line of the ticked classes.

mod draw;
mod read;
mod view;
mod window;

pub(crate) use read::{Ask, Inbox, Reply, run};
pub(crate) use view::{Preset, Shown, class, shown};
pub(crate) use window::{Logs, Tab, open_folder};

#[cfg(test)]
mod tests;
