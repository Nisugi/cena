//! The HTTPS web-login fallback: a second way in when eaccess is down.
//!
//! # Why this exists
//!
//! **It is the outage path.** `plan/10`'s incident timeline records 2026-09-08:
//! `nc -vz eaccess.play.net 7910` exited 124 -- the SYN silently dropped, no
//! RST -- while *"website auth and Play-in-Browser stayed up throughout"*. The
//! author put it plainly (2026-09-19): they have been *"having issues with the
//! normal login, and the web one seems to stay up during these times allowing
//! access to the game."*
//!
//! So this is not a nicety. When it matters, it is the only way in.
//!
//! # An entirely different mechanism
//!
//! [`eaccess`](crate::eaccess) is tab-delimited records over a raw TLS socket
//! with a client-side XOR-hashed password. This is ordinary HTTPS: ASP form
//! POSTs, a session cookie, and a redirect chain whose **target path** is the
//! success/failure signal. It ends at the same place -- a game host, a port and
//! a one-shot key.
//!
//! Three consequences worth stating, because each one inverts something the
//! eaccess module establishes:
//!
//! | | eaccess | here |
//! |---|---|---|
//! | the password | XOR-hashed against a server nonce | **sent in the clear** as a form field |
//! | TLS | 1.2, static-RSA, self-signed, three documented weakenings | 1.3, valid chain, ordinary verification |
//! | the character list | the `C` command, tab-delimited | **scraped out of HTML** |
//!
//! The password point is not a flaw to fix: play.net's form takes a cleartext
//! field and security rests entirely on TLS. It is recorded so nobody ports
//! eaccess's hashing here expecting it to apply, and so the cert verification
//! is never relaxed the way [`live`](crate::live) deliberately relaxes it for
//! eaccess -- **VERIFIED 2026-09-19**: `www.play.net` negotiates TLS 1.3 with
//! a valid chain, so none of those weakenings have a reason to travel.
//!
//! # The knowledge here was paid for live, and cannot be re-derived offline
//!
//! Six behaviours in this flow were invisible in a browser capture and only
//! surfaced by building a standalone client and testing several accounts
//! (`lich-5/docs/web-login-protocol-analysis.md`, "Implementation Status").
//! They are ported deliberately, each at the place it bites:
//!
//! 1. a browser-like **`User-Agent` is mandatory on every request**, including
//!    the first GET -- play.net's WAF returns a bare 500 otherwise ([`http`]).
//! 2. the login POST needs a **session cookie from a prior GET** of the sign-in
//!    page; posting cold also returns a bare 500 ([`http`]).
//! 3. an account with no security question is redirected **to a different page
//!    on success** ([`scrape`]).
//! 4. a character can exist on one instance and not appear on its family's
//!    generic page at all ([`instance`]).
//! 5. an account without a subscription gets a **second** redirect that a
//!    non-following client never sees ([`scrape`]).
//! 6. that redirect's path is **instance-specific**, so it is matched by shape
//!    ([`scrape`]).
//!
//! `CLAUDE.md` forbids logging into a live service from this workspace, so none
//! of these can be re-observed here. Every one is therefore a **test against a
//! recorded fixture**, and the fixtures are the record.
//!
//! # Layout
//!
//! [`instance`] is the table: which page to scrape and which host the answer
//! must name. [`scrape`] is the **pure** half -- responses in, verdicts out --
//! and carries every test. [`failure`] is what went wrong and whether anything
//! else should be tried. [`http`] is the half that touches the network, and is
//! the only part a live run can check.

pub mod failure;
pub mod http;
pub mod instance;
pub mod scrape;

#[cfg(test)]
#[path = "weblogin/scrape_tests.rs"]
mod scrape_tests;

// Only what crosses this module's boundary. The rest -- the redirect
// classifier, the scraper, the instance lookup -- is reached by path from
// `http` and from the tests, and re-exporting it here would be a second name
// for one item (`plan/05` Rule 3.3).
pub use failure::WebLoginFailure;
pub use http::{WebLoginRequest, authenticate_via_web};
pub use scrape::Launch;
