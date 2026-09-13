//! MCP server for 3D New Era AI.
//!
//! Tools are thin adapters: they translate MCP parameters into
//! [`newera_core::Command`]s and execute them on the [`SharedDocument`]
//! that the desktop UI is also rendering. Whatever an agent does shows up on
//! screen immediately and can be undone with Ctrl+Z.

mod tools;

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
