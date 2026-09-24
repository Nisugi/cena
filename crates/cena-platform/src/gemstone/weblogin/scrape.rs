//! Reading the three answers out of play.net's HTML and URLs.
//!
//! Everything here is **pure**: a response's status, headers and body in, a
//! verdict out. That is the same seam [`reject`](crate::eaccess) draws, and it
//! is what lets the fragile half of this protocol be tested without a network
//! -- which matters more here than anywhere else in the login, because
//! `CLAUDE.md` forbids reaching a live service from this workspace and the
//! markup can change under us at any time.
//!
//! # The character scrape is the most fragile thing in this module
//!
//! Lich says so outright, and the reason is worth keeping in view: there is no
//! enumeration endpoint. Unlike eaccess's tab-delimited `C` response, the
//! character list is **server-rendered HTML**, so a markup change does not
//! error -- it silently matches nothing, which looks exactly like "no such
//! character". That is why the response status is checked **before** the body
//! is ever scraped: a transient 503 would otherwise be reported as a missing
//! character, and a missing character is fatal.

use super::failure::WebLoginFailure;
use super::instance::Instance;

/// Where a login POST's redirect landed.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LoginRedirect {
    /// Authenticated. The session cookie is set.
    Authenticated,
    /// The account or password was rejected.
    Rejected,
    /// Somewhere the confirmed protocol does not describe.
    Unexpected,
}

/// The path an account that has never set a security question is sent to.
///
/// **This is a success, not a failure**, and it is the kind of thing only a
/// live run finds. Lich confirmed it with a second test account: the session is
/// already fully authenticated at this point -- the account name renders in the
/// page banner and the real session cookie is set -- and navigating straight to
/// the play page works. Treating it as a failure would lock out every account
/// that has never set a security question.
const SECURITY_QA_PATH: &str = "/playdotnet/account/security_qa.asp";

/// Classify the `Location` of a login POST's redirect.
///
/// The **target path is the signal**; the body is never parsed for control
/// flow (`plan/10` §5.4, from Lich: *"compare the `Location` header's path
/// against the two page params you sent, don't parse body text for control
/// flow"*).
///
/// A query string is ignored, because the two confirmed failure shapes differ
/// only there: a wrong password gives `?error=&returnto=/dr/` and a nonexistent
/// account `?error=&returnto=/dr/play/home.asp`. Both mean the same thing.
#[must_use]
pub fn classify_login_redirect(location: Option<&str>, instance: &Instance) -> LoginRedirect {
    let Some(location) = location else {
        return LoginRedirect::Unexpected;
    };
    let path = location.split(['?', '#']).next().unwrap_or(location);

    if path == instance.error_page() {
        LoginRedirect::Rejected
    } else if path == instance.okay_page() || path == SECURITY_QA_PATH {
        LoginRedirect::Authenticated
    } else {
        LoginRedirect::Unexpected
    }
}

/// Whether a redirect away from the character list means "no subscription".
///
/// Matched by **shape, not by an exact string**. Lich confirmed live that the
/// path is instance-specific -- `DragonRealms` Prime's is plain
/// `subscription_needed.asp`, but Platinum's is `subscription_to_plat_needed.asp`
/// and Fallen's is `subscription_to_fall_needed.asp` -- so an exact match would
/// let an unseen variant fall through to a generic error and lose the real
/// cause.
///
/// No `GemStone` instance has been observed with a `_to_` variant. That is
/// exactly why the loose match is kept rather than tightened: assuming `GemStone`
/// will never have one is the guess this avoids making.
#[must_use]
pub fn is_subscription_needed(location: &str, instance: &Instance) -> bool {
    let path = location.split(['?', '#']).next().unwrap_or(location);
    let prefix = format!("/{}/play/subscription", instance.family);
    let Some(tail) = path.strip_prefix(prefix.as_str()) else {
        return false;
    };

    // A path separator in the variable middle would mean this is a DIFFERENT
    // page under a similar prefix, not an instance-specific variant.
    if tail.contains('/') {
        return false;
    }
    if tail == "_needed.asp" {
        return true;
    }
    tail.strip_prefix("_to_")
        .and_then(|rest| rest.strip_suffix("_needed.asp"))
        .is_some_and(|middle| !middle.is_empty())
}

/// Find a character's `charID` in a character-list page.
///
/// The markup, from Lich's live capture:
///
/// ```html
/// <input type=radio name="charID" id="W_TESTACCOUNT_000" value="W_TESTACCOUNT_000" checked  >
/// <label for="W_TESTACCOUNT_000"><span class="normS1">Raiyen</span></label><br>
/// ```
///
/// The id and the display name are in **two different elements**, tied by the
/// `for` attribute. Matching is case-insensitive on a trimmed name, as Lich's
/// `casecmp?` does.
///
/// Written as a scan rather than with a regex crate: the workspace has no regex
/// dependency, and `plan/05` §-1 says build the simplest thing that works. The
/// shape being matched is fixed and small.
#[must_use]
pub fn find_char_code(body: &str, character: &str) -> Option<String> {
    let wanted = character.trim();
    character_entries(body)
        .into_iter()
        .find(|(_, name)| name.trim().eq_ignore_ascii_case(wanted))
        .map(|(code, _)| code)
}

