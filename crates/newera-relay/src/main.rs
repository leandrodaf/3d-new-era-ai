//! Runs the relay. One port, no database, no state worth keeping: everything
//! it holds dies with the process, and nothing anybody drew is in it.
//!
//!   newera-relay                 # 0.0.0.0:7979
//!   PORT=8080 newera-relay

use std::net::SocketAddr;

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
