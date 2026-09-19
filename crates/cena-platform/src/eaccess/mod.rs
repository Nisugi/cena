//! The `EAccess` login handshake: account and password in, game host and key out.
//!
//! `plan/10-eaccess-spec.md` is the protocol; this is the port of it, and of
//! the spike that proved it (`spike/eaccess-spike/src/main.rs`). The sequence
//! is `K A M F G P C L` over one TLS connection, and its product is a
//! [`LaunchPayload`] -- a plain TCP host, a port, and a one-shot key.
//!
//! # Why this is in `cena-platform`
//!
//! **AMENDED 2026-09-18 (author's call), Milestone 1 Step 2.** `plan/12` §2's
//! crate table assigns `EAccess` to no crate; `plan/12` §7.1 puts "`EAccess` login
//! (`10`, incl. the spike)" *In* for Milestone 1, so a home had to be chosen.
//!
//! It is here because **nothing in this module knows what a `Frame` is**,
//! which is exactly the line `crate`'s own header draws around this layer.
//! `EAccess` speaks tab-delimited fields over TLS and stops the moment the game
//! socket opens; it never sees game markup. [`LiveSource::connect_tls`] was
//! already here for its transport, and `native-tls` was already a dependency
//! of this crate for the same reason.
//!
//! The cost is recorded rather than hidden: the `cena` binary gains a direct
//! `cena-platform` edge, four layers below it, which
//! `crates/cena-arch-tests/tests/layering.rs` calls out as the shortcut the
//! graph exists to prevent. Taken deliberately, with the alternative
//! (`cena-session::login`, no edge change) considered and declined: it would
//! put a protocol `cena-session` does not otherwise speak inside the session,
//! and make the login unrunnable without first constructing one.
//!
//! # Rewritten async, not ported line for line
//!
//! The spike is blocking `std::net` with `set_read_timeout`
//! (`spike/eaccess-spike/src/main.rs:23-24`). `plan/12` §5.5 requires every
//! wait to be cancellable and a timeout is not a cancellation, so the I/O is
//! rewritten on [`LiveSource`]. What is ported **verbatim** is the knowledge:
//! every refusal below was paid for in debugging on 2026-09-18, and each one
//! turns a failure that points somewhere else into a failure that names its
//! own cause.
//!
//! # BUILT, NOT RUN
//!
//! `CLAUDE.md` forbids logging into a live game service without the author
//! present, and `plan/12` §7.2 criterion 1 is the one criterion Milestone 1
//! does not exercise in this workspace. **No test here calls
//! [`authenticate`]**, and none may.
//!
//! # Layout
//!
//! [`wire`] is the **pure** half: the types that cross the wire and the
//! functions that read its fields. Every one is covered by
//! `tests/eaccess_wire.rs`, because a live login is a terrible place to find
//! an off-by-one and `CLAUDE.md` forbids running one to find out.
//! [`handshake`] is the half that touches a socket, and is checked by the
//! author's eyes, once.

mod game;
mod handshake;
mod refusal;
mod reject;
mod wire;

pub use crate::gemstone::endpoint::other_spelling;
pub use game::connect_game;
pub use handshake::authenticate;
pub use refusal::{describe_launch_refusal, launch_refusal_is_fatal};
pub use reject::{Rejection, classify_a_rejection};
pub use wire::{
    CLIENT_BANNER, Credentials, EaccessError, LaunchPayload, expect_echo, hash_password,
    offered_game_codes, parse_launch, redact, redact_char_code, resolve_char_code,
    trim_ascii_whitespace,
};
