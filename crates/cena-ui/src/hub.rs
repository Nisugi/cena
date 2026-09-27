//! What a hub asks of whoever runs the sessions: start a character, quit
//! one, log a stopped one back in, shut Hydra down (`plan/29` step 5c).
//!
//! Two hubs ask it -- Despana's page and the window's (`plan/47` §4) -- and
//! one answerer, the binary, which alone knows the roster and the keyring.
//! Here so both name the same requests rather than each its own copy.

use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

/// A request from a hub, for whoever runs the sessions.
///
/// A session is named by its number, `cena_session::SessionId`'s, which a
/// hub has from its card ([`SessionCard::session`](crate::SessionCard::session),
/// canonical decimal): this crate cannot name the session crate's type, and
/// each frontend parses at its own edge.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum HubRequest {
    /// Start this character: one the hub offered as available.
    Add(String),
    /// Quit this session and take it off the table.
    Remove(u32),
    /// Log this stopped session's character back in.
    Reconnect(u32),
    /// Shut Hydra down in order, as Ctrl-C does.
    Shutdown,
}

/// What answers a hub's requests: the owner of the session table. It returns
/// one line for the hub that asked. A closure, not a trait: there is one
/// answerer.
pub type HubControl =
    Arc<dyn Fn(HubRequest) -> Pin<Box<dyn Future<Output = String> + Send>> + Send + Sync>;
