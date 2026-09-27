//! `--agent`: M7's MCP listener (`plan/35`), read-only in step 1.
//!
//! **Off unless asked for.** Control levels are step 2 (`plan/35` §8), so
//! until then the whole listener is the switch: without `--agent`, nothing
//! listens. With it, every character this Hydra runs can be read, and nothing
//! can be sent.
//!
//! **A fixed port and a kept token**, where Despana's page takes any port and
//! prints its link: an MCP client's configuration names the address and the
//! token once, so both stay the same from run to run. The token is made the
//! first time and kept in `<data>/agent.json`, beside the other stores; delete
//! the file to make a new one. `--agent-port N` picks another port.

use std::path::{Path, PathBuf};
use std::sync::{Mutex, PoisonError};

use cena_agent::Characters;
use cena_session::{SessionId, SessionObserver};
use tokio_util::sync::CancellationToken;

/// Where the agent listens unless told otherwise.
const DEFAULT_PORT: u16 = 47_700;

/// The file that keeps the token.
const FILE: &str = "agent.json";

/// Whether `--agent` was given.
pub(crate) fn requested() -> bool {
    std::env::args().skip(1).any(|arg| arg == "--agent")
}

/// `--agent-port N` or `--agent-port=N`, else the default.
fn port() -> u16 {
    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        let value = match arg.strip_prefix("--agent-port=") {
            Some(value) => Some(value.to_owned()),
            None if arg == "--agent-port" => args.next(),
            None => None,
        };
        if let Some(port) = value.and_then(|v| v.parse().ok()) {
            return port;
        }
    }
    DEFAULT_PORT
}

/// The listener, and the characters it can see.
pub(crate) struct Agent {
    characters: Characters,
    stop: CancellationToken,
    task: Mutex<Option<tokio::task::JoinHandle<()>>>,
}

impl Agent {
    /// Listen, when `--agent` was given. `None` when it was not, or when the
    /// listener cannot start; the run goes on without it, and says why.
    pub(crate) async fn open(dir: &Path) -> Option<Self> {
        if !requested() {
            return None;
        }
        let token = match token(dir) {
            Ok(token) => token,
            Err(e) => {
                eprintln!("[agent] not started: no token -- {e}");
                return None;
            }
        };
        let port = port();
        let listener =
            match tokio::net::TcpListener::bind((std::net::Ipv4Addr::LOCALHOST, port)).await {
                Ok(listener) => listener,
                Err(e) => {
                    eprintln!(
                        "[agent] not started: port {port} -- {e}. `--agent-port N` picks another."
                    );
                    return None;
                }
            };
        let url = format!("http://127.0.0.1:{port}/mcp");
        if let Err(e) = save(dir, &url, &token) {
            eprintln!("[agent] the address and token were not saved -- {e}");
        }
        eprintln!(
            "[agent] MCP, read-only, at {url}; its token is in {}",
            dir.join(FILE).display()
        );
        eprintln!(
            "[agent] Claude Code: claude mcp add --transport http hydra {url} --header \"Authorization: Bearer {token}\""
        );
        let characters = Characters::default();
        let stop = CancellationToken::new();
        let serving = (characters.clone(), stop.clone());
        let task = tokio::spawn(async move {
            if let Err(e) = cena_agent::serve(listener, serving.0, token, serving.1).await {
                eprintln!("[agent] stopped -- {e}");
            }
        });
        Some(Self {
            characters,
            stop,
            task: Mutex::new(Some(task)),
        })
    }

    /// A character started: the agent can read it.
    pub(crate) fn seat(
        &self,
        id: SessionId,
        character: &str,
        observer: SessionObserver,
        database: Option<PathBuf>,
    ) {
        let recording = Some(crate::setup::recording());
        self.characters
            .seat(id, character, observer, database, recording);
    }

    /// A character stopped.
    pub(crate) fn unseat(&self, id: SessionId) {
        self.characters.unseat(id);
    }

    /// Stop listening.
    pub(crate) async fn shutdown(&self) {
        self.stop.cancel();
        let task = self
            .task
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .take();
        if let Some(task) = task {
            let _ = task.await;
        }
    }
}

/// The kept token, or a new one, kept.
fn token(dir: &Path) -> Result<String, String> {
    let kept = std::fs::read_to_string(dir.join(FILE))
        .ok()
        .and_then(|text| serde_json::from_str::<serde_json::Value>(&text).ok())
        .and_then(|json| json.get("token")?.as_str().map(str::to_owned))
        .filter(|token| !token.is_empty());
    match kept {
        Some(token) => Ok(token),
        None => cena_agent::new_token(),
    }
}

fn save(dir: &Path, url: &str, token: &str) -> std::io::Result<()> {
    std::fs::create_dir_all(dir)?;
    let text = serde_json::to_string_pretty(&serde_json::json!({ "url": url, "token": token }))
        .map_err(std::io::Error::other)?;
    std::fs::write(dir.join(FILE), text)
}
