//! HTTP server for 3D New Era AI.
//!
//! Routes:
//! - `GET /health` — liveness probe
//! - `GET /api/home` — current home as JSON
//! - `GET /api/plan.png?w=&h=` — floor plan image
//! - `GET /api/plan.svg` — floor plan at true scale
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
        .route("/api/plan.png", get(plan_png))
        .route("/api/plan.svg", get(plan_svg))
        .route("/api/view.png", get(view_png))
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

#[derive(Debug, serde::Deserialize)]
struct PlanQuery {
    w: Option<u32>,
    h: Option<u32>,
}

fn scene(document: &SharedDocument) -> (newera_draw::Scene, Option<std::path::PathBuf>) {
    let doc = document.read();
    let options = newera_draw::SceneOptions {
        show_background: true,
        ..newera_draw::SceneOptions::default()
    };
    (
        newera_draw::plan_scene(doc.home(), &options),
        doc.asset_dir(),
    )
}

async fn plan_png(
    State(document): State<SharedDocument>,
    axum::extract::Query(query): axum::extract::Query<PlanQuery>,
) -> Result<([(axum::http::header::HeaderName, &'static str); 1], Vec<u8>), axum::http::StatusCode>
{
    let (scene, project) = scene(&document);
    let options = newera_draw::RenderOptions {
        width: query.w.unwrap_or(1024).clamp(64, 4096),
        height: query.h.unwrap_or(768).clamp(64, 4096),
        ..newera_draw::RenderOptions::default()
    };
    let load = |path: &str| {
        image::open(newera_core::resolve_asset(project.as_deref(), path))
            .ok()
            .map(|img| img.to_rgba8())
    };
    let png = newera_draw::render_png(&scene, &options, &load)
        .map_err(|_| axum::http::StatusCode::INTERNAL_SERVER_ERROR)?;
    Ok(([(axum::http::header::CONTENT_TYPE, "image/png")], png))
}

#[derive(Debug, serde::Deserialize)]
struct ViewQuery {
    w: Option<u32>,
    h: Option<u32>,
    /// Stored point of view index.
    cam: Option<usize>,
    yaw: Option<f32>,
    pitch: Option<f32>,
}

/// The home in 3D, rendered in software (aerial view, or a stored camera).
async fn view_png(
    State(document): State<SharedDocument>,
    axum::extract::Query(query): axum::extract::Query<ViewQuery>,
) -> Result<([(axum::http::header::HeaderName, &'static str); 1], Vec<u8>), axum::http::StatusCode>
{
    let (w, h) = (
        query.w.unwrap_or(800).clamp(64, 2048),
        query.h.unwrap_or(600).clamp(64, 2048),
    );
    let (home, assets) = {
        let doc = document.read();
        (doc.home().clone(), doc.asset_dir())
    };
    let png = tokio::task::spawn_blocking(move || {
        #[allow(clippy::cast_precision_loss)]
        let aspect = w as f32 / h as f32;
        let view = match query.cam.and_then(|i| home.cameras.stored.get(i)) {
            Some(camera) => newera_render::View::from_camera(camera, aspect),
            None => newera_render::View::aerial(
                &home,
                query.yaw.unwrap_or(-60.0),
                query.pitch.unwrap_or(45.0),
            ),
        };
        let image = newera_render::render_home(&home, &view, w, h, assets.as_deref());
        let mut png = Vec::new();
        image::DynamicImage::ImageRgba8(image)
            .write_to(&mut std::io::Cursor::new(&mut png), image::ImageFormat::Png)
            .map(|()| png)
    })
    .await
    .map_err(|_| axum::http::StatusCode::INTERNAL_SERVER_ERROR)?
    .map_err(|_| axum::http::StatusCode::INTERNAL_SERVER_ERROR)?;
    Ok(([(axum::http::header::CONTENT_TYPE, "image/png")], png))
}

async fn plan_svg(
    State(document): State<SharedDocument>,
) -> ([(axum::http::header::HeaderName, &'static str); 1], String) {
    let (scene, _) = scene(&document);
    (
        [(axum::http::header::CONTENT_TYPE, "image/svg+xml")],
        newera_draw::to_svg(&scene, &newera_draw::SvgOptions::default()),
    )
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
            doc.execute(Command::insert(wall)).unwrap();
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
