//! `cena-agent`: a program outside Hydra reads a character through MCP
//! (`plan/35-m7-agent.md`, M7).
//!
//! An MCP client, Claude Code among them, lists the characters, reads one's
//! state with every status the game has reported, waits for what happens
//! next, reads the game's text, and asks the character's own database a
//! read-only question; and, as far as each character's level allows
//! (`cena_session::agent`, `plan/35` §3), tells the player something, runs a
//! behavior as an operation and steers it, or sends a line to the game. Every
//! act goes through the session's door, which checks the level; this crate
//! holds no handle to act with.
//!
//! - [`projection`]: the vocabulary an agent reads, its own and versioned
//!   ([`PROTOCOL`], `CONTRACT.md`), never the session's `Event` or `Frame`.
//! - [`happenings`]: `wait`, from the difference between the session's own
//!   snapshots, never from a second model.
//! - [`records`]: the database, read-only by construction.
//! - [`characters`]: who is seated, as the binary starts and stops them.
//! - [`server`]: the MCP tools.
//! - [`http`]: the listener: `/mcp` behind a bearer token, `/health`, on
//!   loopback.
//! - [`scripts`]: a script runner's own listener and tools (`plan/46`, M7b):
//!   the game's lines as it sent them, a line as if typed, and text for the
//!   player, over the character's script door.

pub mod characters;
pub mod gemstone;
pub mod happenings;
pub mod http;
pub mod projection;
pub mod records;
pub mod scripts;
pub mod server;

pub use characters::Characters;
pub use http::{new_token, router, serve};

/// The contract this speaks (`CONTRACT.md`). Bumped when a field changes
/// meaning or goes; a new field or kind is not a break, because a client
/// ignores what it does not know.
pub const PROTOCOL: &str = "hydra-agent/1";
