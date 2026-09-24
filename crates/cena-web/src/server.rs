//! One loopback listener, immutable bundled assets, and a secret held in memory.

use crate::listener::BoundedListener;
use crate::presentation::{Hub, pump};
use crate::socket;
use axum::Router;
use axum::extract::{Request, State, WebSocketUpgrade};
use axum::http::{HeaderMap, HeaderValue, StatusCode};
use axum::middleware::{self, Next};
use axum::response::{IntoResponse, Response};
use axum::routing::get;
use cena_session::{SessionHandle, SessionId, SessionObserver};
use std::collections::BTreeMap;
use std::fmt;
use std::future::{Future, IntoFuture};
use std::io;
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::net::TcpListener;
use tokio::sync::{Mutex, Semaphore};
use tokio_util::sync::CancellationToken;

pub(crate) const MAX_CLIENTS: usize = 8;
pub(crate) const MAX_MESSAGE_BYTES: usize = 16 * 1024;

pub(crate) struct Shared {
    pub(crate) token: String,
    pub(crate) authority: String,
    pub(crate) origin: String,
    csp: HeaderValue,
    /// Every session a viewer can attach to, by id (`plan/29` step 5). One
    /// listener serves them all (`plan/23` §D1a); each has its own hub.
    pub(crate) sessions: std::sync::Mutex<BTreeMap<SessionId, Arc<Viewed>>>,
    /// Signalled when a session is attached or detached, or publishes a new
    /// view: the hub page rebuilds its cards on it.
    pub(crate) changed: tokio::sync::broadcast::Sender<()>,
    /// Who answers the hub's add and remove requests; `None`, and the hub
    /// offers neither (`plan/29` step 5c).
    pub(crate) control: std::sync::Mutex<Option<HubControl>>,
    /// Characters the hub may add, as the control's owner last said.
    pub(crate) available: std::sync::Mutex<Vec<String>>,
    /// Every session's shared streams, merged for the hub (`plan/29` 5d).
    pub(crate) merged: Arc<crate::merged::MergedFeed>,
    pub(crate) clients: Arc<Semaphore>,
    pub(crate) stop: CancellationToken,
}

impl Shared {
    /// What a viewer that asked for `asked` is shown.
    pub(crate) fn choose(&self, asked: Asked) -> Choice {
        let sessions = self
            .sessions
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        match asked {
            Asked::Session(id) => sessions.get(&id).map_or(Choice::Missing, |viewed| {
                Choice::Session(Arc::clone(viewed))
            }),
            Asked::Only if sessions.len() == 1 => sessions
                .values()
                .next()
                .map_or(Choice::Hub, |viewed| Choice::Session(Arc::clone(viewed))),
            Asked::Only => Choice::Hub,
        }
    }

    /// Every session's card, in the order they were added.
    pub(crate) async fn cards(&self) -> Vec<cena_ui::SessionCard> {
        let sessions: Vec<(SessionId, Arc<Viewed>)> = self
            .sessions
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .iter()
            .map(|(id, viewed)| (*id, Arc::clone(viewed)))
            .collect();
        let mut cards = Vec::with_capacity(sessions.len());
        for (id, viewed) in sessions {
            let hub = viewed.hub.lock().await;
            cards.push(cena_ui::SessionCard::of(
                id.0.to_string(),
                viewed.name.clone(),
                hub.view(),
            ));
        }
        cards
    }
}

/// A request from the hub page, for whoever runs the sessions.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum HubRequest {
    /// Start this character: one the hub offered as available.
    Add(String),
    /// Quit this session and take it off the table.
    Remove(SessionId),
}

/// What answers the hub's requests: the owner of the session table, which
/// alone knows the roster and the keyring. It returns one line for the page
/// that asked. A closure, not a trait: there is one answerer.
pub type HubControl =
    Arc<dyn Fn(HubRequest) -> std::pin::Pin<Box<dyn Future<Output = String> + Send>> + Send + Sync>;

/// What an authenticated viewer is shown.
pub(crate) enum Choice {
    /// The session it named, or the only one.
    Session(Arc<Viewed>),
    /// It named none and there is not exactly one: every session's card.
    Hub,
    /// It named a session that is not served.
    Missing,
}

/// Which session an authenticated viewer is for.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Asked {
    /// It named none: the only session, when there is exactly one.
    Only,
    /// It named this one.
    Session(SessionId),
}

