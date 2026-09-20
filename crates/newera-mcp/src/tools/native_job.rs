//! Bounded native rendering with transport liveness and cooperative cancellation.
use std::sync::{Arc, Mutex};
use std::time::Duration;

use rmcp::model::{
    CallToolRequestParams, CallToolResponse, PingRequest, ProgressNotificationParam, ServerRequest,
};
use rmcp::service::{PeerRequestOptions, RequestContext};
use rmcp::{ErrorData, RoleServer};
use tokio_util::sync::CancellationToken;

use super::{NewEraMcp, reply::invalid};

static RENDER: tokio::sync::Semaphore = tokio::sync::Semaphore::const_new(1);

#[derive(Default)]
struct State {
    phase: String,
    done: u64,
    total: u64,
    progress: u64,
}
struct Watcher {
    cancel: CancellationToken,
    state: Mutex<State>,
}
impl newera_core::progress::Watcher for Watcher {
    fn step(&self, phase: &str, done: u64, total: u64) {
        let mut s = self
            .state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let previous = if s.phase == phase { s.done } else { 0 };
        s.progress = s.progress.saturating_add(done.saturating_sub(previous));
        s.done = if s.phase == phase {
            s.done.max(done)
        } else {
            done
        };
        phase.clone_into(&mut s.phase);
        s.total = total;
    }
    fn cancelled(&self) -> bool {
        self.cancel.is_cancelled()
    }
}
struct CancelOnDrop(CancellationToken);
impl Drop for CancelOnDrop {
    fn drop(&mut self) {
        self.0.cancel();
    }
}

pub(super) async fn run(
    server: NewEraMcp,
    request: CallToolRequestParams,
    context: RequestContext<RoleServer>,
) -> Result<CallToolResponse, ErrorData> {
    let permit = RENDER.try_acquire().map_err(|_| {
        invalid("Uma renderização já está em andamento; aguarde ou cancele antes de iniciar outra.")
    })?;
    let handle = tokio::runtime::Handle::current();
    let work_context = context.clone();
    supervise(
        context,
        Duration::from_secs(20),
        Duration::from_secs(10),
        move || {
            // Keep the permit until the actual worker has stopped, even if the
            // client abandoned its async request earlier.
            let _permit = permit;
            handle.block_on(async {
                let call = rmcp::handler::server::tool::ToolCallContext::new(
                    &server,
                    request,
                    work_context,
                );
                server.tool_router.call(call).await
            })
        },
    )
    .await
}

