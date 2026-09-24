//! The legacy/modern game-host table, used **only** as a retry.
//!
//! Split out of [`game`](super::game) on the same seam as
//! [`reject`](super::reject): this is pure -- a host and a port in, a host and
//! a port out -- and the connect around it needs a socket.
//!
//! # What this is, and what it deliberately is not
//!
//! Simutronics has changed game endpoints under Lich before, and Lich carries
//! a four-row translation table in both directions:
//!
//! | Legacy | Modern |
//! |---|---|
//! | `gs-plat.simutronics.net:10121` | `storm.gs4.game.play.net:10124` |
//! | `gs3.simutronics.net:4900` | `storm.gs4.game.play.net:10024` |
//! | `gs4.simutronics.net:10321` | `storm.gs4.game.play.net:10324` |
//! | `prime.dr.game.play.net:4901` | `dr.simutronics.net:11024` |
//!
//! Lich applies `fix_game_host_port` (legacy -> modern) **pre-emptively**, then
//! on a connect failure applies `break_game_host_port` (the exact inverse) and
//! retries once (`lib/main/main.rb:581-598`).
//!
//! **Cena implements the retry and not the pre-emptive rewrite** (author's
//! decision, 2026-09-19: *"I know lich recently added some fall back connection
//! ways, we should have those fallbacks as well but other than that I don't
//! think we need anything else legacy"*).
//!
//! This is a deviation from `plan/10` §7.2, which calls the rewrite
//! "MANDATORY". Recorded rather than silently skipped, with the reasoning:
//!
//! - A pre-emptive rewrite **overrides the server**. If `L` names a host, that
//!   is the server's answer to "where do I connect", and rewriting it means
//!   believing a hard-coded table over the live endpoint. When Simutronics
//!   retires a spelling, the table is wrong and the rewrite makes it
//!   authoritative.
//! - As a **fallback**, the same table can only ever help: it fires when the
//!   server's own answer did not connect, and if the legacy hosts are retired
//!   it simply never fires.
//!
//! # Why the mapping is applied in BOTH directions here
//!
//! Lich knows which direction it needs because it rewrote the pair itself, so
//! its retry is the exact inverse of the rewrite it just did. Cena does not
//! rewrite, so it does not know which spelling `L` returned.
//!
//! Whether STORM still returns legacy hostnames is **UNVERIFIED** -- the
//! 2026-09-18 tcpdump filtered on `host eaccess.play.net` (`plan/10:1771`) and
//! so never captured the game connect, and the `L` response recorded at
//! `plan/10:1741` has its `GAMEHOST` redacted. That is the open question PL-1
//! called *"a question for the author, not the corpus"*.
//!
//! **A symmetric table does not need the answer.** Each pair has exactly one
//! counterpart whichever side it arrives on, so the fallback is correct for a
//! legacy host, a modern host, and an unknown host (no counterpart, no retry).

/// The instance a login asks for when the player names none: `GST`, the
/// `GemStone` IV test instance, so an unconsidered login lands somewhere a
/// mistake costs nothing.
///
/// Here rather than as a literal at the prompt because an instance code is
/// game-specific data, which Rule 3.4 keeps under this namespace -- the
/// hardened `game_names_outside_game_modules_are_flagged` found the literal
/// in `cena/src/ask.rs`.
pub const DEFAULT_GAME_CODE: &str = "GST";

