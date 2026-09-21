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
use cena_session::{SessionHandle, SessionObserver};
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
    pub(crate) hub: Mutex<Hub>,
    pub(crate) handle: SessionHandle,
    pub(crate) clients: Arc<Semaphore>,
    pub(crate) stop: CancellationToken,
}

/// A bound, authenticated, single-session viewer. Binding never logs in or
/// waits for the session actor. Start `run` alongside that actor.
pub struct WebServer {
    listener: TcpListener,
    observer: SessionObserver,
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
    /// Bind an ephemeral IPv4 loopback port and create a fresh 256-bit secret.
    ///
    /// # Errors
    /// Returns local bind/address errors or failure of OS random generation.
    pub async fn bind(observer: SessionObserver, handle: SessionHandle) -> io::Result<Self> {
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
            hub: Mutex::new(Hub::new()),
            handle,
            clients: Arc::new(Semaphore::new(MAX_CLIENTS)),
            stop: CancellationToken::new(),
        });
        Ok(Self {
            listener,
            observer,
            shared,
        })
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

    /// Serve until shutdown, without owning the runtime or session lifetime.
    /// All upgraded viewers and the projection task stop with this future.
    ///
    /// # Errors
    /// Returns a listener/server I/O failure, unavailable native observation,
    /// or a presentation that exceeds the bounded message size.
    pub async fn run(self, shutdown: impl Future<Output = ()> + Send + 'static) -> io::Result<()> {
        let stop = self.shared.stop.clone();
        let projection = pump(self.observer, Arc::clone(&self.shared));
        let server = axum::serve(BoundedListener::new(self.listener), router(self.shared))
            .with_graceful_shutdown({
                let stop = stop.clone();
                async move {
                    tokio::select! { () = shutdown => {}, () = stop.cancelled() => {} }
                    stop.cancel();
                }
            })
            .into_future();
        tokio::pin!(projection);
        let result = tokio::select! {
            result = server => result,
            result = &mut projection => result,
        };
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
