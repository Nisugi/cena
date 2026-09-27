//! `cena-agent`: a program outside Hydra reads a character through MCP
//! (`plan/35-m7-agent.md`, M7).
//!
//! **Step 1 is read-only** (`plan/35` §8): an MCP client, Claude Code among
//! them, lists the characters, reads one's state with every status the game
//! has reported, waits for what happens next, and asks the character's own
//! database a read-only question. Nothing here sends to the game; control
//! levels, operations and commands are the later steps.
//!
//! - [`projection`]: the vocabulary an agent reads, its own and versioned
//!   ([`PROTOCOL`], `CONTRACT.md`), never the session's `Event` or `Frame`.
//! - [`happenings`]: `wait`, from the difference between the session's own
//!   snapshots, never from a second model.
//! - [`records`]: the database, read-only by construction.
//! - [`characters`]: who is seated, as the binary starts and stops them.
//! - [`server`]: the MCP tools, on loopback, behind a bearer token.

pub mod characters;
pub mod happenings;
pub mod projection;
pub mod records;
pub mod server;

pub use characters::Characters;
pub use server::{router, serve};

/// The contract this speaks (`CONTRACT.md`). Bumped when a field changes
/// meaning or goes; a new field or kind is not a break, because a client
/// ignores what it does not know.
pub const PROTOCOL: &str = "hydra-agent/1";

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