/// One endpoint pair: the two spellings of the same game server.
type Pair = (&'static str, u16);

/// The table, verbatim from Lich (`lib/lich.rb:737-768`, both directions).
///
/// Ported rather than derived: these are observed server endpoints, which
/// `plan/13` §4a says to port. Expect to update it -- this table is itself the
/// clearest evidence that endpoints change (`plan/10:1189-1191`).
const ENDPOINT_PAIRS: &[(Pair, Pair)] = &[
    (
        ("gs-plat.simutronics.net", 10121),
        ("storm.gs4.game.play.net", 10124),
    ),
    (
        ("gs3.simutronics.net", 4900),
        ("storm.gs4.game.play.net", 10024),
    ),
    (
        ("gs4.simutronics.net", 10321),
        ("storm.gs4.game.play.net", 10324),
    ),
    (
        ("prime.dr.game.play.net", 4901),
        ("dr.simutronics.net", 11024),
    ),
];

/// The other spelling of this endpoint, if the table knows one.
///
/// `None` means "no counterpart", which is the common case and is **not** an
/// error: it is what an endpoint outside the table looks like, and the caller's
/// answer to it is to stop rather than to retry.
///
/// Matching is on the **host and the port together**. `storm.gs4.game.play.net`
/// appears three times with three different ports, so a host-only match would
/// be ambiguous; the port is what distinguishes Prime from Platinum from GS3.
#[must_use]
pub fn other_spelling(host: &str, port: u16) -> Option<(&'static str, u16)> {
    ENDPOINT_PAIRS.iter().find_map(|&(legacy, modern)| {
        if (host, port) == legacy {
            Some(modern)
        } else if (host, port) == modern {
            Some(legacy)
        } else {
            None
        }
    })
}

#[cfg(test)]
// Why these live here and not in `tests/eaccess_endpoint_fallback.rs`: every
// assertion below has to SPELL a game hostname, and `cena-arch-tests`' Rule 3.4
// scan covers integration tests too. Under `src/gemstone/` they are exempt by
// path -- the same reason the table itself is here. The behavioural tests that
// need no hostname stayed in the integration test.
mod tests {
    use super::{ENDPOINT_PAIRS, other_spelling};

    #[test]
    fn every_pair_maps_in_both_directions() {
        // Lich keeps these as two functions -- `fix_game_host_port` and
        // `break_game_host_port` -- and Cena needs both, because it does not
        // rewrite pre-emptively and so cannot know which spelling `L` returned.
        for &(legacy, modern) in ENDPOINT_PAIRS {
            assert_eq!(
                other_spelling(legacy.0, legacy.1),
                Some(modern),
                "{legacy:?} did not map to its modern spelling"
            );
            assert_eq!(
                other_spelling(modern.0, modern.1),
                Some(legacy),
                "{modern:?} did not map back to its legacy spelling"
            );
        }
    }

    #[test]
    fn the_port_distinguishes_three_rows_that_share_a_host() {
        // **Why matching is on the pair, not the host.** One modern hostname is
        // the spelling of THREE different servers, and only the port tells them
        // apart. A host-only match would send a Platinum player to the GS3
        // fallback.
        let shared: Vec<_> = ENDPOINT_PAIRS
            .iter()
            .filter(|(_, modern)| modern.0 == ENDPOINT_PAIRS[0].1.0)
            .collect();
        assert_eq!(
            shared.len(),
            3,
            "the table's shape changed; re-read this test"
        );

        for &&(legacy, modern) in &shared {
            assert_eq!(
                other_spelling(modern.0, modern.1),
                Some(legacy),
                "{modern:?} resolved to the wrong row -- the port was ignored"
            );
        }
    }

    #[test]
    fn the_mapping_is_an_involution() {
        // Applying it twice returns the original. This is the property that
        // makes ONE symmetric table safe to use without knowing which side you
        // started on: no row may have a counterpart whose own counterpart is a
        // third thing. `connect_with_fallback` relies on it -- it looks up a
        // counterpart without ever asking which direction it just went.
        for &(legacy, modern) in ENDPOINT_PAIRS {
            for (host, port) in [legacy, modern] {
                let (back_host, back_port) =
                    other_spelling(host, port).expect("a table row must have a counterpart");
                assert_eq!(
                    other_spelling(back_host, back_port),
                    Some((host, port)),
                    "{host}:{port} did not round-trip"
                );
            }
        }
    }

    #[test]
    fn a_known_host_on_an_unknown_port_has_no_counterpart() {
        // The pair must match whole. A right host with a wrong port is not a
        // row, and guessing a counterpart would retry against a server the
        // table has no evidence about.
        let (host, port) = ENDPOINT_PAIRS[0].1;
        assert_eq!(other_spelling(host, port.wrapping_add(1)), None);
    }

    #[test]
    fn the_host_match_is_exact_not_a_prefix_or_suffix() {
        // `plan/10:1184-1186` records a verifier correction on exactly this:
        // Lich's DragonRealms guard matches the full hostname, not a prefix. A
        // loose match would also let an attacker-chosen hostname that merely
        // ENDS with a table entry map to a real endpoint.
        let (host, port) = ENDPOINT_PAIRS[0].0;
        assert_eq!(other_spelling(&format!("evil-{host}"), port), None);
        assert_eq!(other_spelling(&format!("{host}.evil.example"), port), None);
        assert_eq!(
            other_spelling(host.split('.').next().expect("a dotted host"), port),
            None
        );
    }
}