async fn supervise(
    context: RequestContext<RoleServer>,
    heartbeat: Duration,
    response_timeout: Duration,
    work: impl FnOnce() -> Result<CallToolResponse, ErrorData> + Send + 'static,
) -> Result<CallToolResponse, ErrorData> {
    let cancel = context.ct.child_token();
    let _cancel_on_drop = CancelOnDrop(cancel.clone());
    let watcher = Arc::new(Watcher {
        cancel: cancel.clone(),
        state: Mutex::new(State::default()),
    });
    let listener: Arc<dyn newera_core::progress::Watcher> = watcher.clone();
    let mut worker = tokio::task::spawn_blocking(move || {
        if listener.cancelled() {
            return Err(invalid(newera_core::progress::CANCELLED));
        }
        let result = newera_core::progress::watched(&listener, work);
        if listener.cancelled() {
            Err(invalid(newera_core::progress::CANCELLED))
        } else {
            result
        }
    });
    let token = context.meta.get_progress_token();
    let mut poll = tokio::time::interval(Duration::from_millis(250));
    let mut next_ping = tokio::time::Instant::now() + heartbeat;
    let mut next_progress = tokio::time::Instant::now();
    loop {
        tokio::select! {
            result = &mut worker => return result.map_err(|e| invalid(format!("Falha na renderização: {e}")))?,
            () = cancel.cancelled() => return Err(invalid(newera_core::progress::CANCELLED)),
            _ = poll.tick() => {
                if context.peer.is_transport_closed() { return Err(invalid("Cliente desconectado; renderização cancelada.")); }
                let now = tokio::time::Instant::now();
                if now >= next_ping {
                    // Ping is part of MCP even when no progress token was
                    // supplied. Require a reply: notifications alone could
                    // keep an abandoned HTTP session alive indefinitely.
                    let ping = async {
                        context.peer.send_request_with_option(
                            ServerRequest::PingRequest(PingRequest { method: rmcp::model::PingRequestMethod, extensions: rmcp::model::Extensions::default() }),
                            PeerRequestOptions::with_timeout(response_timeout),
                        ).await?.await_response().await
                    };
                    tokio::select! {
                        result = &mut worker => return result.map_err(|e| invalid(format!("Falha na renderização: {e}")))?,
                        () = cancel.cancelled() => return Err(invalid(newera_core::progress::CANCELLED)),
                        result = tokio::time::timeout(response_timeout, ping) => {
                            if !matches!(result, Ok(Ok(_))) { return Err(invalid("Cliente não respondeu; renderização cancelada.")); }
                        }
                    }
                    next_ping = tokio::time::Instant::now() + heartbeat;
                }
                if now >= next_progress {
                    if let Some(token) = &token {
                        let notification = {
                            let s = watcher.state.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
                            #[allow(clippy::cast_precision_loss)]
                            let progress = s.progress as f64;
                            let message = if s.total == 0 { s.phase.clone() } else { format!("{}: {}/{}", s.phase, s.done, s.total) };
                            ProgressNotificationParam::new(token.clone(), progress).with_message(message)
                        };
                        if !matches!(tokio::time::timeout(response_timeout, context.peer.notify_progress(notification)).await, Ok(Ok(()))) {
                            return Err(invalid("Falha no envio de progresso; renderização cancelada."));
                        }
                    }
                    next_progress = now + Duration::from_secs(1);
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::{Router, body::Body, http::Request};
    use futures_util::StreamExt;
    use rmcp::ServerHandler;
    use rmcp::transport::streamable_http_server::{
        StreamableHttpServerConfig, StreamableHttpService, session::local::LocalSessionManager,
    };
    use serde_json::{Value, json};
    use std::sync::atomic::{AtomicBool, Ordering};
    use tower::ServiceExt;

    #[derive(Clone)]
    struct SlowServer(Arc<AtomicBool>);
    impl ServerHandler for SlowServer {
        async fn call_tool(
            &self,
            _: CallToolRequestParams,
            context: RequestContext<RoleServer>,
        ) -> Result<CallToolResponse, ErrorData> {
            let stopped = self.0.clone();
            supervise(
                context,
                Duration::from_millis(50),
                Duration::from_millis(400),
                move || {
                    for i in 0..250 {
                        if newera_core::progress::cancelled() {
                            break;
                        }
                        newera_core::progress::step("Test render", i, 250);
                        std::thread::sleep(Duration::from_millis(10));
                    }
                    stopped.store(true, Ordering::SeqCst);
                    Ok(rmcp::model::CallToolResult::success(vec![]).into())
                },
            )
            .await
        }
    }
    async fn post(
        router: &Router,
        session: Option<&str>,
        value: Value,
    ) -> axum::response::Response {
        let mut request = Request::builder()
            .method("POST")
            .uri("/")
            .header("host", "localhost")
            .header("content-type", "application/json")
            .header("accept", "application/json, text/event-stream");
        if let Some(session) = session {
            request = request.header("mcp-session-id", session);
        }
        router
            .clone()
            .oneshot(request.body(Body::from(value.to_string())).unwrap())
            .await
            .unwrap()
    }
    async fn setup() -> (Router, String, Arc<AtomicBool>) {
        let stopped = Arc::new(AtomicBool::new(false));
        let server = SlowServer(stopped.clone());
        let mut sessions = LocalSessionManager::default();
        sessions.session_config.keep_alive = Some(Duration::from_secs(1));
        let service = StreamableHttpService::new(
            move || Ok(server.clone()),
            Arc::new(sessions),
            StreamableHttpServerConfig::default(),
        );
        let router = Router::new().fallback_service(service);
        let response = post(&router, None, json!({"jsonrpc":"2.0","id":1,"method":"initialize","params":{
            "protocolVersion":"2025-03-26","capabilities":{},"clientInfo":{"name":"test","version":"1"}
        }})).await;
        assert!(response.status().is_success());
        let session = response.headers()["mcp-session-id"]
            .to_str()
            .unwrap()
            .to_owned();
        let _ = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        post(
            &router,
            Some(&session),
            json!({"jsonrpc":"2.0","method":"notifications/initialized"}),
        )
        .await;
        (router, session, stopped)
    }
    async fn exercise(mode: &str) {
        let (router, session, stopped) = setup().await;
        let mut params = json!({"name":"slow"});
        if mode != "success" {
            params["_meta"] = json!({"progressToken":"progress"});
        }
        let response = post(
            &router,
            Some(&session),
            json!({"jsonrpc":"2.0","id":2,"method":"tools/call","params":params}),
        )
        .await;
        let mut stream = response.into_body().into_data_stream();
        let mut buffer = String::new();
        let mut pings = 0;
        let mut progress = 0;
        let mut result = None;
        let mut interrupted = false;
        while let Some(chunk) = tokio::time::timeout(Duration::from_secs(6), stream.next())
            .await
            .unwrap()
        {
            buffer.push_str(std::str::from_utf8(&chunk.unwrap()).unwrap());
            while let Some(end) = buffer.find("\n\n") {
                let event: String = buffer.drain(..end + 2).collect();
                let Some(data) = event.lines().find_map(|l| l.strip_prefix("data: ")) else {
                    continue;
                };
                if data.trim().is_empty() {
                    continue;
                }
                let value: Value = serde_json::from_str(data).unwrap();
                if value["method"] == "ping" {
                    pings += 1;
                    if mode != "unresponsive" {
                        post(
                            &router,
                            Some(&session),
                            json!({"jsonrpc":"2.0","id":value["id"],"result":{}}),
                        )
                        .await;
                    }
                } else if value["method"] == "notifications/progress" {
                    progress += 1;
                    if !interrupted
                        && value["params"]["message"]
                            .as_str()
                            .is_some_and(|m| m.starts_with("Test render"))
                    {
                        if mode == "cancel" {
                            post(&router, Some(&session), json!({"jsonrpc":"2.0","method":"notifications/cancelled","params":{"requestId":2,"reason":"test"}})).await;
                            interrupted = true;
                        } else if mode == "disconnect" {
                            let response = router
                                .clone()
                                .oneshot(
                                    Request::builder()
                                        .method("DELETE")
                                        .header("host", "localhost")
                                        .uri("/")
                                        .header("mcp-session-id", &session)
                                        .body(Body::empty())
                                        .unwrap(),
                                )
                                .await
                                .unwrap();
                            assert!(response.status().is_success());
                            interrupted = true;
                        }
                    }
                } else if value["id"] == 2 {
                    result = Some(value);
                }
            }
        }
        if mode == "success" {
            assert!(
                pings >= 2,
                "long request must heartbeat without a progress token"
            );
            assert!(result.unwrap().get("result").is_some());
        } else {
            assert!(progress > 0);
            assert!(result.as_ref().is_none_or(|r| r.get("error").is_some()));
        }
        tokio::time::timeout(Duration::from_secs(1), async {
            while !stopped.load(Ordering::SeqCst) {
                tokio::time::sleep(Duration::from_millis(10)).await;
            }
        })
        .await
        .expect("worker must stop after its client cancels/disconnects/fails ping");
    }
    #[tokio::test]
    async fn http_long_job_survives_short_session_idle_limit_without_progress_token() {
        exercise("success").await;
    }
    #[tokio::test]
    async fn http_cancel_stops_worker() {
        exercise("cancel").await;
    }
    #[tokio::test]
    async fn http_session_deletion_stops_worker() {
        exercise("disconnect").await;
    }
    #[tokio::test]
    async fn http_unresponsive_client_stops_worker() {
        exercise("unresponsive").await;
    }
}
