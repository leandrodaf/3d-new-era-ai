//! Runs the hosted service.
//!
//!   `DATABASE_URL=postgres://… RESEND_API_KEY=… newera-cloud`
//!
//! See `newera_cloud::Config` for the rest of the environment.

use anyhow::Context as _;
use tracing_subscriber::layer::SubscriberExt as _;
use tracing_subscriber::util::SubscriberInitExt as _;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::registry()
        .with(tracing_subscriber::fmt::layer())
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| "info".into()),
        )
        .init();
    let config = newera_cloud::Config::from_env()?;
    let db = newera_cloud::database(&config.database_url)
        .await
        .context("database")?;
    let port = config.port;
    let state = newera_cloud::AppState::new(config, db.clone());

    // Closed accounts and expired secrets go, once an hour.
    tokio::spawn(async move {
        let mut every = tokio::time::interval(std::time::Duration::from_secs(3600));
        loop {
            every.tick().await;
            match newera_cloud::accounts::purge_closed(&db).await {
                Ok(0) => {}
                Ok(n) => tracing::info!("purged {n} closed accounts"),
                Err(err) => tracing::warn!("purge failed: {err}"),
            }
        }
    });

    let listener = tokio::net::TcpListener::bind(("0.0.0.0", port)).await?;
    tracing::info!("newera-cloud on http://0.0.0.0:{port}");
    axum::serve(
        listener,
        newera_cloud::router(&state).into_make_service_with_connect_info::<std::net::SocketAddr>(),
    )
    .with_graceful_shutdown(async {
        let _ = tokio::signal::ctrl_c().await;
    })
    .await?;
    Ok(())
}
