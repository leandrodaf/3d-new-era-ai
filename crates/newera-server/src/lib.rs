//! HTTP server for 3D New Era AI.
//!
//! Routes:
//! - `GET /health` — liveness probe
//! - `GET /api/home` — current home as JSON
//! - `GET /api/plan.png?w=&h=` — floor plan image
//! - `GET /api/plan.svg` — floor plan at true scale
//! - `GET /api/view.png?w=&h=&cam=&yaw=&pitch=` — 3D view (software render)
//! - `GET /api/events` — server-sent `revision` events when the document changes
//! - `POST /api/commands` — apply core commands atomically (`{"commands": [...]}`)
//! - `/mcp` — Model Context Protocol (Streamable HTTP)
//!
//! With a token configured, every route except `/health` requires
//! `Authorization: Bearer <token>` (or `?token=` for browsers and SSE).
//! Binding to a non-loopback address without a token is refused.

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

/// Server settings beyond the address.
#[derive(Debug, Clone, Default)]
pub struct ServerOptions {
    /// Shared secret required on every request when set.
    pub token: Option<String>,
}

/// Builds the application router (no authentication).
pub fn router(document: SharedDocument, addr: SocketAddr, shutdown: CancellationToken) -> Router {
    router_with(document, addr, shutdown, &ServerOptions::default())
}

/// Builds the application router with `options`.
pub fn router_with(
    document: SharedDocument,
    addr: SocketAddr,
    shutdown: CancellationToken,
    options: &ServerOptions,
) -> Router {
    // Host checks stop DNS rebinding against a loopback server; a server
    // exposed on purpose is protected by its token instead, and is reached
    // under whatever name the network gives it.
    let allowed_hosts = if options.token.is_some() && !addr.ip().is_loopback() {
        Vec::new()
    } else {
        vec![
            "localhost".to_owned(),
            "127.0.0.1".to_owned(),
            "::1".to_owned(),
            format!("localhost:{}", addr.port()),
            format!("127.0.0.1:{}", addr.port()),
            addr.to_string(),
        ]
    };
    let mcp = newera_mcp::http_service(document.clone(), allowed_hosts, shutdown);

    Router::new()
        .route("/health", get(|| async { "ok" }))
        .route("/api/home", get(get_home))
        .route("/api/plan.png", get(plan_png))
        .route("/api/plan.svg", get(plan_svg))
        .route("/api/view.png", get(view_png))
        .route("/api/events", get(events))
        .route("/api/commands", axum::routing::post(post_commands))
        .nest_service("/mcp", mcp)
        .layer(axum::middleware::from_fn(require_token(
            options.token.clone(),
        )))
        .layer(TraceLayer::new_for_http())
        .with_state(document)
}

type Middleware =
    std::pin::Pin<Box<dyn std::future::Future<Output = axum::response::Response> + Send>>;

/// Rejects requests without the token (when one is configured).
fn require_token(
    token: Option<String>,
) -> impl Fn(axum::extract::Request, axum::middleware::Next) -> Middleware + Clone + Send + Sync + 'static
{
    let token: Option<std::sync::Arc<str>> = token.map(Into::into);
    move |request: axum::extract::Request, next: axum::middleware::Next| {
        let token = token.clone();
        Box::pin(async move {
            let Some(expected) = token else {
                return next.run(request).await;
            };
            if request.uri().path() == "/health" {
                return next.run(request).await;
            }
            let bearer = request
                .headers()
                .get(axum::http::header::AUTHORIZATION)
                .and_then(|v| v.to_str().ok())
                .and_then(|v| v.strip_prefix("Bearer "))
                .map(str::to_owned);
            let query = request.uri().query().and_then(|q| {
                q.split('&')
                    .find_map(|pair| pair.strip_prefix("token="))
                    .map(str::to_owned)
            });
            let given = bearer.or(query).unwrap_or_default();
            // Constant-time comparison.
            let ok = given.len() == expected.len()
                && given
                    .bytes()
                    .zip(expected.bytes())
                    .fold(0u8, |acc, (a, b)| acc | (a ^ b))
                    == 0;
            if ok {
                next.run(request).await
            } else {
                axum::response::IntoResponse::into_response((
                    axum::http::StatusCode::UNAUTHORIZED,
                    "missing or wrong token",
                ))
            }
        })
    }
}

/// `revision` events whenever the document changes (checked 4× per second).
async fn events(
    State(document): State<SharedDocument>,
) -> axum::response::sse::Sse<
    impl futures_util::Stream<Item = Result<axum::response::sse::Event, std::convert::Infallible>>,
