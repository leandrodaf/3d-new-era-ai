//! MCP server for 3D New Era AI.
//!
//! Tools are thin adapters: they translate MCP parameters into
//! [`newera_core::Command`]s and execute them on the [`SharedDocument`]
//! that the desktop UI is also rendering. Whatever an agent does shows up on
//! screen immediately and can be undone with Ctrl+Z.

mod compact;
mod edit;
mod schema;
mod tools;
mod trace;

use std::sync::Arc;

use newera_core::SharedDocument;
use rmcp::ServiceExt;
use rmcp::transport::streamable_http_server::{
    StreamableHttpServerConfig, StreamableHttpService, session::local::LocalSessionManager,
};
use tokio_util::sync::CancellationToken;

pub use tools::NewEraMcp;

/// Serves MCP over stdin/stdout until the client disconnects.
pub async fn serve_stdio(document: SharedDocument) -> std::io::Result<()> {
    let service = NewEraMcp::new(document)
        .serve(rmcp::transport::stdio())
        .await
        .map_err(std::io::Error::other)?;
    service.waiting().await.map_err(std::io::Error::other)?;
    Ok(())
}

/// Builds a Streamable HTTP MCP service, ready to be mounted on a router.
///
/// `allowed_hosts` guards against DNS rebinding; keep it to loopback names
/// unless the server is intentionally exposed.
pub fn http_service(
    document: SharedDocument,
    allowed_hosts: Vec<String>,
    shutdown: CancellationToken,
) -> StreamableHttpService<NewEraMcp, LocalSessionManager> {
    let mut config = StreamableHttpServerConfig::default();
    config.allowed_hosts = allowed_hosts;
    config.cancellation_token = shutdown;
    StreamableHttpService::new(
        move || Ok(NewEraMcp::new(document.clone())),
        Arc::new(LocalSessionManager::default()),
        config,
    )
}

/// The analyses an agent asks the MCP tools for, without an MCP session:
/// `check_layout`, `ergonomics`, `measure` and `annotations`, with the same
/// arguments and the same answer.
///
/// A script that edits the plan over REST could not ask whether the edit
/// was any good — the questions lived only behind the MCP handshake — so it
/// had to hand the question back to the agent. Unknown names and arguments
/// that do not fit answer `Err` with the reason.
pub fn analysis(
    document: SharedDocument,
    name: &str,
    args: serde_json::Value,
) -> Result<String, String> {
    use rmcp::handler::server::wrapper::Parameters;

    fn params<T: serde::de::DeserializeOwned>(args: serde_json::Value) -> Result<T, String> {
        let args = if args.is_null() {
            serde_json::json!({})
        } else {
            args
        };
        serde_json::from_value(args).map_err(|e| format!("arguments: {e}"))
    }
    let reason = |e: rmcp::ErrorData| e.message.to_string();
    let server = NewEraMcp::new(document);
    match name {
        "check_layout" => server
            .check_layout(Parameters(params(args)?))
            .map_err(reason),
        "ergonomics" => Ok(server.ergonomics(Parameters(params(args)?))),
        "measure" => server.measure(Parameters(params(args)?)).map_err(reason),
        "annotations" => server
            .annotations(Parameters(params(args)?))
            .map_err(reason),
        other => Err(format!(
            "no analysis {other}: check_layout, ergonomics, measure or annotations"
        )),
    }
}
