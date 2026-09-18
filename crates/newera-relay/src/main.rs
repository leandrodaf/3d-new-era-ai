//! Runs the relay. One port, no database, no state worth keeping: everything
//! it holds dies with the process, and nothing anybody drew is in it.
//!
//!   newera-relay                 # 0.0.0.0:7979
//!   PORT=8080 newera-relay

use std::net::SocketAddr;

/// Asks the running service whether it is well, from inside its own
/// container, where there is no shell and no curl to ask with.
async fn health(port: u16) -> std::io::Result<()> {
    use tokio::io::{AsyncReadExt as _, AsyncWriteExt as _};

    let mut stream = tokio::net::TcpStream::connect(("127.0.0.1", port)).await?;
    stream
        .write_all(b"GET /health HTTP/1.0\r\nHost: localhost\r\n\r\n")
        .await?;
    let mut said = String::new();
    stream.read_to_string(&mut said).await?;
    if said.starts_with("HTTP/1.0 200") || said.starts_with("HTTP/1.1 200") {
        Ok(())
    } else {
        Err(std::io::Error::other(said.lines().next().unwrap_or("no answer").to_owned()))
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "newera_relay=info,tower_http=warn".into()),
        )
        .init();

    let port: u16 = std::env::var("PORT")
        .ok()
        .and_then(|p| p.parse().ok())
        .unwrap_or(7979);
    // `--health` is how the container checks itself: same binary, no shell.
    if std::env::args().any(|arg| arg == "--health") {
        health(port).await?;
        println!("ok");
        return Ok(());
    }
    let addr = SocketAddr::from(([0, 0, 0, 0], port));
    let listener = tokio::net::TcpListener::bind(addr).await?;
    tracing::info!("relay on http://{addr} — health at /health");
    axum::serve(listener, newera_relay::router())
        .with_graceful_shutdown(async {
            let _ = tokio::signal::ctrl_c().await;
        })
        .await?;
    Ok(())
}
