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
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::Duration;

use anyhow::Context as _;

const SESSION: &str = "mcp-session-id";

/// How long opening a new session may take. Ordinary calls have no limit (a
/// photo can take minutes), but a window that stops answering the replayed
/// `initialize` must not hold every other call behind `Link::reopening`.
const RECOVERY_TIMEOUT: Duration = Duration::from_secs(10);

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
    let link = Arc::new(Link::new(agent, endpoint.clone()));
    let out: Out = Arc::new(Mutex::new(std::io::stdout()));
    let mut calls = Vec::new();
    for line in std::io::stdin().lock().lines() {
        let line = line.context("reading stdin")?;
        if line.trim().is_empty() {
            continue;
        }
        let first = is_initialize(&line) && matches!(link.session(), Ok(None));
        let (link, out) = (link.clone(), out.clone());
        let pass = move || {
            if let Err(err) = link.forward(&line, &out) {
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

/// Where the answers go: stdout, or a buffer in the tests.
type Out = Arc<Mutex<dyn Write + Send>>;

/// The bridge's hold on the window: its session, and the client's own
/// `initialize`, kept to open a new session when the window forgets the old one.
///
/// The window closes a session nobody used for a while (five minutes, rmcp's
/// default), and from then on answers every call that carries it with
/// `404 Not Found`. The protocol says a client that hears that starts a new
/// session; the client here is this bridge, since the one on stdin knows
/// nothing of sessions. So the bridge says `initialize` again, as the client
/// first did, and sends the call once more: an agent that paused to think, or
/// waited for its person to answer, carries on instead of failing every call
/// until it is restarted.
struct Link {
    agent: ureq::Agent,
    /// The same agent with a time limit, for opening a new session.
    recovery: ureq::Agent,
    endpoint: Endpoint,
    session: Mutex<Option<String>>,
    hello: Mutex<Option<String>>,
    /// Held while a new session is opened, so calls that found the old one
    /// gone together open one between them, not one each.
    reopening: Mutex<()>,
}

impl Link {
    fn new(agent: ureq::Agent, endpoint: Endpoint) -> Self {
        let recovery = ureq::Agent::config_builder()
            .http_status_as_error(false)
            .timeout_global(Some(RECOVERY_TIMEOUT))
            .build()
            .into();
        Self {
            agent,
            recovery,
            endpoint,
            session: Mutex::default(),
            hello: Mutex::default(),
            reopening: Mutex::default(),
        }
    }

    fn session(&self) -> anyhow::Result<Option<String>> {
        Ok(lock(&self.session, "session")?.clone())
    }

    /// One message to the window, and whatever it answers back out.
    fn forward(&self, line: &str, out: &Out) -> anyhow::Result<()> {
        let hello = is_initialize(line);
        if hello {
            *lock(&self.hello, "initialize")? = Some(line.to_owned());
        }
        let sent_with = self.session()?;
        let mut response = self.send(&self.agent, line, sent_with.as_deref())?;
        if response.status().as_u16() == 404 && sent_with.is_some() && !hello {
            drop(response);
            self.reopen(sent_with.as_deref())?;
            response = self.send(&self.agent, line, self.session()?.as_deref())?;
        }
        deliver(response, out)
    }

    fn send(
        &self,
        agent: &ureq::Agent,
        line: &str,
        session: Option<&str>,
    ) -> anyhow::Result<ureq::http::Response<ureq::Body>> {
        let mut request = agent
            .post(&self.endpoint.url)
            .header("content-type", "application/json")
            .header("accept", "application/json, text/event-stream");
        if let Some(id) = session {
            request = request.header(SESSION, id);
        }
        if let Some(token) = &self.endpoint.token {
            request = request.header("authorization", format!("Bearer {token}"));
        }
        let response = request.send(line).context("the window stopped answering")?;
        if let Some(id) = response
            .headers()
            .get(SESSION)
            .and_then(|v| v.to_str().ok())
        {
            *lock(&self.session, "session")? = Some(id.to_owned());
        }
        Ok(response)
    }

    /// A new session in place of `stale`, introduced the way the client
    /// introduced the first one. Its answer to `initialize` is the bridge's
    /// own business: the client already had one and is not waiting for another.
    fn reopen(&self, stale: Option<&str>) -> anyhow::Result<()> {
        let _one_at_a_time = lock(&self.reopening, "reopening")?;
        if self.session()?.as_deref() != stale {
            return Ok(()); // another call already opened the new one
        }
        let hello = lock(&self.hello, "initialize")?
            .clone()
            .context("the window closed the session, and there is no initialize to open another")?;
        tracing::info!("window bridge: the window closed the session; opening a new one");
        let mut answer = self.send(&self.recovery, &hello, None)?;
        let status = answer.status();
        answer
            .body_mut()
            .read_to_string()
            .context("reading the window's answer to initialize")?;
        if !status.is_success() || self.session()?.as_deref() == stale {
            anyhow::bail!("the window closed the session and did not open another ({status})");
        }
        let session = self.session()?;
        let ready = r#"{"jsonrpc":"2.0","method":"notifications/initialized"}"#;
        let mut answer = self.send(&self.recovery, ready, session.as_deref())?;
        answer
            .body_mut()
            .read_to_string()
            .context("reading the window's answer to notifications/initialized")?;
        Ok(())
    }
}

/// A lock another thread panicked while holding is an error for this call,
/// answered to the client like any other, not a panic of the bridge.
fn lock<'a, T>(m: &'a Mutex<T>, what: &str) -> anyhow::Result<MutexGuard<'a, T>> {
    m.lock()
        .map_err(|_| anyhow::anyhow!("the bridge's {what} lock is poisoned"))
}

fn is_initialize(line: &str) -> bool {
    serde_json::from_str::<serde_json::Value>(line)
        .is_ok_and(|message| message.get("method").and_then(|m| m.as_str()) == Some("initialize"))
}

/// The window's answer, written out one JSON-RPC message per line.
fn deliver(mut response: ureq::http::Response<ureq::Body>, out: &Out) -> anyhow::Result<()> {
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

fn write_line(message: &str, out: &Out) {
    let mut out = out.lock().expect("lock");
    let _ = writeln!(out, "{message}");
    let _ = out.flush();
}

/// A request the window could not answer still gets an answer, or the client
/// would wait for it forever. Notifications (no id) get none, by the protocol.
fn answer_error(line: &str, why: &str, out: &Out) {
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

    /// A window that knows one session at a time, as rmcp does once it has
    /// closed an idle one: a call with any other session gets `404`, and each
    /// `initialize` opens the next session, `s1`, `s2`… It answers every
    /// request it takes with an empty result under the request's id.
    fn forgetful_window() -> (Endpoint, Arc<Mutex<Vec<String>>>) {
        use std::io::Read as _;
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let url = format!("http://{}/mcp", listener.local_addr().unwrap());
        let seen: Arc<Mutex<Vec<String>>> = Arc::default();
        let log = seen.clone();
        std::thread::spawn(move || {
            let mut opened = 0;
            for stream in listener.incoming() {
                let mut stream = stream.unwrap();
                let mut reader = BufReader::new(stream.try_clone().unwrap());
                let (mut session, mut length) = (None, 0);
                loop {
                    let mut header = String::new();
                    reader.read_line(&mut header).unwrap();
                    let header = header.trim_end();
                    if header.is_empty() {
                        break;
                    }
                    let (name, value) = header.split_once(':').unwrap_or((header, ""));
                    match name.to_ascii_lowercase().as_str() {
                        SESSION => session = Some(value.trim().to_owned()),
                        "content-length" => length = value.trim().parse().unwrap(),
                        _ => {}
                    }
                }
                let mut body = vec![0; length];
                reader.read_exact(&mut body).unwrap();
                let message: serde_json::Value = serde_json::from_slice(&body).unwrap();
                let method = message["method"].as_str().unwrap_or_default().to_owned();
                log.lock()
                    .unwrap()
                    .push(format!("{method} {}", session.as_deref().unwrap_or("-")));
                let current = format!("s{opened}");
                let reply = if method == "initialize" {
                    opened += 1;
                    let id = &message["id"];
                    format!(
                        "HTTP/1.1 200 OK\r\ncontent-type: application/json\r\n{SESSION}: s{opened}\r\n\r\n{}",
                        serde_json::json!({"jsonrpc": "2.0", "id": id, "result": {}})
                    )
                } else if session.as_deref() != Some(current.as_str()) {
                    "HTTP/1.1 404 Not Found\r\n\r\nNot Found: Session not found".to_owned()
                } else if message.get("id").is_none() {
                    "HTTP/1.1 202 Accepted\r\n\r\n".to_owned()
                } else {
                    format!(
                        "HTTP/1.1 200 OK\r\ncontent-type: application/json\r\n\r\n{}",
                        serde_json::json!({"jsonrpc": "2.0", "id": message["id"], "result": {}})
                    )
                };
                let reply = reply.replacen("\r\n\r\n", "\r\nconnection: close\r\n\r\n", 1);
                stream.write_all(reply.as_bytes()).unwrap();
            }
        });
        (Endpoint { url, token: None }, seen)
    }

    fn link(endpoint: Endpoint) -> Link {
        let agent: ureq::Agent = ureq::Agent::config_builder()
            .http_status_as_error(false)
            .build()
            .into();
        Link::new(agent, endpoint)
    }

    fn answers(out: &Arc<Mutex<Vec<u8>>>) -> Vec<serde_json::Value> {
        String::from_utf8(out.lock().unwrap().clone())
            .unwrap()
            .lines()
            .map(|l| serde_json::from_str(l).unwrap())
            .collect()
    }

    const HELLO: &str = r#"{"jsonrpc":"2.0","id":0,"method":"initialize","params":{}}"#;
    const CALL: &str = r#"{"jsonrpc":"2.0","id":7,"method":"tools/call","params":{}}"#;

    /// The window closed the session while the agent was quiet: the call still
    /// gets its answer, through a new session the bridge opened for it, and
    /// the client hears nothing of the second `initialize`.
    #[test]
    fn a_closed_session_is_opened_again() {
        let (endpoint, seen) = forgetful_window();
        let link = link(endpoint);
        let buffer: Arc<Mutex<Vec<u8>>> = Arc::default();
        let out: Out = buffer.clone();
        link.forward(HELLO, &out).unwrap();
        assert_eq!(link.session().unwrap().as_deref(), Some("s1"));
        *link.session.lock().unwrap() = Some("closed".to_owned());

        link.forward(CALL, &out).unwrap();

        let answers = answers(&buffer);
        assert_eq!(answers.len(), 2, "{answers:?}");
        assert_eq!(answers[1]["id"], 7);
        assert!(answers[1].get("result").is_some(), "{answers:?}");
        assert_eq!(link.session().unwrap().as_deref(), Some("s2"));
        assert_eq!(
            *seen.lock().unwrap(),
            [
                "initialize -",
                "tools/call closed",
                "initialize -",
                "notifications/initialized s2",
                "tools/call s2",
            ]
        );
    }

    /// Without the client's own `initialize` there is nothing to open a new
    /// session with: the `404` comes back as the error it is.
    #[test]
    fn no_initialize_no_new_session() {
        let (endpoint, _) = forgetful_window();
        let link = link(endpoint);
        let out: Out = Arc::new(Mutex::new(Vec::new()));
        *link.session.lock().unwrap() = Some("closed".to_owned());
        let err = link.forward(CALL, &out).unwrap_err();
        assert!(format!("{err:#}").contains("no initialize"), "{err:#}");
    }
}
