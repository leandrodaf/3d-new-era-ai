//! HTTP server for 3D New Era AI.
//!
//! Routes:
//! - `GET /health` — liveness probe
//! - `GET /api/home` — current home as JSON
//! - `/mcp` — Model Context Protocol (Streamable HTTP)

use std::net::SocketAddr;

use axum::extract::State;
use axum::routing::get;
use axum::{Json, Router};
use newera_core::{Home, SharedDocument};
use serde::Serialize;
use tokio::net::TcpListener;
use tokio_util::sync::CancellationToken;
use tower_http::trace::TraceLayer;

/// Default address: loopback only, so nothing is exposed to the network
/// unless the user asks for it.
pub const DEFAULT_ADDR: SocketAddr =
    SocketAddr::new(std::net::IpAddr::V4(std::net::Ipv4Addr::LOCALHOST), 7878);

#[derive(Debug, Serialize)]
struct HomeResponse {
    revision: u64,
    home: Home,
}

/// Builds the application router.
pub fn router(document: SharedDocument, addr: SocketAddr, shutdown: CancellationToken) -> Router {
    let allowed_hosts = vec![
        "localhost".to_owned(),
        "127.0.0.1".to_owned(),
        "::1".to_owned(),
        format!("localhost:{}", addr.port()),
        format!("127.0.0.1:{}", addr.port()),
        addr.to_string(),
    ];
    let mcp = newera_mcp::http_service(document.clone(), allowed_hosts, shutdown);

    Router::new()
        .route("/health", get(|| async { "ok" }))
        .route("/api/home", get(get_home))
        .nest_service("/mcp", mcp)
        .layer(TraceLayer::new_for_http())
        .with_state(document)
}

/// Binds `addr` and serves until `shutdown` is cancelled.
pub async fn serve(
    document: SharedDocument,
    addr: SocketAddr,
    shutdown: CancellationToken,
) -> std::io::Result<()> {
    serve_listener(document, TcpListener::bind(addr).await?, shutdown).await
}

/// Serves on an already bound listener until `shutdown` is cancelled.
pub async fn serve_listener(
    document: SharedDocument,
    listener: TcpListener,
    shutdown: CancellationToken,
) -> std::io::Result<()> {
    let local = listener.local_addr()?;
    tracing::info!("HTTP em http://{local} · MCP em http://{local}/mcp");

    let app = router(document, local, shutdown.clone());
    axum::serve(listener, app)
        .with_graceful_shutdown(async move { shutdown.cancelled().await })
        .await
}

async fn get_home(State(document): State<SharedDocument>) -> Json<HomeResponse> {
    let doc = document.read();
    Json(HomeResponse {
        revision: doc.revision(),
        home: doc.home().clone(),
    })
}

#[cfg(test)]
mod tests {
    use axum::body::{Body, to_bytes};
    use axum::http::{Request, StatusCode};
    use newera_core::{Command, Document, Point2, Wall};
    use tower::ServiceExt;

    use super::*;

    #[tokio::test]
    async fn api_home_reflects_document() {
        let document = SharedDocument::new(Document::default());
        {
            let mut doc = document.write();
            let wall = Wall::new(
                doc.new_wall_id(),
                Point2::new(0.0, 0.0),
                Point2::new(300.0, 0.0),
            );
            doc.execute(Command::add_wall(wall)).unwrap();
        }

        let app = router(document, DEFAULT_ADDR, CancellationToken::new());
        let response = app
            .oneshot(Request::get("/api/home").body(Body::empty()).unwrap())
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
        let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
        let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(json["revision"], 1);
        assert_eq!(json["home"]["walls"][0]["id"], "w1");
        assert_eq!(
            json["home"]["walls"][0]["end"],
            serde_json::json!([300.0, 0.0])
        );
    }
}