/// Every `(charID, display name)` pair the page declares.
///
/// Public so a diagnostic can report *which* characters were found when the one
/// asked for was not. That is the difference between "that character is not on
/// this instance" and "the markup changed and we matched nothing at all" --
/// precisely the failure Lich warns is otherwise silent.
#[must_use]
pub fn character_entries(body: &str) -> Vec<(String, String)> {
    let mut found = Vec::new();
    for tail in body.split("id=\"W_").skip(1) {
        let Some((code_rest, after)) = tail.split_once('"') else {
            continue;
        };
        if !code_rest
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'_')
        {
            continue;
        }
        let code = format!("W_{code_rest}");

        // The label for THIS code, not merely the next label on the page: an
        // input with no matching label must not borrow the following
        // character's name.
        let marker = format!("<label for=\"{code}\">");
        let Some(label) = after
            .find(marker.as_str())
            .map(|at| &after[at + marker.len()..])
        else {
            continue;
        };
        // Skip the opening `<span ...>` and take its text.
        let Some((_, span)) = label.split_once('>') else {
            continue;
        };
        if let Some((name, _)) = span.split_once('<') {
            found.push((code, name.to_owned()));
        }
    }
    found
}

/// The host, port and key from the final launch URL.
///
/// `Debug` is hand-written to redact the key; see the impl below.
#[derive(Clone, PartialEq, Eq)]
pub struct Launch {
    /// The game host to connect to.
    pub host: String,
    /// The game port.
    pub port: u16,
    /// The one-shot launch key.
    pub key: String,
}

impl std::fmt::Debug for Launch {
    /// Redacts the key, as [`LaunchPayload`](crate::eaccess::LaunchPayload)
    /// does: it is the live game credential.
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Launch")
            .field("host", &self.host)
            .field("port", &self.port)
            .field("key", &"<REDACTED>")
            .finish()
    }
}

/// Parse and **verify** the final launch URL.
///
/// # This is a security boundary, not a parse
///
/// By the time this runs the URL has come through a redirect chain. It decides
/// where the client opens a socket and hands over a launch key, so every field
/// is checked against the instance table rather than trusted:
///
/// - a **duplicate** `host`, `port` or `key` is refused outright. Last-value-
///   wins would otherwise let a repeated parameter override the real one --
///   Lich's `DUPLICATE_QUERY_PARAM`, and the subtlest of these checks.
/// - the host and port must **equal** the instance's pinned pair. Every
///   `GemStone` row is confirmed live, so there is no looser suffix path here;
///   Lich needs one only for its unverified `DragonRealms` rows.
/// - a blank value is a missing value.
///
/// # Errors
///
/// [`WebLoginFailure::UnexpectedResponse`] with a label naming which check
/// failed. **The URL itself is never carried**: it holds the launch key.
pub fn parse_launch(url: &str, instance: &Instance) -> Result<Launch, WebLoginFailure> {
    let query = url
        .split_once('?')
        .map(|(_, query)| query)
        .ok_or(WebLoginFailure::UnexpectedResponse("launch url: no query"))?;
    let query = query.split('#').next().unwrap_or(query);

    let mut host = None;
    let mut port = None;
    let mut key = None;
    for pair in query.split('&').filter(|pair| !pair.is_empty()) {
        let (name, value) = pair.split_once('=').unwrap_or((pair, ""));
        let slot = match name {
            "host" => &mut host,
            "port" => &mut port,
            "key" => &mut key,
            _ => continue,
        };
        if slot.is_some() {
            return Err(WebLoginFailure::UnexpectedResponse(
                "launch url: duplicate host, port or key",
            ));
        }
        *slot = Some(percent_decode(value));
    }

    let (Some(host), Some(port), Some(key)) = (host, port, key) else {
        return Err(WebLoginFailure::UnexpectedResponse(
            "launch url: missing host, port or key",
        ));
    };
    if host.is_empty() || port.is_empty() || key.is_empty() {
        return Err(WebLoginFailure::UnexpectedResponse(
            "launch url: blank host, port or key",
        ));
    }

    let port: u16 = port.parse().map_err(|_error| {
        WebLoginFailure::UnexpectedResponse("launch url: port is not a number")
    })?;

    if host != instance.expected_host || port != instance.expected_port {
        return Err(WebLoginFailure::UnexpectedResponse(
            "launch url: host or port is not this instance's pinned pair",
        ));
    }

    Ok(Launch { host, port, key })
}