/// One session as the web frontend sees it: its presentation hub, the
/// handle its viewers' commands go through, and the stop for its pump.
pub(crate) struct Viewed {
    /// Which session this is.
    pub(crate) id: SessionId,
    /// The character's name, for its card on the hub page.
    pub(crate) name: String,
    pub(crate) hub: Mutex<Hub>,
    pub(crate) handle: SessionHandle,
    /// Cancelled when the session is detached, or the whole server stops.
    pub(crate) stop: CancellationToken,
    /// The server's hub-page signal, sent after each publish.
    pub(crate) changed: tokio::sync::broadcast::Sender<()>,
    /// Where this session's shared-stream lines are merged with the others'.
    pub(crate) merged: Arc<crate::merged::MergedFeed>,
}

impl Viewed {
    /// This session's tag on a merged line: the character's name, or its id
    /// when it was given none.
    pub(crate) fn tag(&self) -> String {
        if self.name.is_empty() {
            format!("Session {}", self.id.0)
        } else {
            self.name.clone()
        }
    }
}

#[cfg(test)]
impl Viewed {
    /// A session view with no session behind it, so a test can drive the
    /// presentation pump directly. The handle's inbox has no reader.
    pub(crate) fn for_test() -> Arc<Self> {
        let handle = SessionHandle::new(
            tokio::sync::mpsc::channel(1).0,
            cena_session::GenerationCell::default(),
            tokio::sync::broadcast::channel(1).0,
        );
        Arc::new(Self {
            id: handle.session(),
            name: String::new(),
            hub: Mutex::new(Hub::new()),
            handle,
            stop: CancellationToken::new(),
            changed: tokio::sync::broadcast::channel(1).0,
            merged: Arc::new(crate::merged::MergedFeed::new()),
        })
    }
}

/// Attach and detach sessions on a [`WebServer`]. Cloneable, and usable after
/// `run` has taken the server -- which is when a session table adds and
/// removes characters.
#[derive(Clone)]
pub struct Sessions {
    shared: Arc<Shared>,
}

impl fmt::Debug for Sessions {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Sessions").finish_non_exhaustive()
    }
}

impl Sessions {
    /// Serve `handle`'s session to viewers as `name` -- the character, for
    /// its card on the hub page -- and start its presentation pump. Replaces
    /// an earlier attachment of the same session.
    pub fn attach(
        &self,
        name: impl Into<String>,
        observer: SessionObserver,
        handle: SessionHandle,
    ) {
        let id = handle.session();
        let viewed = Arc::new(Viewed {
            id,
            name: name.into(),
            hub: Mutex::new(Hub::new()),
            handle,
            stop: self.shared.stop.child_token(),
            changed: self.shared.changed.clone(),
            merged: Arc::clone(&self.shared.merged),
        });
        let replaced = self
            .shared
            .sessions
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .insert(id, Arc::clone(&viewed));
        if let Some(old) = replaced {
            old.stop.cancel();
        }
        let _ = self.shared.changed.send(());
        // The pump's ending is this session's own: an owner gone ends this
        // pump, and its last view stays for any viewer still looking.
        tokio::spawn(pump(observer, viewed));
    }

    /// Answer the hub page's add and remove requests with `control`
    /// (`plan/29` step 5c). Until this is called the hub offers neither.
    pub fn control(&self, control: HubControl) {
        *self
            .shared
            .control
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner) = Some(control);
        let _ = self.shared.changed.send(());
    }

    /// The characters the hub may offer to add, replacing the last list.
    pub fn offer(&self, available: Vec<String>) {
        *self
            .shared
            .available
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner) = available;
        let _ = self.shared.changed.send(());
    }

    /// Stop serving session `id`. Its viewers are closed; the session itself
    /// is not touched -- the session table owns its lifetime.
    pub fn detach(&self, id: SessionId) {
        let removed = self
            .shared
            .sessions
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .remove(&id);
        if let Some(viewed) = removed {
            viewed.stop.cancel();
        }
        let _ = self.shared.changed.send(());
    }
}

/// A bound, authenticated viewer of one or more sessions. Binding never logs
/// in or waits for a session actor. Start `run` alongside the actors.
pub struct WebServer {
    listener: TcpListener,
    shared: Arc<Shared>,
}

