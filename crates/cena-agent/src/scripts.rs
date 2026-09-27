//! Scripts (`plan/46`, M7b): a **script runner**, a program outside Hydra
//! that runs one character's scripts in their own language -- Ruby first,
//! on Lich's own engine -- reaches its character through MCP here.
//!
//! **Its own listener, apart from the agent's.** The agent's has a fixed
//! port and a kept token because an MCP client outside Hydra is configured
//! once (`plan/35` §2). A runner is Hydra's own child: whoever starts one
//! admits it here, which makes it a token kept only in memory, and gives it
//! that token and this listener's address in its environment. The port is
//! whatever the system gives, on loopback.
//!
//! **A runner is not an agent.** It has no level and no denylist: a script
//! is the player's own program, which the player started, and does what a
//! Lich script does (`cena_session::script`). It acts on its own character
//! only, through the session's script door; this crate holds that door and
//! never the handle, as it holds the agent's.
//!
//! - [`listening`]: what a runner listens to, at its own positions.
//! - [`local`]: its local copy of the character, and the map it is placed on.
//! - `watch`: what the session publishes, told to a runner in Lich's order.
//! - [`tools`]: `listen`, `send`, `say`, `room` and `spell`, the contract
//!   in `SCRIPTS.md`.
//! - [`runner`]: the Ruby runner's files, carried in the binary, and how one
//!   is started.

use std::collections::BTreeMap;
use std::sync::{Arc, Mutex, PoisonError};

use axum::Router;
use axum::extract::Request;
use axum::http::{StatusCode, header};
use axum::middleware::Next;
use axum::response::{IntoResponse, Response};
use cena_session::SessionObserver;
use cena_session::script::Door;
use rmcp::transport::streamable_http_server::session::local::LocalSessionManager;
use rmcp::transport::streamable_http_server::{StreamableHttpServerConfig, StreamableHttpService};
use tokio_util::sync::CancellationToken;

pub mod listening;
pub mod local;
pub mod runner;
pub mod tools;
mod watch;

use listening::Listening;
use local::Atlas;

/// The contract a runner speaks (`SCRIPTS.md`), versioned as the agent's is.
pub const PROTOCOL: &str = "hydra-script/1";

/// The runners this Hydra has started, by the token each was given.
/// Cloned freely: one table behind it.
#[derive(Clone, Debug, Default)]
pub struct Runners {
    seats: Arc<Mutex<BTreeMap<String, Arc<Seat>>>>,
    /// The map runners place their characters on, when Hydra has one.
    atlas: Option<Arc<Atlas>>,
}

/// One runner's character: what it listens to, and how it acts.
#[derive(Debug)]
pub struct Seat {
    /// The character it runs scripts for.
    pub character: String,
    door: Door,
    listening: Arc<Listening>,
    atlas: Option<Arc<Atlas>>,
    /// Read for what is evaluated for the character on asking (`spell`).
    observer: SessionObserver,
    stop: CancellationToken,
}

impl Runners {
    /// Runners whose characters are placed on `atlas`'s map: `map_room` in
    /// the local copy, and `room` answered.
    #[must_use]
    pub fn with_atlas(atlas: Atlas) -> Self {
        Self {
            atlas: Some(Arc::new(atlas)),
            ..Self::default()
        }
    }

    /// Admit a runner for `character`: its token, and from the moment this
    /// returns each line the game sends kept for it. Called inside a Tokio
    /// runtime.
    ///
    /// # Errors
    ///
    /// The operating system gave no randomness for a token, or the session
    /// is gone.
    pub async fn admit(
        &self,
        character: &str,
        door: Door,
        observer: &SessionObserver,
    ) -> Result<String, String> {
        let token = crate::new_token()?;
        let stop = CancellationToken::new();
        // On before subscribing, so every line after the subscription is
        // published as heard.
        door.listen(true);
        let Some((snapshot, events)) = crate::characters::subscribe(observer, &stop).await else {
            return Err(format!("{character} has no session to listen to"));
        };
        let seat = Arc::new(Seat {
            character: character.to_owned(),
            door,
            listening: Arc::default(),
            atlas: self.atlas.clone(),
            observer: observer.clone(),
            stop,
        });
        let watching = watch::Watching {
            character: character.to_owned(),
            observer: observer.clone(),
            atlas: self.atlas.clone(),
            listening: Arc::clone(&seat.listening),
            stop: seat.stop.clone(),
        };
        tokio::spawn(watch::watch(watching, snapshot, events));
        self.lock().insert(token.clone(), seat);
        Ok(token)
    }

    /// The runner with `token` has stopped: nothing more is kept for it, and
    /// its token opens nothing.
    pub fn dismiss(&self, token: &str) {
        let removed = self.lock().remove(token);
        if let Some(seat) = removed {
            seat.stop.cancel();
            seat.listening.close();
            if !self
                .lock()
                .values()
                .any(|other| other.character == seat.character)
            {
                seat.door.listen(false);
            }
        }
    }

    /// The player typed `line` (without the command symbol) for the runner
    /// with `token`. `false` when no such runner is admitted.
    #[must_use]
    pub fn typed(&self, token: &str, line: &str) -> bool {
        let Some(seat) = self.lock().get(token).cloned() else {
            return false;
        };
        seat.listening.push(listening::Event::Typed {
            line: line.to_owned(),
        });
        true
    }

    /// The runner whose token `offered` is, compared in constant time.
    fn holding(&self, offered: &str) -> Option<Arc<Seat>> {
        self.lock()
            .iter()
            .find(|(token, _)| crate::http::same(offered.as_bytes(), token.as_bytes()))
            .map(|(_, seat)| Arc::clone(seat))
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, BTreeMap<String, Arc<Seat>>> {
        self.seats.lock().unwrap_or_else(PoisonError::into_inner)
    }
}

/// The listener: `/mcp` behind a runner's token, which names its seat.
pub fn router(runners: Runners, stop: &CancellationToken) -> Router {
    let tools = tools::Scripting::new();
    let service = StreamableHttpService::new(
        move || Ok(tools.clone()),
        Arc::new(LocalSessionManager::default()),
        // No MCP session to keep: a runner asks, and is answered, one
        // request at a time, and a restart of either side needs no
        // handshake again.
        StreamableHttpServerConfig::default()
            .with_legacy_session_mode(false)
            .with_json_response(true)
            .with_cancellation_token(stop.child_token()),
    );
    Router::new()
        .nest_service("/mcp", service)
        .layer(axum::middleware::from_fn(
            move |request: Request, next: Next| {
                let runners = runners.clone();
                async move { seated(&runners, request, next).await }
            },
        ))
}

/// Serve on `listener` until `stop`.
///
/// # Errors
///
/// The listener fails.
pub async fn serve(
    listener: tokio::net::TcpListener,
    runners: Runners,
    stop: CancellationToken,
) -> std::io::Result<()> {
    let app = router(runners, &stop);
    axum::serve(listener, app)
        .with_graceful_shutdown(stop.cancelled_owned())
        .await
}

async fn seated(runners: &Runners, mut request: Request, next: Next) -> Response {
    let seat = request
        .headers()
        .get(header::AUTHORIZATION)
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.strip_prefix("Bearer "))
        .and_then(|offered| runners.holding(offered));
    match seat {
        Some(seat) => {
            request.extensions_mut().insert(seat);
            next.run(request).await
        }
        None => (StatusCode::UNAUTHORIZED, "a runner's token is required").into_response(),
    }
}
