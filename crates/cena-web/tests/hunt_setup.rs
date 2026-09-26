//! Configuration cannot be reached without pairing, origin, and an explicit
//! session handler. No game session is created by this test.
use std::{
    net::SocketAddr,
    sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    },
};
use tokio::io::{AsyncReadExt, AsyncWriteExt};

async fn post(
    address: SocketAddr,
    token: &str,
    origin: &str,
    session: &str,
) -> std::io::Result<String> {
    let mut stream = tokio::net::TcpStream::connect(address).await?;
    let body = serde_json::json!({"session":session,"message":{"action":"inspect"}}).to_string();
    let wire = format!(
        "POST /hunt/setup HTTP/1.1\r\nHost: {address}\r\nOrigin: {origin}\r\nAuthorization: Bearer {token}\r\nContent-Type: application/json\r\nConnection: close\r\nContent-Length: {}\r\n\r\n{body}",
        body.len()
    );
    stream.write_all(wire.as_bytes()).await?;
    let mut response = String::new();
    stream.read_to_string(&mut response).await?;
    Ok(response)
}

#[tokio::test]
async fn pairing_origin_and_session_are_required_and_detach_removes_access() {
    let server = cena_web::WebServer::open().await.unwrap();
    let addr = server.local_addr().unwrap();
    let origin = format!("http://{addr}");
    let pair = server.pairing_url();
    let token = pair.split("token=").nth(1).unwrap().to_owned();
    let sessions = server.sessions();
    let calls = Arc::new(AtomicUsize::new(0));
    let seen = Arc::clone(&calls);
    sessions.hunt_setup(
        cena_session::SessionId(7),
        Arc::new(move |_| {
            seen.fetch_add(1, Ordering::Relaxed);
            Box::pin(async { Ok(serde_json::json!({"character":"Fixture","started":false})) })
        }),
    );
    let stop = tokio_util::sync::CancellationToken::new();
    let end = stop.clone();
    let task = tokio::spawn(server.run(end.cancelled_owned()));
    for (token, origin, session, status) in [
        ("bad", origin.as_str(), "7", "403"),
        (token.as_str(), "http://foreign.invalid", "7", "403"),
        (token.as_str(), origin.as_str(), "8", "404"),
    ] {
        let reply = post(addr, token, origin, session).await.unwrap();
        assert!(reply.starts_with(&format!("HTTP/1.1 {status}")), "{reply}");
    }
    assert_eq!(calls.load(Ordering::Relaxed), 0);
    let reply = post(addr, &token, &origin, "7").await.unwrap();
    assert!(reply.starts_with("HTTP/1.1 200"));
    assert!(reply.contains("Fixture"));
    assert_eq!(calls.load(Ordering::Relaxed), 1);
    sessions.detach(cena_session::SessionId(7));
    assert!(
        post(addr, &token, &origin, "7")
            .await
            .unwrap()
            .starts_with("HTTP/1.1 404")
    );
    stop.cancel();
    task.await.unwrap().unwrap();
}
