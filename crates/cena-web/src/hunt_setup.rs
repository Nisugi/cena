//! Authenticated, explicitly installed native configuration. The callback has
//! no command-sending interface; neither Save nor Inspect starts a behavior.

use crate::server::Shared;
use axum::{
    Json,
    extract::{DefaultBodyLimit, State},
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, Response},
};
use cena_session::SessionId;
use serde::Deserialize;
use std::{
    collections::BTreeMap,
    future::Future,
    pin::Pin,
    sync::{Arc, Mutex},
};

/// Host-owned configuration handler, bound to a single native session or an
/// explicitly offline fixture. JSON is transport only; native Hunt validates it.
pub type HuntSetup = Arc<
    dyn Fn(
            serde_json::Value,
        ) -> Pin<Box<dyn Future<Output = Result<serde_json::Value, String>> + Send>>
        + Send
        + Sync,
>;
pub(crate) type Handlers = Mutex<BTreeMap<SessionId, HuntSetup>>;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Request {
    session: String,
    message: serde_json::Value,
}

pub(crate) fn limit() -> DefaultBodyLimit {
    DefaultBodyLimit::max(256 * 1024)
}

pub(crate) async fn configure(
    State(shared): State<Arc<Shared>>,
    headers: HeaderMap,
    Json(request): Json<Request>,
) -> Response {
    let token = headers
        .get("authorization")
        .and_then(|h| h.to_str().ok())
        .and_then(|h| h.strip_prefix("Bearer "));
    let origin = headers.get("origin").and_then(|h| h.to_str().ok());
    if token != Some(shared.token.as_str()) || origin != Some(shared.origin.as_str()) {
        return (
            StatusCode::FORBIDDEN,
            "Pairing and same-origin authorization required",
        )
            .into_response();
    }
    let Ok(session) = request.session.parse::<u32>() else {
        return (StatusCode::BAD_REQUEST, "Invalid session").into_response();
    };
    let handler = shared
        .hunt_setup
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .get(&SessionId(session))
        .cloned();
    let Some(handler) = handler else {
        return (
            StatusCode::NOT_FOUND,
            "Native setup is not enabled for this session",
        )
            .into_response();
    };
    match handler(request.message).await {
        Ok(reply) => Json(reply).into_response(),
        Err(error) => (StatusCode::BAD_REQUEST, error).into_response(),
    }
}
