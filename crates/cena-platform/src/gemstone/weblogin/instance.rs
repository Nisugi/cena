//! Which page to scrape, and which host the answer must name.
//!
//! The web flow has **its own game-code table**, and it is not the `eaccess` one.
//! `plan/10` §5.4 and Lich's `CONFIRMED_INSTANCES`
//! (`lib/common/authentication/web_login.rb:143-151`) record the divergence:
//! `GemStone` Prime is `GS3` over eaccess and **`GS4`** here. Everything else
//! observed so far passes through unchanged, which is exactly why the one
//! exception has to be a table rather than an assumption.
//!
//! # Two things the table pins, for two different reasons
//!
//! 1. **`character_list_path`** -- the page whose HTML carries the character
//!    radio inputs. This is *not* derivable from the family: Lich confirmed
//!    live that a Shattered-only character appears on `/gs4/play/playf.asp` and
//!    **not at all** on `GemStone` Prime's `/gs4/play/home.asp`. Scraping the
//!    family's generic page would report "no such character" for a character
//!    that exists.
//! 2. **`expected_host` / `expected_port`** -- what the final redirect must
//!    name. The launch URL is attacker-visible input by the time we parse it,
//!    and it decides where the client opens a socket and hands over a key.
//!    Pinning turns "trust the redirect" into "verify the redirect".
//!
//! # `GemStone` only, deliberately
//!
//! Lich's table also carries `DR`, `DRT`, `DRX` and `DRF`. Those are **not**
//! ported: `CLAUDE.md` defers `DragonRealms` all-or-nothing, and an unreachable
//! DR row here would be the first piece of the structure that deferral avoids.
//! The one DR row Cena does carry is in
//! [`endpoint`](crate::gemstone::endpoint), where it is one row of a table
//! ported whole rather than a standalone entry -- see that module's header.
//!
//! `GSX` (`GemStone` Platinum) is absent because the instance is **retired**
//! (Lich's note at `web_login.rb:236`, confirmed by the account holder).

/// One `GemStone` instance, as the web flow sees it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Instance {
    /// The eaccess-style code the caller asks for, e.g. `GS3`.
    pub game_code: &'static str,
    /// The code the **web** flow wants in its form post. Differs for Prime.
    pub web_game_code: &'static str,
    /// The game family, lowercase -- the path prefix for the login pages.
    pub family: &'static str,
    /// The page whose HTML carries this instance's character list.
    pub character_list_path: &'static str,
    /// The host the final redirect must name, exactly.
    pub expected_host: &'static str,
    /// The port the final redirect must name, exactly.
    pub expected_port: u16,
}

/// The `GemStone` instances confirmed live by Lich, ported verbatim.
///
/// Every row here is one Lich exercised end-to-end against the real servers
/// (`docs/web-login-protocol-analysis.md`, "Implementation Status"). Lich's
/// unverified rows carry `expected_host: nil`; **none of those are `GemStone`**,
/// so every row below is pinned and Cena needs no unverified-host path at all.
const INSTANCES: &[Instance] = &[
    Instance {
        game_code: "GS3",
        // **The divergence.** Prime is `GS3` over eaccess and `GS4` here.
        // Lich records that `GS3` is not accepted by the web layer.
        web_game_code: "GS4",
        family: "gs4",
        character_list_path: "/gs4/play/home.asp",
        expected_host: "storm.gs4.game.play.net",
        expected_port: 10024,
    },
    Instance {
        game_code: "GST",
        web_game_code: "GST",
        family: "gs4",
        character_list_path: "/gs4/play/play_test.asp",
        expected_host: "chimera.simutronics.com",
        expected_port: 10624,
    },
    Instance {
        game_code: "GSF",
        web_game_code: "GSF",
        family: "gs4",
        character_list_path: "/gs4/play/playf.asp",
        expected_host: "storm.gs4.game.play.net",
        expected_port: 10324,
    },
];

/// The instance for an eaccess-style game code.
///
/// `None` means the web flow has no confirmed route for this code, which is a
/// **refusal, not a guess**: Lich's `instance_for` raises
/// `UNSUPPORTED_GAME_CODE` rather than assuming passthrough, on the grounds
/// that the web layer's codes are not always the eaccess ones. Since the one
/// known divergence is silent -- `GS3` is simply not accepted -- guessing would
/// fail at the far end with a worse error.
#[must_use]
pub fn instance_for(game_code: &str) -> Option<&'static Instance> {
    INSTANCES.iter().find(|i| i.game_code == game_code)
}

impl Instance {
    /// The page a successful login redirects to.
    #[must_use]
    pub fn okay_page(&self) -> String {
        format!("/{}/play/home.asp", self.family)
    }

    /// The page a failed login redirects to.
    ///
    /// **The redirect target is the pass/fail signal**, which is why this is
    /// built rather than matched loosely. `plan/10` §5.4, from Lich: *"compare
    /// the `Location` header's path against the two page params you sent, don't
    /// parse body text for control flow."*
    #[must_use]
    pub fn error_page(&self) -> String {
        format!("/{}/login_error.asp", self.family)
    }
}
