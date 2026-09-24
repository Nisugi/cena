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
