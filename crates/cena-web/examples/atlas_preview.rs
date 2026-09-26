//! Offline explorer review server. No credentials, session, or game connection.

#[tokio::main(flavor = "current_thread")]
async fn main() -> std::io::Result<()> {
    let server = if let Some(directory) = std::env::var_os("CENA_HUNTING_CORRECTIONS_DIR") {
        cena_web::WebServer::open_hunting_editor(std::path::Path::new(&directory)).await?
    } else {
        cena_web::WebServer::open().await?
    };
    println!("http://{}/atlas/corpus/", server.local_addr()?);
    server.run(std::future::pending()).await
}