> {
    let start = document.read().revision();
    let stream = futures_util::stream::unfold(
        (document, None::<u64>),
        move |(document, last)| async move {
            let mut last = last;
            loop {
                let revision = document.read().revision();
                if last != Some(revision) {
                    last = Some(revision);
                    let event = axum::response::sse::Event::default()
                        .event("revision")
                        .data(revision.to_string());
                    return Some((Ok(event), (document, last)));
                }
                tokio::time::sleep(std::time::Duration::from_millis(250)).await;
            }
        },
    );
    let _ = start;
    axum::response::sse::Sse::new(stream).keep_alive(axum::response::sse::KeepAlive::default())
}

#[derive(Debug, serde::Deserialize)]
struct CommandsBody {
    commands: Vec<newera_core::Command>,
}

/// Applies commands as one undoable step.
async fn post_commands(
    State(document): State<SharedDocument>,
    Json(body): Json<CommandsBody>,
) -> Result<Json<serde_json::Value>, (axum::http::StatusCode, String)> {
    let mut doc = document.write();
    doc.execute(newera_core::Command::Batch {
        commands: body.commands,
    })
    .map_err(|e| (axum::http::StatusCode::UNPROCESSABLE_ENTITY, e.to_string()))?;
    Ok(Json(serde_json::json!({ "revision": doc.revision() })))
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
    serve_listener_with(document, listener, shutdown, ServerOptions::default()).await
}

/// Serves with `options`. Refuses to expose an unauthenticated server beyond
/// this machine.
pub async fn serve_listener_with(
    document: SharedDocument,
    listener: TcpListener,
    shutdown: CancellationToken,
    options: ServerOptions,
) -> std::io::Result<()> {
    let local = listener.local_addr()?;
    if !local.ip().is_loopback() && options.token.is_none() {
        return Err(std::io::Error::other(format!(
            "{local} is reachable from the network: set a token (--token or NEWERA_TOKEN)"
        )));
    }
    tracing::info!("HTTP em http://{local} · MCP em http://{local}/mcp");

    let app = router_with(document, local, shutdown.clone(), &options);
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

    #[tokio::test]
    async fn token_guards_everything_but_health_and_commands_apply() {
        let document = SharedDocument::default();
        let options = ServerOptions {
            token: Some("s3cret".into()),
        };
        let app = router_with(
            document.clone(),
            DEFAULT_ADDR,
            CancellationToken::new(),
            &options,
        );
        let status = |request: Request<Body>| {
            let app = app.clone();
            async move { app.oneshot(request).await.unwrap().status() }
        };
        assert_eq!(
            status(Request::get("/health").body(Body::empty()).unwrap()).await,
            StatusCode::OK
        );
        assert_eq!(
            status(Request::get("/api/home").body(Body::empty()).unwrap()).await,
            StatusCode::UNAUTHORIZED
        );
        assert_eq!(
            status(
                Request::get("/api/home?token=wrong")
                    .body(Body::empty())
                    .unwrap()
            )
            .await,
            StatusCode::UNAUTHORIZED
        );
        assert_eq!(
            status(
                Request::get("/api/home?token=s3cret")
                    .body(Body::empty())
                    .unwrap()
            )
            .await,
            StatusCode::OK
        );
        let body = serde_json::json!({"commands": [{"op": "insert", "element": {"kind": "wall", "id": "w1", "start": [0, 0], "end": [250, 0], "thickness": 15, "height": 250}}]});
        let request = Request::post("/api/commands")
            .header("authorization", "Bearer s3cret")
            .header("content-type", "application/json")
            .body(Body::from(body.to_string()))
            .unwrap();
        let response = app.clone().oneshot(request).await.unwrap();
        let status_code = response.status();
        let text = to_bytes(response.into_body(), usize::MAX).await.unwrap();
        assert_eq!(
            status_code,
            StatusCode::OK,
            "{}",
            String::from_utf8_lossy(&text)
        );
        assert_eq!(document.read().home().walls.len(), 1);
    }

    #[tokio::test]
    async fn exposing_without_a_token_is_refused() {
        let listener = TcpListener::bind("0.0.0.0:0").await.unwrap();
        let result = serve_listener_with(
            SharedDocument::default(),
            listener,
            CancellationToken::new(),
            ServerOptions::default(),
        )
        .await;
        assert!(result.is_err());
    }
}
