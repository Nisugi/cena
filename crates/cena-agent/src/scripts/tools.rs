//! A runner's tools (`SCRIPTS.md`): `listen` to its character, `send` a line
//! as if typed, `say` something to the player. The token a request carries
//! names the seat it acts on ([`super::Runners`]), so no tool names a
//! character.

use std::sync::Arc;
use std::time::Duration;

use axum::http::request::Parts;
use cena_session::script::Sending;
use cena_session::{Notice, NoticeKind, Refusal};
use rmcp::handler::server::router::tool::ToolRouter;
use rmcp::handler::server::tool::Extension;
use rmcp::handler::server::wrapper::Parameters;
use rmcp::model::{CallToolResult, ContentBlock, ServerCapabilities, ServerConfig};
use rmcp::{ErrorData, ServerHandler, schemars, tool, tool_handler, tool_router};
use serde::Deserialize;

use super::Seat;

/// What the server tells a runner it is for.
const INSTRUCTIONS: &str = "Hydra runs game characters; this listener is for the script runner \
Hydra started for one of them, and the token names which. `listen` returns what happened \
after a position: the game's lines as it sent them, lines sent, prompts, the player's \
commands for the runner, the connection's state. `send` sends a line as the player would \
type it, Hydra's own command when it starts with the command symbol. `say` shows the \
player text. See SCRIPTS.md (hydra-script/1).";

/// The longest `say` may be, in characters: a screenful of a table.
pub const MAX_SAID: usize = 20_000;

/// The tools, over the seat each request's token names.
#[derive(Clone)]
pub struct Scripting {
    tool_router: ToolRouter<Self>,
}

/// `listen`'s question.
#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct Listen {
    /// Return what came after this position: 0, or absent, for everything
    /// kept; then the last answer's `next`. Asking from a position lets go
    /// of everything at or before it.
    pub since: Option<u64>,
    /// How long to wait for the first, in milliseconds; at most 30000.
    /// Absent is 10000.
    pub timeout_ms: Option<u64>,
}

/// `send`'s line.
#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct Line {
    /// One line, as the player would type it.
    pub line: String,
}

/// `room`'s question.
#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct Room {
    /// The map's own room number, as `map_room` gives it.
    pub id: u32,
}

/// `spell`'s question: by number, or by name.
#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct SpellAsked {
    /// The spell's number: 215.
    pub number: Option<u16>,
    /// Its name, ignoring case: `Heroism`.
    pub name: Option<String>,
}

/// `say`'s text.
#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct Said {
    /// What to show: split at each line break, every piece a line, so an
    /// empty text is one blank line.
    pub text: String,
    /// `info` (absent), `warn`, `error` or `debug`: how it is coloured.
    pub kind: Option<String>,
    /// Fixed-width and never re-wrapped, for a table. Absent is false.
    pub mono: Option<bool>,
}

impl Default for Scripting {
    fn default() -> Self {
        Self::new()
    }
}

#[tool_router]
impl Scripting {
    /// The tools.
    #[must_use]
    pub fn new() -> Self {
        Self {
            tool_router: Self::tool_router(),
        }
    }

