//! Offline explorer review server. No credentials, session, or game connection.

#[tokio::main(flavor = "current_thread")]
async fn main() -> std::io::Result<()> {
    let server = cena_web::WebServer::open().await?;
    println!("http://{}/atlas/corpus/", server.local_addr()?);
    server.run(std::future::pending()).await
}