/// Minimal `application/x-www-form-urlencoded` value decoding.
///
/// Only what a query value needs: `+` for a space, and `%XX`. A malformed
/// escape is left verbatim rather than dropped, so a value that fails to decode
/// still fails the pinned-host comparison rather than silently becoming
/// something else.
fn percent_decode(value: &str) -> String {
    let bytes = value.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut index = 0;
    while index < bytes.len() {
        match bytes[index] {
            b'+' => {
                out.push(b' ');
                index += 1;
            }
            b'%' if index + 2 < bytes.len() => {
                if let Ok(byte) = u8::from_str_radix(&value[index + 1..index + 3], 16) {
                    out.push(byte);
                    index += 3;
                } else {
                    out.push(b'%');
                    index += 1;
                }
            }
            byte => {
                out.push(byte);
                index += 1;
            }
        }
    }
    String::from_utf8_lossy(&out).into_owned()
}

/// The one host every hop of the selection chain may point at.
pub const TRUSTED_HOST: &str = "www.play.net";

/// The path the chain's final, absolute redirect must name. Lich's
/// `validate_final_url!` (`web_login.rb:483-487`) accepts nothing else.
const FINAL_PATH: &str = "/play/home.asp";

/// What to do with one `Location` from the character-selection chain.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Hop {
    /// The chain has ended: hand this to [`parse_launch`]. Carries the launch
    /// key, so it is never printed.
    Launch(String),
    /// Request this absolute URL next. Always `https://www.play.net/...`.
    Follow(String),
}

/// Decide where one redirect in the selection chain goes -- **and refuse any
/// that leaves play.net**.
///
/// # This is a security boundary, and it was not one
///
/// The loop in `http.rs` used to follow any `Location` that started with
/// `http`, verbatim. So a redirect to `http://anything/` -- a downgrade, an
/// off-site host, a MITM'd hop -- was fetched with the session's cookie jar,
/// and the chain it led to could end on a URL whose `host=`/`key=` then
/// reached [`parse_launch`] (review finding 3). `parse_launch` pins host and
/// port against the instance table, which kept the game socket safe; nothing
/// kept the REQUESTS safe.
///
/// Lich refuses it (`web_login.rb:430-431`, `:483-487`) and this ports its
/// rule:
///
/// - **An absolute `Location` ends the chain**, and must be exactly
///   `https://www.play.net/play/home.asp` -- scheme `https`, default port,
///   no userinfo, that path. Lich's reason for checking every absolute URL,
///   `http://` included, is the downgrade: a plain-http target must be
///   recognised and REJECTED, not requested.
/// - **A relative one is followed**, resolved against `https://www.play.net/`,
///   and the resolved URL is checked again. That catches the one relative
///   form that is not relative: `//evil.example/x` is a network-path
///   reference, and resolving it lands on another host. Lich sends it as a
///   literal path on its open connection, so it cannot leave play.net there;
///   Cena resolves, so it must re-check.
///
/// One Cena behaviour is kept that Lich does not have: a RELATIVE location
/// already carrying `host=` and `key=` is taken as the launch URL. It is
/// same-origin by construction and `parse_launch` still pins what it names.
///
/// # Errors
///
/// [`WebLoginFailure::UnexpectedResponse`] -- transient, as every "the server
/// did something the confirmed shape does not cover" is. The URL is never
/// carried: on the last hop it holds the launch key.
pub fn next_hop(location: &str) -> Result<Hop, WebLoginFailure> {
    const UNTRUSTED: WebLoginFailure =
        WebLoginFailure::UnexpectedResponse("character selection: untrusted redirect");

    let has_prefix = |prefix: &str| {
        location
            .get(..prefix.len())
            .is_some_and(|head| head.eq_ignore_ascii_case(prefix))
    };
    if has_prefix("https://") || has_prefix("http://") {
        let url = reqwest::Url::parse(location).map_err(|_| UNTRUSTED)?;
        return if is_play_net(&url) && url.path() == FINAL_PATH {
            Ok(Hop::Launch(location.to_owned()))
        } else {
            Err(UNTRUSTED)
        };
    }

    if location.contains("host=") && location.contains("key=") {
        return Ok(Hop::Launch(location.to_owned()));
    }

    let base = reqwest::Url::parse(&format!("https://{TRUSTED_HOST}/")).map_err(|_| UNTRUSTED)?;
    let next = base.join(location).map_err(|_| UNTRUSTED)?;
    if !is_play_net(&next) {
        return Err(UNTRUSTED);
    }
    Ok(Hop::Follow(next.into()))
}

/// Scheme `https`, host `www.play.net`, port 443, no userinfo -- the origin
/// half of Lich's `validate_final_url!`.
fn is_play_net(url: &reqwest::Url) -> bool {
    url.scheme() == "https"
        && url.host_str() == Some(TRUSTED_HOST)
        && url.port_or_known_default() == Some(443)
        && url.username().is_empty()
        && url.password().is_none()
}