impl fmt::Debug for WebServer {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("WebServer")
            .field("address", &self.shared.authority)
            .finish_non_exhaustive()
    }
}

impl WebServer {
    /// Bind for one session: [`Self::open`], then [`Sessions::attach`].
    ///
    /// # Errors
    /// As [`Self::open`].
    pub async fn bind(observer: SessionObserver, handle: SessionHandle) -> io::Result<Self> {
        let server = Self::open().await?;
        server.sessions().attach(String::new(), observer, handle);
        Ok(server)
    }

    /// Bind an ephemeral IPv4 loopback port and create a fresh 256-bit secret,
    /// serving no session until one is attached ([`Self::sessions`]).
    ///
    /// # Errors
    /// Returns local bind/address errors or failure of OS random generation.
    pub async fn open() -> io::Result<Self> {
        let listener = TcpListener::bind((std::net::Ipv4Addr::LOCALHOST, 0)).await?;
        let authority = listener.local_addr()?.to_string();
        let csp = HeaderValue::from_str(&format!(
            "default-src 'none'; script-src 'self'; style-src 'self'; connect-src 'self' ws://{authority}; img-src 'self'; base-uri 'none'; frame-ancestors 'none'; form-action 'none'"
        )).map_err(io::Error::other)?;
        let shared = Arc::new(Shared {
            token: fresh_token()?,
            origin: format!("http://{authority}"),
            authority,
            csp,
            sessions: std::sync::Mutex::new(BTreeMap::new()),
            changed: tokio::sync::broadcast::channel(1).0,
            control: std::sync::Mutex::new(None),
            available: std::sync::Mutex::new(Vec::new()),
            merged: Arc::new(crate::merged::MergedFeed::new()),
            clients: Arc::new(Semaphore::new(MAX_CLIENTS)),
            stop: CancellationToken::new(),
        });
        Ok(Self { listener, shared })
    }

    /// Attach and detach sessions, now or after `run` has taken the server.
    #[must_use]
    pub fn sessions(&self) -> Sessions {
        Sessions {
            shared: Arc::clone(&self.shared),
        }
    }

    /// Explicit pairing handoff. The fragment is never sent in an HTTP request.
    /// Treat the returned URL as a secret; it is deliberately absent from Debug.
    #[must_use]
    pub fn pairing_url(&self) -> String {
        format!("{}/#token={}", self.shared.origin, self.shared.token)
    }

    /// The actual loopback address, without authentication material.
    ///
    /// # Errors
    /// Returns an error if the listener address cannot be read.
    pub fn local_addr(&self) -> io::Result<SocketAddr> {
        self.listener.local_addr()
    }

    /// Serve until shutdown, without owning the runtime or any session's
    /// lifetime. All upgraded viewers and every presentation pump stop with
    /// this future.
    ///
    /// A session whose owner has gone ends only its own pump, and its last
    /// view stays up. It no longer ends the server, because one server serves
    /// every session (`plan/29` step 5). A busy or slow owner is retried, and
    /// an oversized presentation is degraded (see `presentation`).
    ///
    /// # Errors
    /// Returns a listener/server I/O failure.
    pub async fn run(self, shutdown: impl Future<Output = ()> + Send + 'static) -> io::Result<()> {
        let stop = self.shared.stop.clone();
        let result = axum::serve(BoundedListener::new(self.listener), router(self.shared))
            .with_graceful_shutdown({
                let stop = stop.clone();
                async move {
                    tokio::select! { () = shutdown => {}, () = stop.cancelled() => {} }
                    stop.cancel();
                }
            })
            .into_future()
            .await;
        stop.cancel();
        result
    }
}

fn fresh_token() -> io::Result<String> {
    use std::fmt::Write;
    let mut bytes = [0_u8; 32];
    getrandom::fill(&mut bytes).map_err(|error| io::Error::other(error.to_string()))?;
    let mut token = String::with_capacity(64);
    for byte in bytes {
        // Formatting into String cannot fail.
        let _ = write!(token, "{byte:02x}");
    }
    Ok(token)
}

