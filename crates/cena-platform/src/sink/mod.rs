//! Where a session's **wire** traffic goes to live on disk.
//!
//! # This is one of two logs, and it is the lower one
//!
//! Author's call, 2026-09-18: *"ideally logging will have a few options right.
//! We'll have this dev log of the wire, then we would want a user log of the
//! processed text. We only need the dev now but I'm sure that matters."*
//!
//! | | this file | the user log, later |
//! |---|---|---|
//! | content | raw bytes, both directions, chunk boundaries | display text as a player reads it |
//! | audience | us: debugging, and cutting fixtures | the author: "what happened in that hunt" |
//! | layer | `cena-platform`, **below** the parser | needs `Frame`s, so **above** it |
//! | lifetime | churn freely | kept; searched years later |
//!
//! **The user log cannot live in this crate.** Processed text does not exist
//! until `cena-protocol` has parsed it, and `cena-platform` is below
//! `cena-protocol` (`plan/12:74-76`). So it is not a later retrofit of this
//! type -- it is a different type at a different layer, and [`SessionSink`] is
//! named for the wire rather than for logging in general so that it does not
//! squat on the name the other one will want.
//!
//! [`Recorder`](crate::record::Recorder) holds one session's wire bytes in
//! memory and is what criterion 7 replays. It is **pure** -- no clock, no file
//! handle, `seq` seeded at 0 -- and that purity is what makes a replay of a
//! replay byte-identical. This module is the other half: the part that writes,
//! kept separate so the recorder stays deterministic.
//!
//! # Why the recorder does not own a file
//!
//! Giving `Recorder` a `File` would put I/O, a path, and a failure mode inside
//! the type every replay test constructs. The tests would then either touch
//! the filesystem or carry a null-writer branch that production never takes.
//! A sink the session flushes to costs one field and leaves
//! `crates/cena-session/tests/replay_determinism.rs` untouched.
//!
//! # What lands on disk
//!
//! Two files per session, beside each other:
//!
//! | File | Holds | For |
//! |---|---|---|
//! | `<char>-<stamp>.bytes` | the raw wire, with chunk boundaries | replay, and cutting fixtures |
//! | `<char>-<stamp>.log` | structured lines: commands, lifecycle, errors | reading back what happened |
//!
//! The bytes file is the one that matters most. `plan/12` criterion 7 replays
//! a recording, and until now the only recordings were synthetic. A session
//! captured here is the wire **as Cena's own parser saw it**, which is where
//! future fixtures should come from -- today's were hand-written or lifted
//! from a Lich archive whose format is a different thing entirely
//! (`CLAUDE.md`: "Only `.xml` is wire data").
//!
//! # This is a PRIVATE development log
//!
//! Credentials are scrubbed exactly (see [`Redactions`]): the account name,
//! the author's real name and the session key are known before or during the
//! handshake, so removing them is exact rather than heuristic.
//!
//! **Other players' names are NOT scrubbed, and this file does not pretend
//! otherwise.** `cena_protocol::scrub` deliberately refuses to guess which
//! capitalised word is a person -- "`Inochi` and `Rawknuckle's` are the same
//! shape, one is a player and one is a tavern" -- and a live session has
//! nobody to name them in advance. So a session log is safe from the
//! *credential* angle and is still **not automatically shareable**. Cutting a
//! fixture out of one goes through `Scrubber` with the names named, exactly as
//! it does today.
//!
//! Author's call, 2026-09-18: a private dev log, matching the 49.55 GB archive
//! it sits beside.

mod config;
mod writer;

pub use config::{
    BYTES_TIMESTAMP_ENV, DEFAULT_LOG_DIR, LOG_DIR_ENV, ROTATE_AFTER_LINES, ROTATE_ENV, TIME_FORMAT,
    bytes_timestamps_enabled, log_dir, rotate_after_lines,
};
pub use writer::{Redactions, SessionSink};
