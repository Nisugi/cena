//! The half that touches the network.
//!
//! Split from [`scrape`](super::scrape) on the seam
//! [`eaccess`](crate::eaccess) already draws: everything decidable from bytes
//! is pure and tested, and what is left here is the request sequence. That
//! matters more in this module than anywhere else, because `CLAUDE.md` forbids
//! reaching a live service from this workspace -- so this file is the part no
//! test in this repository can execute, and it is kept as thin as the protocol
//! allows.
//!
//! # BUILT, NOT RUN
//!
//! No test here calls [`authenticate_via_web`], and none may. It is checked by
//! the author's eyes and by one live run with them present, exactly as
//! `plan/12` §7.2 criterion 1 was.

use std::time::Duration;

use super::failure::WebLoginFailure;
use super::instance::{Instance, instance_for};
use super::scrape::{
    Launch, LoginRedirect, classify_login_redirect, find_char_code, is_subscription_needed,
    parse_launch,
};

/// What a web login needs to know.
///
/// Mirrors [`Credentials`](crate::eaccess::Credentials) rather than inventing a
/// second shape, so the fallback is a swap at the call site.
#[derive(Clone, Copy)]
pub struct WebLoginRequest<'a> {
    /// The account name.
    pub account: &'a str,
    /// The account password. **Sent as a cleartext form field** -- see the
    /// module header; this is the protocol, not an oversight.
    pub password: &'a str,
    /// The character to launch.
    pub character: &'a str,
    /// The eaccess-style game code, e.g. `GS3`.
    pub game_code: &'a str,
}

impl std::fmt::Debug for WebLoginRequest<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("WebLoginRequest")
            .field("account", &"<REDACTED>")
            .field("password", &"<REDACTED>")
            .field("character", &self.character)
            .field("game_code", &self.game_code)
            .finish()
    }
}

/// The host every request in this flow goes to.
const BASE: &str = "https://www.play.net";

/// A browser-like `User-Agent`, **mandatory on every request**.
///
/// play.net's CloudFront/WAF returns a bare HTTP 500 for a default client UA --
/// Ruby's `Ruby/x.y.z` and `reqwest`'s own are both rejected (`plan/10` §5.4,
/// `web_login.rb:103`). Invisible from a browser capture, because a browser
/// always sends one. This is the first thing to suspect if the flow starts
/// answering 500 everywhere.
const USER_AGENT: &str = "Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 (KHTML, like Gecko) \
     Chrome/120.0.0.0 Safari/537.36";

/// Bounds the connect phase of each request.
///
/// Mirrors [`CONNECT_TIMEOUT`](crate::live) and exists for the same reason: a
/// dropped SYN with no RST otherwise hangs on the OS timeout. That is not
/// hypothetical here -- it is the exact shape of the 2026-09-08 outage that
/// makes this module necessary.
const CONNECT_TIMEOUT: Duration = Duration::from_secs(5);

/// Bounds each individual request end to end.
const REQUEST_TIMEOUT: Duration = Duration::from_secs(15);

/// Redirect hops to follow after the character selection.
///
/// The confirmed live chain is `goplay2.asp` -> `playing_web.asp` ->
/// `goplay_web.asp` -> the final URL. Lich allows 5, which leaves headroom
/// without permitting an unbounded loop; the same number is kept.
const MAX_REDIRECTS: usize = 5;

/// Log in through play.net's web flow and resolve a character launch.
///
/// # Cancellation
///
/// Every await here is `reqwest`'s, which is genuinely async -- so a caller may
/// drop this future and the in-flight request is cancelled with it. That is
/// `plan/12` §5.5's requirement and the reason `reqwest` was chosen over
/// `ureq`: the supervisor races `connect()` against its cancel token, and a
/// blocking client inside that future would make the whole connect
/// uncancellable. See `cena-platform/Cargo.toml`.
///
/// # Errors
///
/// [`WebLoginFailure`], whose [`is_fatal`](WebLoginFailure::is_fatal) says
/// whether anything else is worth trying.
pub async fn authenticate_via_web(request: WebLoginRequest<'_>) -> Result<Launch, WebLoginFailure> {
    let instance = instance_for(request.game_code).ok_or(WebLoginFailure::UnsupportedGameCode)?;

    // `cookie_store` carries the ASP session cookie between these requests,
    // which is the whole reason the sequence works. `redirect::Policy::none()`
    // is equally load-bearing and for the opposite reason: this protocol reads
    // the redirect TARGET as its signal, so following redirects automatically
    // would consume the very thing being measured.
    let client = reqwest::Client::builder()
        .cookie_store(true)
        .redirect(reqwest::redirect::Policy::none())
        .user_agent(USER_AGENT)
        .connect_timeout(CONNECT_TIMEOUT)
        .timeout(REQUEST_TIMEOUT)
        .build()
        .map_err(|error| WebLoginFailure::Transport(error.to_string()))?;

    login(&client, &request, instance).await?;
    let char_code = resolve_char_code(&client, &request, instance).await?;
    select_character(&client, &char_code, instance).await
}

