//! Embedded loopback viewer. The native session remains authoritative even
//! when every browser disconnects; this crate only projects and submits input.

mod atlas;
mod atlas_assets;
mod atlas_data;
mod listener;
mod merged;
mod presentation;
mod server;
mod socket;

pub use server::{HubControl, HubRequest, Sessions, WebServer};

/// Pure, bounded host projection over the same snapshot as the room pane.
/// Called only while Ready. It must not perform I/O or send commands.
pub type MapProjection =
    std::sync::Arc<dyn Fn(&cena_session::Snapshot) -> cena_ui::MapLocationView + Send + Sync>;
