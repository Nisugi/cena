//! Offline native setup harness. No credentials, connector, or session exists.
//! Requires explicit map and isolated data-directory arguments.

use cena_behavior::{hunt::setup, travel::read_map};
use serde_json::json;
use sha2::{Digest, Sha256};
use std::{path::PathBuf, sync::Arc};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut args = std::env::args().skip(1);
    let map_path = args
        .next()
        .ok_or("Pass a map path and an isolated fixture data directory")?;
    let dir = PathBuf::from(
        args.next()
            .ok_or("Pass an isolated fixture data directory")?,
    );
    if !dir.is_absolute() {
        return Err("Fixture data directory must be absolute".into());
    }
    let bytes = std::fs::read(map_path)?;
    let hash = format!("{:x}", Sha256::digest(&bytes));
    let map = Arc::new(read_map(&bytes)?);
    let server = cena_web::WebServer::open().await?;
    server.sessions().hunt_setup(cena_session::SessionId(1), Arc::new(move |message| {
        let (map, hash, dir) = (Arc::clone(&map), hash.clone(), dir.clone());
        Box::pin(async move {
            if message["action"] == "inspect" {
                return Ok(json!({"character":"OfflineFixture","instance":"test","generation":"0","map_sha256":hash,"offline":true}));
            }
            if message["action"] != "configure" || message["generation"] != "0" { return Err("Invalid offline request".into()); }
            let config: setup::Request = serde_json::from_value(message["config"].clone()).map_err(|e| e.to_string())?;
            tokio::task::spawn_blocking(move || {
                serde_json::to_value(setup::configure(&dir, ("test", "OfflineFixture"), &map, &hash, &config)?).map_err(|e| e.to_string())
            }).await.map_err(|e| e.to_string())?
        })
    }));
    let pair = server.pairing_url();
    let token = pair
        .split("token=")
        .nth(1)
        .ok_or("Missing pairing fragment")?;
    // Private local launch URL; never a game log or shared test artifact.
    println!(
        "http://{}/atlas/landing/#room=3703&setup_session=1&setup_token={token}",
        server.local_addr()?
    );
    server
        .run(async {
            let _ = tokio::signal::ctrl_c().await;
        })
        .await?;
    Ok(())
}