/// Step 1: establish a session and authenticate.
async fn login(
    client: &reqwest::Client,
    request: &WebLoginRequest<'_>,
    instance: &Instance,
) -> Result<(), WebLoginFailure> {
    // **This GET is not optional.** Posting `login.asp` cold -- with no session
    // cookie from the sign-in page -- returns a bare 500. A browser always
    // visits the sign-in page first, so the dependency is invisible in a
    // browser capture (`plan/10` §5.4, discovery 2).
    let sign_in = format!("{BASE}/{}/signin_needed.asp", instance.family);
    client
        .get(&sign_in)
        .send()
        .await
        .map_err(transport("sign-in page"))?;

    let okay_page = instance.okay_page();
    let error_page = instance.error_page();
    let form = [
        ("return_okay_page", okay_page.as_str()),
        ("return_error_page", error_page.as_str()),
        ("remember_account", ""),
        ("remember_password", ""),
        ("account_name", request.account),
        ("account_password", request.password),
        ("submit", "Login"),
    ];

    let response = client
        .post(format!("{BASE}/includes/common/login/login.asp"))
        .form(&form)
        .send()
        .await
        .map_err(transport("login"))?;

    match classify_login_redirect(location_of(&response).as_deref(), instance) {
        LoginRedirect::Authenticated => Ok(()),
        LoginRedirect::Rejected => Err(WebLoginFailure::LoginRejected),
        LoginRedirect::Unexpected => Err(WebLoginFailure::UnexpectedResponse("login redirect")),
    }
}

/// Step 1a: scrape this instance's character list for the `charID`.
async fn resolve_char_code(
    client: &reqwest::Client,
    request: &WebLoginRequest<'_>,
    instance: &Instance,
) -> Result<String, WebLoginFailure> {
    let response = client
        .get(format!("{BASE}{}", instance.character_list_path))
        .send()
        .await
        .map_err(transport("character list"))?;

    let status = response.status();

    // **The status is checked before the body is touched, and that ordering is
    // the point.** A non-200 body scrapes to no match, which is
    // indistinguishable from "no such character" -- and that is FATAL, so a
    // transient 503 would be silently discarded as a permanent failure.
    if status.is_redirection() {
        // A second redirect the first one never hinted at: login itself
        // succeeded and landed on `okay_page`, but GETting that page redirects
        // again when the account has no subscription here. A client that does
        // not follow redirects -- ours, deliberately -- otherwise falls through
        // to an empty scrape and reports the wrong cause entirely.
        let location = location_of(&response).unwrap_or_default();
        if is_subscription_needed(&location, instance) {
            return Err(WebLoginFailure::NoSubscription);
        }
        return Err(WebLoginFailure::UnexpectedResponse(
            "character list: unexpected redirect",
        ));
    }
    if !status.is_success() {
        return Err(WebLoginFailure::UnexpectedResponse(
            "character list: not 200",
        ));
    }

    let body = response.text().await.map_err(transport("character list"))?;
    find_char_code(&body, request.character).ok_or(WebLoginFailure::CharacterNotFound)
}

/// Steps 2-4: select the character and follow the chain to the launch URL.
async fn select_character(
    client: &reqwest::Client,
    char_code: &str,
    instance: &Instance,
) -> Result<Launch, WebLoginFailure> {
    let form = [
        ("charID", char_code),
        ("NEWCHARSUB", "TRUE"),
        ("managesub", "0"),
        ("gameName", instance.family),
        ("instanceID", "0"),
        // The WEB game code, which for Prime is not the eaccess one.
        ("game", instance.web_game_code),
        ("frontend", "web"),
    ];

    let mut response = client
        .post(format!("{BASE}/includes/common/play/goplay2.asp"))
        .form(&form)
        .send()
        .await
        .map_err(transport("character selection"))?;

    // Followed by hand, bounded, because the chain's FINAL url is the payload.
    // An automatic follower would fetch the web client page and discard the
    // very URL being sought.
    for _hop in 0..MAX_REDIRECTS {
        let Some(location) = location_of(&response) else {
            return Err(WebLoginFailure::UnexpectedResponse(
                "character selection: chain ended without a launch url",
            ));
        };

        // The launch URL is recognised by carrying the three parameters, not by
        // its path: the path has changed before and the parameters are what is
        // actually needed.
        if location.contains("host=") && location.contains("key=") {
            return parse_launch(&location, instance);
        }

        let next = if location.starts_with("http") {
            location
        } else {
            format!("{BASE}{location}")
        };
        response = client
            .get(&next)
            .send()
            .await
            .map_err(transport("character selection"))?;
    }

    Err(WebLoginFailure::UnexpectedResponse(
        "character selection: too many redirects",
    ))
}

/// The `Location` header as a string, if the response carries a usable one.
fn location_of(response: &reqwest::Response) -> Option<String> {
    response
        .headers()
        .get(reqwest::header::LOCATION)?
        .to_str()
        .ok()
        .map(std::borrow::ToOwned::to_owned)
}

/// Turn a transport error into a failure that names the step, not the URL.
///
/// The URL would carry the launch key on the last hop, and `reqwest`'s own
/// `Display` includes it. The step name is what a reader actually needs.
fn transport(step: &'static str) -> impl Fn(reqwest::Error) -> WebLoginFailure {
    move |error| {
        let kind = if error.is_timeout() {
            "timed out"
        } else if error.is_connect() {
            "could not connect"
        } else if error.is_body() || error.is_decode() {
            "malformed response body"
        } else {
            "request failed"
        };
        WebLoginFailure::Transport(format!("{step}: {kind}"))
    }
}
