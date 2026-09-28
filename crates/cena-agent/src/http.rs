//! The HTTP side of the agent's listener (`plan/35` §2): `/mcp` behind a
//! bearer token, `/health` open, on loopback.
//!
//! Moved down out of `server.rs` when step 4's `command` and `text` took that
//! file past its cap (`plan/05` Rule 4.1: move code down, do not raise the
//! cap). `server.rs` is the tools; this is how a client reaches them.
//!
//! The token is a header, **never a tool argument** (LAB's rule, `plan/35`
//! §2): a model that saw it in a tool call could repeat it. `/health` needs no
//! token and says only what this is and which contract it speaks, so a client
//! can refuse a listener it does not know.

use std::sync::Arc;

use axum::Router;
use axum::extract::Request;
use axum::http::{StatusCode, header};
use axum::middleware::Next;
use axum::response::{IntoResponse, Response};
use axum::routing::get;
use rmcp::transport::streamable_http_server::session::local::LocalSessionManager;
use rmcp::transport::streamable_http_server::{StreamableHttpServerConfig, StreamableHttpService};
use tokio_util::sync::CancellationToken;

use crate::PROTOCOL;
use crate::characters::Characters;
use crate::server::Agent;

/// The HTTP side: `/mcp` behind the token, `/health` open.
pub fn router(characters: Characters, token: String, stop: &CancellationToken) -> Router {
    let agent = Agent::new(characters);
    let service = StreamableHttpService::new(
        move || Ok(agent.clone()),
        Arc::new(LocalSessionManager::default()),
        // A plain reply for a plain request; a stream still when one is
        // needed, which is how the push `plan/46` §4.1 wants arrives.
        StreamableHttpServerConfig::default()
            .with_json_response(true)
            .with_cancellation_token(stop.child_token()),
    );
    let token = Arc::new(token);
    let guarded = Router::new()
        .nest_service("/mcp", service)
        .layer(axum::middleware::from_fn(
            move |request: Request, next: Next| {
                let token = Arc::clone(&token);
                async move { require_token(&token, request, next).await }
            },
        ));
    Router::new().route("/health", get(health)).merge(guarded)
}

/// Serve on `listener` until `stop`.
///
/// # Errors
///
/// The listener fails.
pub async fn serve(
    listener: tokio::net::TcpListener,
    characters: Characters,
    token: String,
    stop: CancellationToken,
) -> std::io::Result<()> {
    let app = router(characters, token, &stop);
    axum::serve(listener, app)
        .with_graceful_shutdown(stop.cancelled_owned())
        .await
}

async fn health() -> impl IntoResponse {
    axum::Json(serde_json::json!({
        "hydra": env!("CARGO_PKG_VERSION"),
        "protocol": PROTOCOL,
    }))
}

async fn require_token(token: &str, request: Request, next: Next) -> Response {
    let offered = request
        .headers()
        .get(header::AUTHORIZATION)
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.strip_prefix("Bearer "));
    if offered.is_some_and(|offered| same(offered.as_bytes(), token.as_bytes())) {
        next.run(request).await
    } else {
        (StatusCode::UNAUTHORIZED, "a bearer token is required").into_response()
    }
}

/// Compared in constant time, as Despana compares its token
/// (`crates/cena-web/src/socket.rs`).
pub(crate) fn same(a: &[u8], b: &[u8]) -> bool {
    a.len() == b.len()
        && a.iter()
            .zip(b)
            .fold(0_u8, |difference, (a, b)| difference | (a ^ b))
            == 0
}

const HEX: &[u8; 16] = b"0123456789abcdef";

/// A new bearer token: 32 random bytes, hex.
///
/// # Errors
///
/// The operating system gave no randomness.
pub fn new_token() -> Result<String, String> {
    let mut bytes = [0_u8; 32];
    getrandom::fill(&mut bytes).map_err(|e| e.to_string())?;
    Ok(bytes.iter().fold(String::with_capacity(64), |mut hex, b| {
        hex.push(char::from(HEX[usize::from(b >> 4)]));
        hex.push(char::from(HEX[usize::from(b & 0xf)]));
        hex
    }))
}
