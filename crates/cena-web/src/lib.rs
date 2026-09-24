//! Embedded loopback viewer. The native session remains authoritative even
//! when every browser disconnects; this crate only projects and submits input.

mod listener;
mod presentation;
mod server;
mod socket;

pub use server::{Sessions, WebServer};