    #[tool(
        description = "What happened to the character after a position: `line` (the game's text as it sent it), `sent`, `prompt`, `typed` (a command the player typed for the runner), `lifecycle`, `lagged`. Waits up to timeout_ms for the first. Ask next from `next`."
    )]
    async fn listen(
        &self,
        Extension(parts): Extension<Parts>,
        Parameters(asked): Parameters<Listen>,
    ) -> Result<CallToolResult, ErrorData> {
        let seat = seat(&parts)?;
        let timeout = Duration::from_millis(asked.timeout_ms.unwrap_or(10_000));
        let heard = seat
            .listening
            .listen(asked.since.unwrap_or(0), timeout)
            .await;
        json(&heard)
    }

    #[tool(
        description = "Send one line as the player would type it: to the game, or to Hydra when it starts with the command symbol. Answers the `sent` event's cursor, or why it was not sent."
    )]
    async fn send(
        &self,
        Extension(parts): Extension<Parts>,
        Parameters(Line { line }): Parameters<Line>,
    ) -> Result<CallToolResult, ErrorData> {
        let seat = seat(&parts)?;
        if line.contains(['\r', '\n']) {
            return Err(ErrorData::invalid_params("one line, no line breaks", None));
        }
        let answer = match seat.door.send(&line).await {
            Sending::Sent { cursor } => serde_json::json!({ "outcome": "sent", "cursor": cursor }),
            Sending::Ran => serde_json::json!({ "outcome": "ran" }),
            Sending::Unknown => serde_json::json!({ "outcome": "unknown" }),
            Sending::Refused(refusal) => {
                serde_json::json!({ "outcome": "refused", "why": why(refusal) })
            }
            Sending::Lost => serde_json::json!({ "outcome": "lost" }),
        };
        json(&answer)
    }

    #[tool(
        description = "Show the player text, as a script's `respond` or `echo` does: `kind` colours it, `mono` keeps its columns."
    )]
    async fn say(
        &self,
        Extension(parts): Extension<Parts>,
        Parameters(said): Parameters<Said>,
    ) -> Result<CallToolResult, ErrorData> {
        let seat = seat(&parts)?;
        if said.text.chars().count() > MAX_SAID {
            return Err(ErrorData::invalid_params(
                format!("at most {MAX_SAID} characters"),
                None,
            ));
        }
        let kind = match said.kind.as_deref().unwrap_or("info") {
            "info" => NoticeKind::Info,
            "warn" => NoticeKind::Warn,
            "error" => NoticeKind::Error,
            "debug" => NoticeKind::Debug,
            other => {
                return Err(ErrorData::invalid_params(
                    format!("`{other}` is not a kind: info, warn, error or debug"),
                    None,
                ));
            }
        };
        let lines: Vec<String> = said
            .text
            .split('\n')
            .map(|line| line.strip_suffix('\r').unwrap_or(line).to_owned())
            .collect();
        let notice = if said.mono.unwrap_or(false) {
            Notice::table(kind, lines)
        } else {
            Notice {
                kind,
                body: cena_session::Body::Lines(lines),
            }
        };
        seat.door.say(notice);
        json(&serde_json::json!({ "said": true }))
    }

    #[tool(
        description = "A room of Hydra's map, by the map's own number: its titles, descriptions, exits lines, the game's numbers for it, location, tags, and each exit with how it is crossed and what it costs. `room` null when there is no such room, `map` false when Hydra has no map."
    )]
    async fn room(
        &self,
        Extension(parts): Extension<Parts>,
        Parameters(Room { id }): Parameters<Room>,
    ) -> Result<CallToolResult, ErrorData> {
        let seat = seat(&parts)?;
        let Some(atlas) = &seat.atlas else {
            return json(&serde_json::json!({ "map": false, "room": null }));
        };
        let room = atlas.map.room(cena_map::RoomId(id));
        json(&serde_json::json!({ "map": true, "room": room }))
    }

    #[tool(
        description = "A spell of the spell table, by number or name: its name, type, who it can be cast on, its costs, and how long it lasts, both evaluated for the character now. `spell` null when the table has no such spell."
    )]
    async fn spell(
        &self,
        Extension(parts): Extension<Parts>,
        Parameters(asked): Parameters<SpellAsked>,
    ) -> Result<CallToolResult, ErrorData> {
        let seat = seat(&parts)?;
        let spell = match (asked.number, asked.name.as_deref()) {
            (Some(number), _) => cena_session::spells::spell(number),
            (None, Some(name)) => cena_session::spell_named(name),
            (None, None) => {
                return Err(ErrorData::invalid_params("a number or a name", None));
            }
        };
        let Some(spell) = spell else {
            return json(&serde_json::json!({ "spell": null }));
        };
        let stop = tokio_util::sync::CancellationToken::new();
        let (snapshot, _) = crate::characters::subscribe(&seat.observer, &stop)
            .await
            .ok_or_else(|| ErrorData::internal_error("the character has no session", None))?;
        let state = &snapshot.state;
        let cost = |kind| state.spell_cost(spell.number, kind);
        let minutes = |cast| state.spell_minutes(spell.number, cast);
        json(&serde_json::json!({ "spell": {
            "number": spell.number,
            "name": spell.name,
            "type": spell.kind,
            "availability": spell.availability,
            "costs": {
                "mana": cost("mana"),
                "spirit": cost("spirit"),
                "stamina": cost("stamina"),
                "renew": cost("renew"),
            },
            "minutes": {
                "self": minutes(cena_session::spells::CastType::SelfCast),
                "target": minutes(cena_session::spells::CastType::Target),
            },
        }}))
    }
}

#[tool_handler(router = self.tool_router)]
impl ServerHandler for Scripting {
    fn get_info(&self) -> ServerConfig {
        ServerConfig::new(ServerCapabilities::builder().enable_tools().build())
            .with_instructions(INSTRUCTIONS)
    }
}

/// The seat the request's token named.
fn seat(parts: &Parts) -> Result<Arc<Seat>, ErrorData> {
    parts
        .extensions
        .get::<Arc<Seat>>()
        .cloned()
        .ok_or_else(|| ErrorData::internal_error("no runner's seat on this request", None))
}

/// Why the session did not send a line, in words.
fn why(refusal: Refusal) -> &'static str {
    match refusal {
        Refusal::Transient => "busy: the session is not ready, or its queue is full",
        Refusal::Permanent => "refused",
        Refusal::Roundtime => "roundtime",
        Refusal::Stunned => "stunned",
        Refusal::Webbed => "webbed",
        Refusal::Casttime => "cast roundtime",
        Refusal::TargetGone => "its target is gone",
    }
}

fn json(value: &impl serde::Serialize) -> Result<CallToolResult, ErrorData> {
    Ok(CallToolResult::success(vec![ContentBlock::json(value)?]))
}
