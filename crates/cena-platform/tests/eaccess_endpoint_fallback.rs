//! **PL-1: the host/port table, as a fallback rather than a rewrite.**
//!
//! `plan/10` §7.2 calls `Lich.fix_game_host_port` MANDATORY and says *"Cena
//! MUST implement both directions"*. `game.rs` connected to `payload.gamehost`
//! as given, and the omission was not recorded anywhere.
//!
//! The author's decision (2026-09-19) was the retry, not the rewrite: *"I know
//! lich recently added some fall back connection ways, we should have those
//! fallbacks as well but other than that I don't think we need anything else
//! legacy"*. `eaccess/gemstone/endpoint.rs` records why that is the safer half
//! -- a pre-emptive rewrite overrides the server's own answer, a fallback can
//! only help.
//!
//! # Why this file is nearly empty, and where the rest went
//!
//! `cena-arch-tests`' Rule 3.4 scan flags game-name literals in integration
//! tests as well as in `src`, so **every assertion that names an endpoint lives
//! in `eaccess/gemstone/endpoint.rs`'s unit tests**, under the path the scan
//! exempts. That covers both mapping directions, the port-not-host match, and
//! the exact-match guard.
//!
//! What is left here is the one property that can be stated without naming an
//! endpoint at all -- and it is worth keeping separate, because it is the
//! property a CALLER depends on rather than one the table's rows have.
//!
//! An earlier draft tried to keep the involution check here by inventing
//! hostnames to probe with. It passed while matching **nothing**: a test that
//! cannot fail (`plan/19` pattern D, the review's A4/PL-6). The property needs a
//! real row as a seed, so it belongs with the rows.
//!
//! # These tests do not connect to anything
//!
//! `other_spelling` is pure. `CLAUDE.md` forbids reaching a live game service
//! from this workspace, and the retry arm in `connect_game` is exercised by the
//! author's eyes, once -- the same split `eaccess/mod.rs` draws between the
//! pure half and the socket half.

use cena_platform::eaccess::other_spelling;

#[test]
fn an_unknown_endpoint_has_no_counterpart_and_that_is_not_an_error() {
    // The COMMON case, and the one that keeps this safe as endpoints change.
    //
    // `connect_with_fallback` reads `None` as "nothing else to try" and returns
    // the original error, so this is the behaviour that makes a stale table
    // harmless: if every spelling in it is retired, the fallback silently never
    // fires. It cannot send a player somewhere stale, because it only ever runs
    // after the server's own answer already failed to connect.
    assert_eq!(other_spelling("example.invalid", 10324), None);
    assert_eq!(other_spelling("example.invalid", 0), None);
    assert_eq!(other_spelling("", 0), None);
}
