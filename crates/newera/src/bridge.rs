//! `newera mcp` in front of an open window.
//!
//! Some clients only know how to start a program and talk to it over
//! stdin/stdout — Claude Desktop's one-click extensions among them. Started
//! on its own, that program would edit a project nobody sees. When the editor
//! (or `newera serve`) is already running on this machine, this passes the
//! messages through to its HTTP endpoint instead, so the agent edits the plan
//! on screen, exactly as it would through the URL.
//!
//! Only loopback is ever tried: this never reaches for another machine.

use std::io::{BufRead, BufReader, Write};
use std::net::SocketAddr;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use anyhow::Context as _;

const SESSION: &str = "mcp-session-id";

/// The window's endpoint, if one answers on `addr`.
pub(crate) fn window(addr: SocketAddr, token: Option<&str>) -> Option<Endpoint> {
    if !addr.ip().is_loopback() {
        return None;
    }
    let base = format!("http://{addr}");
    let probe: ureq::Agent = ureq::Agent::config_builder()
        .timeout_global(Some(Duration::from_millis(800)))
        .build()
        .into();
    let mut answer = probe.get(format!("{base}/health")).call().ok()?;
    let body = answer.body_mut().read_to_string().ok()?;
    (body.trim() == "ok").then(|| Endpoint {
        url: format!("{base}/mcp"),
        token: token.map(str::to_owned),
    })
}

/// Where the window listens for MCP.
#[derive(Debug, Clone)]
pub(crate) struct Endpoint {
    url: String,
    token: Option<String>,
}

/// Passes stdin to the window and its answers to stdout, until stdin closes.
///
/// Calls run side by side — a photo that takes a minute does not hold up the
/// cancel that follows it — but `initialize` goes alone and first, because
/// it hands out the session every later call must carry.
pub(crate) fn run(endpoint: &Endpoint) -> anyhow::Result<()> {
    let agent: ureq::Agent = ureq::Agent::config_builder()
        .http_status_as_error(false)
        .timeout_global(None)
        .build()
        .into();
    let out = Arc::new(Mutex::new(std::io::stdout()));
    let session: Arc<Mutex<Option<String>>> = Arc::default();
    let mut calls = Vec::new();
    for line in std::io::stdin().lock().lines() {
        let line = line.context("reading stdin")?;
        if line.trim().is_empty() {
            continue;
        }
        let first = line.contains("\"initialize\"") && session.lock().expect("lock").is_none();
        let (agent, endpoint, out, session) = (
            agent.clone(),
            endpoint.clone(),
            out.clone(),
            session.clone(),
        );
        let pass = move || {
            if let Err(err) = forward(&agent, &endpoint, &line, &session, &out) {
                tracing::warn!("window bridge: {err:#}");
                answer_error(&line, &format!("{err:#}"), &out);
            }
        };
        if first {
            pass();
        } else {
            calls.push(std::thread::spawn(pass));
        }
        calls.retain(|call| !call.is_finished());
    }
    for call in calls {
        let _ = call.join();
    }
    Ok(())
}

/// One message to the window, and whatever it answers back out.
fn forward(
    agent: &ureq::Agent,
    endpoint: &Endpoint,
    line: &str,
    session: &Mutex<Option<String>>,
    out: &Mutex<std::io::Stdout>,
) -> anyhow::Result<()> {
    let mut request = agent
        .post(&endpoint.url)
        .header("content-type", "application/json")
        .header("accept", "application/json, text/event-stream");
    if let Some(id) = session.lock().expect("lock").clone() {
        request = request.header(SESSION, id);
    }
    if let Some(token) = &endpoint.token {
        request = request.header("authorization", format!("Bearer {token}"));
    }
    let mut response = request.send(line).context("the window stopped answering")?;
    if let Some(id) = response
        .headers()
        .get(SESSION)
        .and_then(|v| v.to_str().ok())
    {
        *session.lock().expect("lock") = Some(id.to_owned());
    }
    let status = response.status();
    let events = response
        .headers()
        .get("content-type")
        .and_then(|v| v.to_str().ok())
        .is_some_and(|t| t.starts_with("text/event-stream"));
    let body = BufReader::new(response.body_mut().as_reader());
    if status.as_u16() == 202 {
        return Ok(());
    }
    if !status.is_success() {
        let mut text = String::new();
        for l in body.lines().map_while(Result::ok).take(20) {
            text.push_str(&l);
        }
        anyhow::bail!("the window answered {status}: {text}");
    }
    if events {
        // One JSON-RPC message per `data:` event; the rest is framing.
        let mut data = String::new();
        for l in body.lines() {
            let l = l.context("reading the window's answer")?;
            if let Some(rest) = l.strip_prefix("data:") {
                data.push_str(rest.trim_start());
            } else if l.is_empty() && !data.is_empty() {
                write_line(&std::mem::take(&mut data), out);
            }
        }
        if !data.is_empty() {
            write_line(&data, out);
        }
    } else {
        let text: String = body.lines().map_while(Result::ok).collect();
        if !text.trim().is_empty() {
            write_line(&text, out);
        }
    }
    Ok(())
}

fn write_line(message: &str, out: &Mutex<std::io::Stdout>) {
    let mut out = out.lock().expect("lock");
    let _ = writeln!(out, "{message}");
    let _ = out.flush();
}

/// A request the window could not answer still gets an answer, or the client
/// would wait for it forever. Notifications (no id) get none, by the protocol.
fn answer_error(line: &str, why: &str, out: &Mutex<std::io::Stdout>) {
    let Ok(message) = serde_json::from_str::<serde_json::Value>(line) else {
        return;
    };
    let Some(id) = message.get("id").filter(|id| !id.is_null()) else {
        return;
    };
    let reply = serde_json::json!({
        "jsonrpc": "2.0",
        "id": id,
        "error": {"code": -32603, "message": format!(
            "{why}. Is the 3D New Era AI window still open? Reopen it, or run `newera mcp --standalone`."
        )},
    });
    write_line(&reply.to_string(), out);
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Only this machine is ever tried.
    #[test]
    fn another_machine_is_never_tried() {
        let addr: SocketAddr = "192.0.2.1:7878".parse().unwrap();
        assert!(window(addr, None).is_none());
    }

    /// Nothing listening means no window, quickly.
    #[test]
    fn no_window_when_nothing_answers() {
        let free = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = free.local_addr().unwrap();
        drop(free);
        assert!(window(addr, None).is_none());
    }
}
