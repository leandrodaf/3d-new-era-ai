//! HTTP server for 3D New Era AI.
//!
//! Routes:
//! - `GET /health` — liveness probe
//! - `GET /api/home` — current home as JSON; every piece also carries
//!   `bounds` `[[min_x, min_y], [max_x, max_y]]`, its box with `angle` applied
//! - `GET|POST /api/check`, `/api/ergonomics`, `/api/measure`,
//!   `/api/annotations` — the MCP analyses of the same name, same arguments
//!   (query string or JSON body) and same answer, without an MCP session
//! - `GET /api/plan.png?w=&h=` — floor plan image
//! - `GET /api/plan.svg` — floor plan at true scale
//! - `GET /api/view.png?w=&h=&cam=&yaw=&pitch=` — 3D view (software render)
//! - `GET /api/events` — server-sent `revision` events when the document changes
//!   and `sessions` events (the session list) when collaborators come, go or move
//! - `POST /api/commands` — apply core commands atomically (`{"commands": [...]}`);
//!   `base_revision` rejects stale edits with 409, `session` credits the edit
//! - `GET|POST /api/sessions`, `POST|DELETE /api/sessions/{id}` — collaborators:
//!   join with a name, report cursor/level/selection, leave
//! - `GET /api/plugins`, `POST /api/plugins/{name}/run` — list and run plugins
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
    /// Plugin directories searched before the default ones.
    pub plugin_dirs: Vec<std::path::PathBuf>,
}

/// Directories the plugin routes search.
#[derive(Debug, Clone)]
struct PluginDirs(std::sync::Arc<Vec<std::path::PathBuf>>);

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
        .route("/api/check", get(analysis_query).post(analysis_body))
        .route("/api/ergonomics", get(analysis_query).post(analysis_body))
        .route("/api/measure", get(analysis_query).post(analysis_body))
        .route("/api/annotations", get(analysis_query).post(analysis_body))
        .route("/api/plan.png", get(plan_png))
        .route("/api/plan.svg", get(plan_svg))
        .route("/api/view.png", get(view_png))
        .route("/api/events", get(events))
        .route("/api/commands", axum::routing::post(post_commands))
        .route("/api/sessions", get(get_sessions).post(post_session))
        .route(
            "/api/sessions/{id}",
            axum::routing::post(post_presence).delete(delete_session),
        )
        .route("/api/plugins", get(get_plugins))
        .route("/api/plugins/{name}/run", axum::routing::post(run_plugin))
        .nest_service("/mcp", mcp)
        .layer(axum::Extension(PluginDirs(std::sync::Arc::new(
            options
                .plugin_dirs
                .iter()
                .cloned()
                .chain(newera_plugins::plugin_dirs())
                .collect(),
        ))))
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

/// `revision` events whenever the document changes and `sessions` events
/// when collaborators change (checked 4× per second).
async fn events(
    State(document): State<SharedDocument>,
) -> axum::response::sse::Sse<
    impl futures_util::Stream<Item = Result<axum::response::sse::Event, std::convert::Infallible>>,