pub(crate) fn router(shared: Arc<Shared>) -> Router {
    Router::new()
        .route(
            "/",
            get(|| async {
                asset(
                    "text/html; charset=utf-8",
                    include_str!("../assets/index.html"),
                )
            }),
        )
        .route(
            "/app.js",
            get(|| async {
                asset(
                    "text/javascript; charset=utf-8",
                    include_str!("../assets/app.js"),
                )
            }),
        )
        .route(
            "/session.js",
            get(|| async {
                asset(
                    "text/javascript; charset=utf-8",
                    include_str!("../assets/session.js"),
                )
            }),
        )
        .route(
            "/style.css",
            get(|| async {
                asset(
                    "text/css; charset=utf-8",
                    include_str!("../assets/style.css"),
                )
            }),
        )
        .route("/ws", get(upgrade))
        .layer(middleware::from_fn_with_state(Arc::clone(&shared), guard))
        .with_state(shared)
}

fn asset(content_type: &'static str, body: &'static str) -> impl IntoResponse {
    ([("content-type", content_type)], body)
}

fn single_header<'a>(headers: &'a HeaderMap, name: &str) -> Option<&'a str> {
    let mut values = headers.get_all(name).iter();
    let value = values.next()?.to_str().ok()?;
    values.next().is_none().then_some(value)
}

pub(crate) fn authorized_headers(
    headers: &HeaderMap,
    authority: &str,
    origin: &str,
    ws: bool,
) -> bool {
    if single_header(headers, "host") != Some(authority) {
        return false;
    }
    if headers.contains_key("origin") {
        single_header(headers, "origin") == Some(origin)
    } else {
        !ws
    }
}

async fn guard(State(shared): State<Arc<Shared>>, request: Request, next: Next) -> Response {
    let ws = request.uri().path() == "/ws";
    let allowed = request.uri().query().is_none()
        && authorized_headers(request.headers(), &shared.authority, &shared.origin, ws);
    let mut response = if allowed {
        next.run(request).await
    } else {
        StatusCode::FORBIDDEN.into_response()
    };
    let headers = response.headers_mut();
    headers.insert("cache-control", HeaderValue::from_static("no-store"));
    headers.insert("referrer-policy", HeaderValue::from_static("no-referrer"));
    headers.insert(
        "x-content-type-options",
        HeaderValue::from_static("nosniff"),
    );
    headers.insert("x-frame-options", HeaderValue::from_static("DENY"));
    headers.insert("content-security-policy", shared.csp.clone());
    response
}

async fn upgrade(State(shared): State<Arc<Shared>>, ws: WebSocketUpgrade) -> Response {
    let Ok(permit) = Arc::clone(&shared.clients).try_acquire_owned() else {
        return StatusCode::SERVICE_UNAVAILABLE.into_response();
    };
    ws.max_message_size(MAX_MESSAGE_BYTES)
        .max_frame_size(MAX_MESSAGE_BYTES)
        .max_write_buffer_size(1024 * 1024)
        .on_upgrade(move |websocket| socket::serve(websocket, shared, permit))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tokens_are_full_length_fresh_os_random_values() {
        let first = fresh_token().expect("OS randomness");
        assert_eq!(first.len(), 64);
        assert!(first.bytes().all(|byte| byte.is_ascii_hexdigit()));
        assert_ne!(first, fresh_token().expect("OS randomness"));
    }

    #[test]
    fn host_and_origin_require_exact_bound_address() {
        let mut headers = HeaderMap::new();
        headers.insert("host", HeaderValue::from_static("127.0.0.1:40123"));
        assert!(authorized_headers(
            &headers,
            "127.0.0.1:40123",
            "http://127.0.0.1:40123",
            false
        ));
        assert!(!authorized_headers(
            &headers,
            "127.0.0.1:40123",
            "http://127.0.0.1:40123",
            true
        ));
        for origin in [
            "null",
            "https://evil.example",
            "http://127.0.0.1:40124",
            "http://localhost:40123",
        ] {
            headers.insert("origin", HeaderValue::from_str(origin).expect("header"));
            assert!(!authorized_headers(
                &headers,
                "127.0.0.1:40123",
                "http://127.0.0.1:40123",
                true
            ));
        }
        headers.insert("origin", HeaderValue::from_static("http://127.0.0.1:40123"));
        assert!(authorized_headers(
            &headers,
            "127.0.0.1:40123",
            "http://127.0.0.1:40123",
            true
        ));
        headers.append("host", HeaderValue::from_static("evil.example"));
        assert!(!authorized_headers(
            &headers,
            "127.0.0.1:40123",
            "http://127.0.0.1:40123",
            true
        ));
    }
}