> {
    let start = document.read().revision();
    let stream = futures_util::stream::unfold(
        (document, None::<u64>, None::<u64>),
        move |(document, last, last_sessions)| async move {
            let (mut last, mut last_sessions) = (last, last_sessions);
            loop {
                let (revision, generation, sessions) = {
                    let mut doc = document.write();
                    doc.sessions_mut().expire(newera_core::collab::now_ms());
                    let sessions = doc.sessions();
                    (
                        doc.revision(),
                        sessions.generation(),
                        sessions.list().to_vec(),
                    )
                };
                if last != Some(revision) {
                    last = Some(revision);
                    let event = axum::response::sse::Event::default()
                        .event("revision")
                        .data(revision.to_string());
                    return Some((Ok(event), (document, last, last_sessions)));
                }
                if last_sessions != Some(generation) {
                    last_sessions = Some(generation);
                    let event = axum::response::sse::Event::default()
                        .event("sessions")
                        .data(serde_json::to_string(&sessions).unwrap_or_default());
                    return Some((Ok(event), (document, last, last_sessions)));
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
    /// Revision the client based its edit on; a newer document is a conflict.
    #[serde(default)]
    base_revision: Option<u64>,
    /// Session to credit the edit to.
    #[serde(default)]
    session: Option<String>,
}

type ApiError = (axum::http::StatusCode, String);

/// Applies commands as one undoable step.
async fn post_commands(
    State(document): State<SharedDocument>,
    Json(body): Json<CommandsBody>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let mut doc = document.write();
    if let Some(base) = body.base_revision
        && base != doc.revision()
    {
        return Err((
            axum::http::StatusCode::CONFLICT,
            format!(
                "the project changed since revision {base} (now {}); reload and retry",
                doc.revision()
            ),
        ));
    }
    let annotations = doc.home().annotations;
    doc.execute(newera_core::Command::Batch {
        commands: body.commands,
    })
    .map_err(|e| (axum::http::StatusCode::UNPROCESSABLE_ENTITY, e.to_string()))?;
    let revision = doc.revision();
    if let Some(session) = &body.session {
        doc.sessions_mut()
            .record_edit(session, revision, newera_core::collab::now_ms());
    }
    let mut reply = serde_json::json!({ "revision": revision });
    // A batch that switches the plan's annotations says so: losing the
    // reference numbers of a whole drawing must not be silent.
    if doc.home().annotations != annotations {
        let view = |a: newera_core::PlanAnnotations| {
            serde_json::json!({
                "auto_dimensions": a.auto_dimensions,
                "references": a.references,
                "reference_details": a.reference_details,
                "legend": a.legend,
            })
        };
        reply["annotations"] = serde_json::json!({
            "from": view(annotations),
            "to": view(doc.home().annotations),
        });
    }
    Ok(Json(reply))
}

async fn get_sessions(State(document): State<SharedDocument>) -> Json<serde_json::Value> {
    let mut doc = document.write();
    doc.sessions_mut().expire(newera_core::collab::now_ms());
    Json(serde_json::json!({
        "revision": doc.revision(),
        "sessions": doc.sessions().list(),
    }))
}

#[derive(Debug, Default, serde::Deserialize)]
struct JoinBody {
    #[serde(default)]
    name: String,
}

async fn post_session(
    State(document): State<SharedDocument>,
    body: Option<Json<JoinBody>>,
) -> Json<newera_core::collab::Session> {
    let name = body.map(|b| b.0.name).unwrap_or_default();
    Json(
        document
            .write()
            .sessions_mut()
            .join(&name, newera_core::collab::now_ms()),
    )
}

#[derive(Debug, Default, serde::Deserialize)]
struct PresenceBody {
    #[serde(default)]
    cursor: Option<newera_core::Point2>,
    #[serde(default)]
    level: Option<String>,
    #[serde(default)]
    selection: Vec<String>,
}

async fn post_presence(
    State(document): State<SharedDocument>,
    axum::extract::Path(id): axum::extract::Path<String>,
    body: Option<Json<PresenceBody>>,
) -> Result<axum::http::StatusCode, ApiError> {
    let body = body.map(|b| b.0).unwrap_or_default();
    let bad = |e: &dyn std::fmt::Display| (axum::http::StatusCode::BAD_REQUEST, e.to_string());
    let presence = newera_core::collab::Presence {
        cursor: body.cursor,
        level: body
            .level
            .as_deref()
            .map(str::parse)
            .transpose()
            .map_err(|e| bad(&e))?,
        selection: body
            .selection
            .iter()
            .map(|s| s.parse())
            .collect::<Result<_, _>>()
            .map_err(|e| bad(&e))?,
    };
    if document
        .write()
        .sessions_mut()
        .update(&id, presence, newera_core::collab::now_ms())
    {
        Ok(axum::http::StatusCode::NO_CONTENT)
    } else {
        Err((
            axum::http::StatusCode::NOT_FOUND,
            format!("no session {id}"),
        ))
    }
}

async fn delete_session(
    State(document): State<SharedDocument>,
    axum::extract::Path(id): axum::extract::Path<String>,
) -> axum::http::StatusCode {
    if document.write().sessions_mut().leave(&id) {
        axum::http::StatusCode::NO_CONTENT
    } else {
        axum::http::StatusCode::NOT_FOUND
    }
}

async fn get_plugins(
    axum::Extension(dirs): axum::Extension<PluginDirs>,
) -> Json<serde_json::Value> {
    let plugins = tokio::task::spawn_blocking(move || newera_plugins::discover(&dirs.0))
        .await
        .unwrap_or_default();
    Json(serde_json::json!({ "plugins": plugins }))
}

/// Runs a plugin (arguments = request body) under its own session.
async fn run_plugin(
    State(document): State<SharedDocument>,
    axum::Extension(dirs): axum::Extension<PluginDirs>,
    axum::extract::Path(name): axum::extract::Path<String>,
    body: Option<Json<serde_json::Value>>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let args = body.map_or(serde_json::Value::Null, |b| b.0);
    let output = tokio::task::spawn_blocking(move || {
        newera_plugins::run_for_document(&document, &dirs.0, &name, &args)
    })
    .await
    .map_err(|e| (axum::http::StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?
    .map_err(|e| {
        let status = match e {
            newera_plugins::RunError::NotFound(_) => axum::http::StatusCode::NOT_FOUND,
            newera_plugins::RunError::NoServer => axum::http::StatusCode::SERVICE_UNAVAILABLE,
            newera_plugins::RunError::Start(_) => axum::http::StatusCode::INTERNAL_SERVER_ERROR,
        };
        (status, e.to_string())
    })?;
    Ok(Json(output))
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
    // Plugins call back through the loopback when the server listens on all
    // interfaces.
    let reach = if local.ip().is_unspecified() {
        SocketAddr::new(std::net::Ipv4Addr::LOCALHOST.into(), local.port())
    } else {
        local
    };
    document
        .write()
        .set_server(Some(newera_core::collab::ServerInfo {
            url: format!("http://{reach}"),
            token: options.token.clone(),
        }));

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
    // Only the storey being edited, like the editor and MCP draw it.
    let view = doc.home().level_view(doc.home().current_level());
    (newera_draw::plan_scene(&view, &options), doc.asset_dir())
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

/// Adds `bounds` to every piece and part, the box on the plan that every MCP
/// read gives: `width` and `depth` are the piece before `angle`, and a
/// wardrobe turned a quarter turn computed by hand lands 60 cm from where it
/// stands.
fn with_bounds(pieces: &mut serde_json::Value, home: &Home) {
    for piece in pieces.as_array_mut().into_iter().flatten() {
        let found = piece["id"]
            .as_str()
            .and_then(|id| id.parse().ok())
            .and_then(|id| home.find_piece(id));
        if let Some(found) = found {
            let (min, max) = newera_core::plan_bounds(found);
            let round = |v: f64| (v * 10.0).round() / 10.0;
            piece["bounds"] =
                serde_json::json!([[round(min.x), round(min.y)], [round(max.x), round(max.y)]]);
        }
        if let Some(children) = piece.get_mut("children") {
            with_bounds(children, home);
        }
    }
}

async fn get_home(State(document): State<SharedDocument>) -> Json<serde_json::Value> {
    let doc = document.read();
    let mut json = serde_json::to_value(HomeResponse {
        revision: doc.revision(),
        home: doc.home().clone(),
    })
    .unwrap_or_default();
    with_bounds(&mut json["home"]["furniture"], doc.home());
    Json(json)
}

/// Which analysis a route answers.
fn analysis_name(uri: &axum::http::Uri) -> &'static str {
    match uri.path().rsplit('/').next() {
        Some("check") => "check_layout",
        Some("ergonomics") => "ergonomics",
        Some("measure") => "measure",
        _ => "annotations",
    }
}

fn run_analysis(
    document: SharedDocument,
    name: &str,
    args: serde_json::Value,
) -> Result<axum::response::Response, ApiError> {
    let answer = newera_mcp::analysis(document, name, args)
        .map_err(|e| (axum::http::StatusCode::UNPROCESSABLE_ENTITY, e))?;
    Ok(axum::response::IntoResponse::into_response((
        [(axum::http::header::CONTENT_TYPE, "application/json")],
        answer,
    )))
}

/// An analysis with its arguments in the query string: `?occupants=3`.
/// Values that read as JSON (numbers, `true`, `[…]`) are taken as such.
async fn analysis_query(
    State(document): State<SharedDocument>,
    uri: axum::http::Uri,
    axum::extract::Query(query): axum::extract::Query<Vec<(String, String)>>,
) -> Result<axum::response::Response, ApiError> {
    let args: serde_json::Map<String, serde_json::Value> = query
        .into_iter()
        .filter(|(key, _)| key != "token")
        .map(|(key, raw)| {
            let value = serde_json::from_str(&raw).unwrap_or(serde_json::Value::String(raw));
            (key, value)
        })
        .collect();
    run_analysis(
        document,
        analysis_name(&uri),
        serde_json::Value::Object(args),
    )
}

/// An analysis with its arguments as a JSON body.
async fn analysis_body(
    State(document): State<SharedDocument>,
    uri: axum::http::Uri,
    body: Option<Json<serde_json::Value>>,
) -> Result<axum::response::Response, ApiError> {
    let args = body.map_or(serde_json::Value::Null, |b| b.0);
    run_analysis(document, analysis_name(&uri), args)
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
    async fn a_script_can_ask_the_analyses_and_read_turned_boxes() {
        let document = SharedDocument::new(Document::default());
        {
            let mut doc = document.write();
            let piece = |id: u64, x: f64, angle: f64| newera_core::Furniture {
                id: newera_core::FurnitureId(id),
                catalog: "box".into(),
                name: format!("armário {id}"),
                position: Point2::new(x, 221.0),
                width: 185.0,
                depth: 58.0,
                height: 220.0,
                angle,
                ..newera_core::Furniture::default()
            };
            // A wardrobe turned a quarter turn, and another pushed into it.
            doc.execute(Command::insert(piece(1, 221.0, 90.0))).unwrap();
            doc.execute(Command::insert(piece(2, 250.0, 90.0))).unwrap();
        }
        let app = router(document, DEFAULT_ADDR, CancellationToken::new());
        let call = |request: Request<Body>| {
            let app = app.clone();
            async move {
                let response = app.oneshot(request).await.unwrap();
                let status = response.status();
                let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
                (
                    status,
                    serde_json::from_slice::<serde_json::Value>(&body).ok(),
                    body,
                )
            }
        };

        // The box on the plan, not width and depth before the turn.
        let (status, home, _) = call(Request::get("/api/home").body(Body::empty()).unwrap()).await;
        assert_eq!(status, StatusCode::OK);
        let home = home.unwrap();
        assert_eq!(
            home["home"]["furniture"][0]["bounds"],
            serde_json::json!([[192.0, 128.5], [250.0, 313.5]]),
            "{}",
            home["home"]["furniture"][0]
        );

        let (status, check, _) =
            call(Request::get("/api/check").body(Body::empty()).unwrap()).await;
        assert_eq!(status, StatusCode::OK);
        let check = check.unwrap();
        assert_eq!(check["overlap"][0]["kind"], "collision", "{check}");

        let (status, review, _) = call(
            Request::get("/api/ergonomics?occupants=3&wheelchair=false")
                .body(Body::empty())
                .unwrap(),
        )
        .await;
        assert_eq!(status, StatusCode::OK);
        assert!(review.unwrap()["score"].is_u64());

        let (status, tape, _) = call(
            Request::post("/api/measure")
                .header("content-type", "application/json")
                .body(Body::from(r#"{"from":"f1","to":"f2","axis":"x"}"#))
                .unwrap(),
        )
        .await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(tape.unwrap()["cm"], -29.0);

        // Arguments that do not fit say why.
        let (status, _, body) = call(
            Request::get("/api/check?level=nowhere")
                .body(Body::empty())
                .unwrap(),
        )
        .await;
        assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
        assert!(String::from_utf8_lossy(&body).contains("level"));
    }

    #[tokio::test]
    async fn a_part_of_a_group_is_updated_through_the_api_like_a_piece() {
        let document = SharedDocument::new(Document::default());
        {
            let mut doc = document.write();
            let part = |id: u64, name: &str| newera_core::Furniture {
                id: newera_core::FurnitureId(id),
                catalog: "box".into(),
                name: name.to_owned(),
                width: 30.0,
                depth: 30.0,
                height: 30.0,
                ..newera_core::Furniture::default()
            };
            let mut group = part(1, "torre");
            group.children = vec![part(2, "48 — gabinete"), part(3, "puxador")];
            doc.execute(Command::insert(group)).unwrap();
        }
        let mut renamed = document
            .read()
            .home()
            .find_piece(newera_core::FurnitureId(2))
            .unwrap()
            .clone();
        renamed.name = "gabinete".into();
        let body = serde_json::json!({
            "commands": [{"op": "update", "element": newera_core::Element::Furniture(renamed)}]
        });
        let app = router(document.clone(), DEFAULT_ADDR, CancellationToken::new());
        let response = app
            .oneshot(
                Request::post("/api/commands")
                    .header("content-type", "application/json")
                    .body(Body::from(body.to_string()))
                    .unwrap(),
            )
            .await
            .unwrap();
        let status = response.status();
        let text = to_bytes(response.into_body(), usize::MAX).await.unwrap();
        assert_eq!(status, StatusCode::OK, "{}", String::from_utf8_lossy(&text));
        let doc = document.read();
        assert_eq!(
            doc.home()
                .find_piece(newera_core::FurnitureId(2))
                .unwrap()
                .name,
            "gabinete"
        );
        assert_eq!(doc.home().furniture.len(), 1, "still a part of its group");
    }

    #[tokio::test]
    async fn a_batch_that_switches_annotations_off_says_so() {
        let document = SharedDocument::new(Document::default());
        document
            .write()
            .execute(Command::SetAnnotations {
                annotations: newera_core::PlanAnnotations {
                    references: true,
                    ..Default::default()
                },
            })
            .unwrap();
        let app = router(document, DEFAULT_ADDR, CancellationToken::new());
        let post = |body: &'static str| {
            let app = app.clone();
            async move {
                let response = app
                    .oneshot(
                        Request::post("/api/commands")
                            .header("content-type", "application/json")
                            .body(Body::from(body))
                            .unwrap(),
                    )
                    .await
                    .unwrap();
                let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
                serde_json::from_slice::<serde_json::Value>(&body).unwrap()
            }
        };
        let reply = post(r#"{"commands":[{"op":"set_annotations","annotations":{}}]}"#).await;
        assert_eq!(reply["annotations"]["from"]["references"], true, "{reply}");
        assert_eq!(reply["annotations"]["to"]["references"], false, "{reply}");
        // A batch that leaves them alone says nothing about them.
        let reply = post(r#"{"commands":[{"op":"rename_home","name":"Apto"}]}"#).await;
        assert!(reply.get("annotations").is_none(), "{reply}");
    }

    #[tokio::test]
    async fn token_guards_everything_but_health_and_commands_apply() {
        let document = SharedDocument::default();
        let options = ServerOptions {
            token: Some("s3cret".into()),
            ..ServerOptions::default()
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

    async fn call(app: &Router, request: Request<Body>) -> (StatusCode, serde_json::Value) {
        let response = app.clone().oneshot(request).await.unwrap();
        let status = response.status();
        let bytes = to_bytes(response.into_body(), usize::MAX).await.unwrap();
        let json = serde_json::from_slice(&bytes)
            .unwrap_or_else(|_| serde_json::Value::String(String::from_utf8_lossy(&bytes).into()));
        (status, json)
    }

    fn post_json(uri: &str, body: &serde_json::Value) -> Request<Body> {
        Request::post(uri)
            .header("content-type", "application/json")
            .body(Body::from(body.to_string()))
            .unwrap()
    }

    fn wall_command(id: &str, y: f64) -> serde_json::Value {
        serde_json::json!({"op": "insert", "element": {"kind": "wall", "id": id, "start": [0, y], "end": [300, y], "thickness": 15, "height": 250}})
    }

    #[tokio::test]
    async fn sessions_share_presence_credit_edits_and_reject_stale_ones() {
        let document = SharedDocument::default();
        let app = router(document.clone(), DEFAULT_ADDR, CancellationToken::new());
        let (status, ana) = call(
            &app,
            post_json("/api/sessions", &serde_json::json!({"name": "Ana"})),
        )
        .await;
        assert_eq!(status, StatusCode::OK);
        let (_, bia) = call(
            &app,
            post_json("/api/sessions", &serde_json::json!({"name": "Bia"})),
        )
        .await;
        let ana_id = ana["id"].as_str().unwrap().to_owned();

        let presence = serde_json::json!({"cursor": [120, 40], "selection": ["w1"]});
        let (status, _) = call(
            &app,
            post_json(&format!("/api/sessions/{ana_id}"), &presence),
        )
        .await;
        assert_eq!(status, StatusCode::NO_CONTENT);
        let (status, _) = call(&app, post_json("/api/sessions/s99", &presence)).await;
        assert_eq!(status, StatusCode::NOT_FOUND);
        let (status, _) = call(
            &app,
            post_json(
                &format!("/api/sessions/{ana_id}"),
                &serde_json::json!({"selection": ["nope"]}),
            ),
        )
        .await;
        assert_eq!(status, StatusCode::BAD_REQUEST);

        // Both start from revision 0; Ana's edit lands, Bia's is now stale.
        let edit = |id: &str, session: &str| serde_json::json!({"commands": [wall_command(id, 0.0)], "base_revision": 0, "session": session});
        let (status, body) = call(&app, post_json("/api/commands", &edit("w1", &ana_id))).await;
        assert_eq!(status, StatusCode::OK, "{body}");
        let (status, _) = call(
            &app,
            post_json("/api/commands", &edit("w2", bia["id"].as_str().unwrap())),
        )
        .await;
        assert_eq!(status, StatusCode::CONFLICT);
        assert_eq!(document.read().home().walls.len(), 1);

        let (_, list) = call(
            &app,
            Request::get("/api/sessions").body(Body::empty()).unwrap(),
        )
        .await;
        let sessions = list["sessions"].as_array().unwrap();
        assert_eq!(sessions.len(), 2);
        assert_eq!(sessions[0]["name"], "Ana");
        assert_eq!(sessions[0]["cursor"], serde_json::json!([120.0, 40.0]));
        assert_eq!(sessions[0]["edits"], 1);
        assert_eq!(sessions[1]["edits"], 0);

        let delete = Request::delete(format!("/api/sessions/{ana_id}"))
            .body(Body::empty())
            .unwrap();
        assert_eq!(call(&app, delete).await.0, StatusCode::NO_CONTENT);
        assert_eq!(document.read().sessions().list().len(), 1);
    }

    #[cfg(unix)]
    #[tokio::test(flavor = "multi_thread")]
    async fn plugins_edit_the_home_through_the_api() {
        let root =
            std::env::temp_dir().join(format!("newera-server-plugins-{}", std::process::id()));
        let dir = root.join("parede");
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(
            dir.join("plugin.json"),
            r#"{"name":"parede","title":"Parede de teste","command":["sh","run.sh"]}"#,
        )
        .unwrap();
        // A plugin in plain shell: reads its arguments, posts one command.
        std::fs::write(
            dir.join("run.sh"),
            r#"read -r args
body="{\"session\":\"$NEWERA_SESSION\",\"commands\":[{\"op\":\"insert\",\"element\":{\"kind\":\"wall\",\"id\":\"w9\",\"start\":[0,0],\"end\":[420,0],\"thickness\":15,\"height\":250}}]}"
curl -sf -H 'content-type: application/json' -d "$body" "$NEWERA_URL/api/commands" >/dev/null || exit 2
echo "args=$args"
"#,
        )
        .unwrap();

        let document = SharedDocument::default();
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let shutdown = CancellationToken::new();
        let options = ServerOptions {
            plugin_dirs: vec![root.clone()],
            ..ServerOptions::default()
        };
        let addr = listener.local_addr().unwrap();
        let server = tokio::spawn(serve_listener_with(
            document.clone(),
            listener,
            shutdown.clone(),
            options,
        ));
        let base = format!("http://{addr}");
        let client = |method: &str, path: &str, body: Option<&str>| {
            let mut cmd = std::process::Command::new("curl");
            cmd.args(["-s", "-X", method, &format!("{base}{path}")]);
            if let Some(body) = body {
                cmd.args(["-H", "content-type: application/json", "-d", body]);
            }
            cmd
        };
        // Wait for the server to accept connections.
        for _ in 0..50 {
            if client("GET", "/health", None)
                .output()
                .is_ok_and(|o| o.stdout == b"ok")
            {
                break;
            }
            tokio::time::sleep(std::time::Duration::from_millis(50)).await;
        }
        let list = tokio::task::spawn_blocking({
            let mut c = client("GET", "/api/plugins", None);
            move || c.output().unwrap().stdout
        })
        .await
        .unwrap();
        let list: serde_json::Value = serde_json::from_slice(&list).unwrap();
        assert_eq!(list["plugins"][0]["name"], "parede", "{list}");

        let run = tokio::task::spawn_blocking({
            let mut c = client("POST", "/api/plugins/parede/run", Some(r#"{"n":1}"#));
            move || c.output().unwrap().stdout
        })
        .await
        .unwrap();
        let run: serde_json::Value = serde_json::from_slice(&run).unwrap();
        assert_eq!(run["ok"], true, "{run}");
        assert_eq!(run["stdout"], "args={\"n\":1}\n");
        assert_eq!(
            run["edits"], 1,
            "the edit is credited to the plugin session"
        );
        assert_eq!(document.read().home().walls.len(), 1);
        assert!(
            document.read().sessions().list().is_empty(),
            "its session ends with it"
        );

        let missing = tokio::task::spawn_blocking({
            let mut c = client("POST", "/api/plugins/nada/run", None);
            c.args(["-o", "/dev/null", "-w", "%{http_code}"]);
            move || c.output().unwrap().stdout
        })
        .await
        .unwrap();
        assert_eq!(missing, b"404");
        shutdown.cancel();
        server.await.unwrap().unwrap();
        std::fs::remove_dir_all(root).ok();
    }

    #[tokio::test]
    async fn plan_shows_only_the_current_storey() {
        use newera_core::{Label, LabelId, Level, LevelId};
        let mut home = newera_core::Home::default();
        for (id, name) in [(1, "Planta original"), (2, "Novo layout")] {
            home.levels.push(Level {
                id: LevelId(id),
                name: name.into(),
                ..Level::default()
            });
            home.labels.push(Label {
                id: LabelId(10 + id),
                text: format!("texto do {name}"),
                level: Some(LevelId(id)),
                ..Label::default()
            });
        }
        home.selected_level = Some(LevelId(2));
        let app = router(
            SharedDocument::new(Document::new(home)),
            DEFAULT_ADDR,
            CancellationToken::new(),
        );
        let response = app
            .oneshot(Request::get("/api/plan.svg").body(Body::empty()).unwrap())
            .await
            .unwrap();
        let svg = String::from_utf8(
            to_bytes(response.into_body(), usize::MAX)
                .await
                .unwrap()
                .to_vec(),
        )
        .unwrap();
        assert!(svg.contains("texto do Novo layout"));
        assert!(
            !svg.contains("texto do Planta original"),
            "other storeys stay out"
        );
    }
}
