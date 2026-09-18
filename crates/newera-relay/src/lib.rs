//! The editor runs in a browser tab; an AI speaks MCP over HTTP. A tab cannot
//! listen on a port, so the two cannot meet — this stands between them.
//!
//! A tab asks for a **room** and gets back two secrets: one it keeps (to hold
//! its socket) and one the person pastes into their AI client. The AI then
//! talks MCP to this service, and every tool call is handed down the socket to
//! that one tab, which does the work in the project on screen and answers.
//!
//! What this does not do is as important as what it does:
//!
//! - **It stores no project.** Messages pass through; the home, the walls, the
//!   photos live in the tab and nowhere else. Kill this service and nobody
//!   loses a drawing.
//! - **Rooms do not meet.** A room is reachable only by its own secrets, which
//!   are 128 bits of randomness each and never derived from anything. There is
//!   no listing, no enumeration, no "rooms of this user": the id is the
//!   capability, and one room's socket cannot be addressed from another.
//! - **It forgets.** A room dies with its tab (after a short grace, so a
//!   reload is not a new room) and, failing that, after going quiet.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use axum::Router;
use axum::extract::ws::{Message, WebSocket, WebSocketUpgrade};
use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use futures_util::{SinkExt, StreamExt};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::fmt::Write as _;
use tokio::sync::{mpsc, oneshot};

/// How long a room outlives the tab that made it, so a reload keeps the
/// address the person already pasted into their AI.
const GRACE: Duration = Duration::from_secs(120);

/// A room nobody has touched for this long is gone.
const IDLE: Duration = Duration::from_mins(360);

/// Longest a tool may take before the AI is told it timed out. Renders and
/// photos are slow on purpose; an agent waiting forever is worse.
const TOOL_TIMEOUT: Duration = Duration::from_secs(180);

/// How often the tab is pinged. Cloudflare closes a WebSocket that says
/// nothing for about a hundred seconds, and a tab whose socket was closed
/// under it looks, from the outside, exactly like a tab that went away: the
/// address stops answering while somebody is still looking at the editor.
const KEEPALIVE: Duration = Duration::from_secs(30);

/// The protocol version this speaks when a client does not name one.
const PROTOCOL: &str = "2025-06-18";

/// The largest message an AI may send. Images go in over this route (a scanned
/// plan for `set_background`), so it is generous — and finite, because a body
/// nobody bounds is a way to take the machine down with one request.
const MAX_BODY: usize = 24 * 1024 * 1024;

/// Rooms one address may open in an hour. A room costs little, but nothing
/// stops a script from asking for millions unless something does.
const ROOMS_PER_HOUR: usize = 60;

/// Rooms this process holds at once, whoever asked.
const MAX_ROOMS: usize = 5_000;

/// Calls one room may have in flight. An agent that floods its own window
/// hurts only itself, but memory here is shared by everyone.
const MAX_IN_FLIGHT: usize = 16;

/// Where the editor is served from. Anything else is refused at the browser's
/// own gate: the secrets are the real lock, this is the outer door.
const ORIGINS: [&str; 6] = [
    "https://3dneweraai.com",
    "https://www.3dneweraai.com",
    // Where the editor is served while it is being worked on and checked.
    "http://127.0.0.1:8801",
    "http://localhost:8801",
    "http://127.0.0.1:8790",
    "http://localhost:8790",
];

/// What the tab is asked to do, and what it answers.
#[derive(Debug, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
enum ToTab {
    /// Run a tool and answer with `id`.
    Call { id: u64, name: String, args: Value },
    /// An AI client finished the handshake — the window says so on screen.
    Client {
        name: String,
        version: Option<String>,
    },
}

#[derive(Debug, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
enum FromTab {
    /// Sent once, on connecting: the tools this window offers.
    Hello { tools: Value },
    /// The answer to a call.
    Result {
        id: u64,
        #[serde(default)]
        ok: bool,
        #[serde(default)]
        result: Value,
        #[serde(default)]
        error: Option<String>,
    },
}

/// One tab, and the calls it still owes answers for.
#[derive(Debug)]
struct Room {
    /// Secret the tab shows to take (or retake) the socket.
    tab_key: String,
    /// Secret in the URL the person pastes into their AI client.
    client_token: String,
    /// Where to send work, while a tab is connected.
    outbox: Option<mpsc::UnboundedSender<ToTab>>,
    /// The tools this tab said it has, from its `hello`.
    tools: Value,
    /// Calls in flight, waiting on the tab.
    waiting: HashMap<u64, oneshot::Sender<Result<Value, String>>>,
    next_call: u64,
    /// When the tab last went away, for the grace period.
    orphaned_at: Option<Instant>,
    touched: Instant,
}

impl Room {
    fn new(tab_key: String, client_token: String) -> Self {
        Self {
            tab_key,
            client_token,
            outbox: None,
            tools: json!([]),
            waiting: HashMap::new(),
            next_call: 0,
            orphaned_at: Some(Instant::now()),
            touched: Instant::now(),
        }
    }

    fn expired(&self, now: Instant) -> bool {
        now.duration_since(self.touched) > IDLE
            || self
                .orphaned_at
                .is_some_and(|gone| now.duration_since(gone) > GRACE)
    }
}

/// Every room this process is holding, and who has been asking for them.
#[derive(Debug, Clone, Default)]
pub struct Rooms(Arc<Mutex<Held>>);

/// What the service keeps in memory. Nothing here outlives the process, and
/// none of it is anybody's drawing.
#[derive(Debug, Default)]
struct Held {
    rooms: HashMap<String, Room>,
    /// When each address last opened rooms, to keep one from opening all of
    /// them. Trimmed as it is read; nothing is kept for longer than an hour.
    asked: HashMap<String, Vec<Instant>>,
}

/// Compares two secrets without telling anyone, through timing, how much of
/// one was right.
fn same_secret(a: &str, b: &str) -> bool {
    let (a, b) = (a.as_bytes(), b.as_bytes());
    if a.len() != b.len() {
        return false;
    }
    a.iter().zip(b).fold(0u8, |seen, (x, y)| seen | (x ^ y)) == 0
}

/// 128 bits of randomness as hex — an id nobody guesses and nothing derives.
fn secret() -> String {
    let mut bytes = [0u8; 16];
    getrandom::fill(&mut bytes).expect("the system has randomness");
    bytes.iter().fold(String::with_capacity(32), |mut hex, b| {
        let _ = write!(hex, "{b:02x}");
        hex
    })
}

impl Rooms {
    /// Opens a room for a tab. The two secrets are the whole security model:
    /// one holds the socket, the other is what an AI needs to reach it.
    ///
    /// `caller` is whoever asked, as far as the network knows — one address
    /// may not open rooms without end.
    fn open(&self, caller: &str) -> Result<(String, String, String), &'static str> {
        self.sweep();
        let (id, tab_key, client_token) = (secret(), secret(), secret());
        let now = Instant::now();
        let mut held = self.0.lock().expect("rooms");
        if held.rooms.len() >= MAX_ROOMS {
            return Err("this relay is full");
        }
        let hour = Duration::from_secs(3600);
        let asked = held.asked.entry(caller.to_owned()).or_default();
        asked.retain(|at| now.duration_since(*at) < hour);
        if asked.len() >= ROOMS_PER_HOUR {
            return Err("too many rooms from here in the last hour");
        }
        asked.push(now);
        held.rooms
            .insert(id.clone(), Room::new(tab_key.clone(), client_token.clone()));
        Ok((id, tab_key, client_token))
    }

    /// Drops rooms whose tab is gone for good, rooms nobody uses, and the
    /// memory of who asked for what.
    fn sweep(&self) {
        let now = Instant::now();
        let hour = Duration::from_secs(3600);
        let mut held = self.0.lock().expect("rooms");
        held.rooms.retain(|_, room| !room.expired(now));
        held.asked.retain(|_, times| {
            times.retain(|at| now.duration_since(*at) < hour);
            !times.is_empty()
        });
    }

    /// How many rooms are held right now — for `/health`, not for listing.
    fn count(&self) -> usize {
        self.sweep();
        self.0.lock().expect("rooms").rooms.len()
    }
}

/// The service: the tab's socket, the AI's MCP endpoint, and a health check.
pub fn router() -> Router {
    router_with(Rooms::default())
}

/// The same, over rooms you already hold (the tests want to look inside).
pub fn router_with(rooms: Rooms) -> Router {
    // Only the editor's own pages may ask a browser to call this; everything
    // else is a script, and a script has no business opening rooms for a tab
    // that is not there. The secrets are the lock — this is the outer door.
    let origins = ORIGINS
        .iter()
        .filter_map(|origin| origin.parse::<axum::http::HeaderValue>().ok())
        .collect::<Vec<_>>();
    let cors = tower_http::cors::CorsLayer::new()
        .allow_origin(origins)
        .allow_methods([axum::http::Method::GET, axum::http::Method::POST])
        .allow_headers([
            axum::http::header::CONTENT_TYPE,
            axum::http::header::ACCEPT,
            axum::http::HeaderName::from_static("mcp-protocol-version"),
            axum::http::HeaderName::from_static("mcp-session-id"),
        ]);
    Router::new()
        .route("/health", get(health))
        .route("/rooms", post(open_room))
        .route("/r/{room}/tab", get(tab_socket))
        .route("/r/{room}/{token}/mcp", post(mcp).get(no_stream))
        .layer(axum::extract::DefaultBodyLimit::max(MAX_BODY))
        .layer(cors)
        .with_state(rooms)
}

async fn health(State(rooms): State<Rooms>) -> Response {
    axum::Json(json!({"ok": true, "rooms": rooms.count()})).into_response()
}

/// A tab asking for somewhere to be reached.
#[derive(Debug, Serialize)]
struct Opened {
    room: String,
    tab_key: String,
    /// What the person pastes into their AI client — relative, because only
    /// the caller knows the name this service answers to.
    mcp_path: String,
    tab_path: String,
}

async fn open_room(State(rooms): State<Rooms>, headers: axum::http::HeaderMap) -> Response {
    // This runs behind a tunnel, so the peer address is the tunnel's: the
    // forwarded header is what tells one caller from another. It is not proof
    // of anything — it only has to be steady enough to count against, and the
    // ceiling below it holds even when everyone looks the same.
    let who = headers
        .get("x-forwarded-for")
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.split(',').next())
        .map(str::trim)
        .filter(|v| !v.is_empty())
        .map_or_else(|| "unknown".to_owned(), str::to_owned);
    let (room, tab_key, client_token) = match rooms.open(&who) {
        Ok(opened) => opened,
        Err(why) => return (StatusCode::TOO_MANY_REQUESTS, why).into_response(),
    };
    axum::Json(Opened {
        mcp_path: format!("/r/{room}/{client_token}/mcp"),
        tab_path: format!("/r/{room}/tab?key={tab_key}"),
        room,
        tab_key,
    })
    .into_response()
}

#[derive(Debug, Deserialize)]
struct TabQuery {
    key: String,
}

/// The tab takes its socket. Wrong room or wrong key looks the same from
/// outside: there is nothing to learn by trying.
async fn tab_socket(
    State(rooms): State<Rooms>,
    Path(room): Path<String>,
    axum::extract::Query(query): axum::extract::Query<TabQuery>,
    headers: axum::http::HeaderMap,
    upgrade: WebSocketUpgrade,
) -> Response {
    let origin = headers
        .get(axum::http::header::ORIGIN)
        .and_then(|v| v.to_str().ok());
    if !from_the_editor(origin) {
        return (StatusCode::FORBIDDEN, "not from here").into_response();
    }
    {
        let mut held = rooms.0.lock().expect("rooms");
        let Some(entry) = held.rooms.get_mut(&room) else {
            return (StatusCode::NOT_FOUND, "no such room").into_response();
        };
        if !same_secret(&entry.tab_key, &query.key) {
            return (StatusCode::NOT_FOUND, "no such room").into_response();
        }
        entry.touched = Instant::now();
    }
    upgrade.on_upgrade(move |socket| hold_tab(socket, rooms, room))
}

/// Whether a socket handshake came from a page allowed to hold a tab.
///
/// A handshake is not a fetch: the browser sends it whatever CORS says, so the
/// gate has to be here. Something with no origin at all is not a browser — a
/// script, a check, somebody's own tool — and for those the key is the lock.
fn from_the_editor(origin: Option<&str>) -> bool {
    origin.is_none_or(|origin| ORIGINS.contains(&origin))
}

/// Pumps one tab's socket: work down, answers up, until it goes away.
async fn hold_tab(socket: WebSocket, rooms: Rooms, room: String) {
    let (mut sink, mut stream) = socket.split();
    let (tx, mut rx) = mpsc::unbounded_channel::<ToTab>();
    {
        let mut held = rooms.0.lock().expect("rooms");
        let Some(entry) = held.rooms.get_mut(&room) else {
            return;
        };
        entry.outbox = Some(tx);
        entry.orphaned_at = None;
        entry.touched = Instant::now();
    }
    let writing = tokio::spawn(async move {
        let mut beat = tokio::time::interval(KEEPALIVE);
        beat.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
        beat.tick().await; // the first tick is immediate; the socket is new
        loop {
            tokio::select! {
                message = rx.recv() => {
                    let Some(message) = message else { break };
                    let text = serde_json::to_string(&message).unwrap_or_default();
                    if sink.send(Message::text(text)).await.is_err() {
                        break;
                    }
                }
                _ = beat.tick() => {
                    // Silence is what gets a socket closed in the middle; the
                    // browser answers this without the page knowing.
                    if sink.send(Message::Ping(Vec::new().into())).await.is_err() {
                        break;
                    }
                }
            }
        }
    });

    while let Some(Ok(message)) = stream.next().await {
        let Message::Text(text) = message else {
            continue;
        };
        let Ok(from) = serde_json::from_str::<FromTab>(&text) else {
            continue;
        };
        let mut held = rooms.0.lock().expect("rooms");
        let Some(entry) = held.rooms.get_mut(&room) else {
            break;
        };
        entry.touched = Instant::now();
        match from {
            FromTab::Hello { tools } => entry.tools = tools,
            FromTab::Result {
                id,
                ok,
                result,
                error,
            } => {
                if let Some(waiting) = entry.waiting.remove(&id) {
                    let _ = waiting.send(if ok {
                        Ok(result)
                    } else {
                        Err(error.unwrap_or_else(|| "the window did not say why".into()))
                    });
                }
            }
        }
    }

    writing.abort();
    let mut held = rooms.0.lock().expect("rooms");
    if let Some(entry) = held.rooms.get_mut(&room) {
        entry.outbox = None;
        entry.orphaned_at = Some(Instant::now());
        // Whoever was waiting is not going to be answered by this socket.
        for (_, waiting) in entry.waiting.drain() {
            let _ = waiting.send(Err("the editor's tab went away".into()));
        }
    }
}

/// MCP over HTTP: what an AI client talks to.
async fn mcp(
    State(rooms): State<Rooms>,
    Path((room, token)): Path<(String, String)>,
    body: String,
) -> Response {
    let message: Value = match serde_json::from_str(&body) {
        Ok(value) => value,
        Err(err) => return jsonrpc_error(&Value::Null, -32700, &format!("bad JSON: {err}")),
    };
    // A batch is a list; answer each in turn, keeping only what has an id.
    let messages = match &message {
        Value::Array(list) => list.clone(),
        one => vec![one.clone()],
    };
    let mut answers = Vec::new();
    for one in messages {
        if let Some(answer) = answer(&rooms, &room, &token, &one).await {
            answers.push(answer);
        }
    }
    match answers.len() {
        // Notifications only: the protocol wants nothing back.
        0 => StatusCode::ACCEPTED.into_response(),
        1 => axum::Json(answers.remove(0)).into_response(),
        _ => axum::Json(Value::Array(answers)).into_response(),
    }
}

/// This endpoint has no server-to-client stream: the spec's own answer for
/// that is 405, and clients carry on over POST.
async fn no_stream() -> Response {
    StatusCode::METHOD_NOT_ALLOWED.into_response()
}

/// One JSON-RPC message in, at most one out.
async fn answer(rooms: &Rooms, room: &str, token: &str, message: &Value) -> Option<Value> {
    let id = message.get("id").cloned().unwrap_or(Value::Null);
    let method = message.get("method").and_then(Value::as_str).unwrap_or("");
    let params = message.get("params").cloned().unwrap_or(Value::Null);
    let notification = message.get("id").is_none();

    // Every method below needs the room, and a wrong token must not tell
    // anyone whether the room exists.
    let known = {
        let mut held = rooms.0.lock().expect("rooms");
        match held.rooms.get_mut(room) {
            Some(entry) if same_secret(&entry.client_token, token) => {
                entry.touched = Instant::now();
                true
            }
            _ => false,
        }
    };
    if !known {
        return (!notification).then(|| {
            error_body(
                &id,
                -32001,
                "this address is not open — the tab that made it is gone",
            )
        });
    }

    match method {
        "initialize" => {
            let version = params
                .get("protocolVersion")
                .and_then(Value::as_str)
                .unwrap_or(PROTOCOL)
                .to_owned();
            // The window shows who just connected, by name.
            if let Some(client) = params.get("clientInfo") {
                let name = client
                    .get("name")
                    .and_then(Value::as_str)
                    .unwrap_or("MCP")
                    .to_owned();
                let told = ToTab::Client {
                    name,
                    version: client
                        .get("version")
                        .and_then(Value::as_str)
                        .map(str::to_owned),
                };
                let held = rooms.0.lock().expect("rooms");
                if let Some(entry) = held.rooms.get(room)
                    && let Some(outbox) = &entry.outbox
                {
                    let _ = outbox.send(told);
                }
            }
            Some(json!({
                "jsonrpc": "2.0",
                "id": id,
                "result": {
                    "protocolVersion": version,
                    "capabilities": { "tools": {} },
                    "serverInfo": {
                        "name": "3d-new-era-ai",
                        "title": "3D New Era AI (browser)",
                        "version": env!("CARGO_PKG_VERSION"),
                    },
                    "instructions": INSTRUCTIONS,
                }
            }))
        }
        "notifications/initialized" | "notifications/cancelled" => None,
        "ping" => Some(json!({"jsonrpc": "2.0", "id": id, "result": {}})),
        "tools/list" => {
            let tools = {
                let held = rooms.0.lock().expect("rooms");
                held.rooms.get(room).map(|entry| entry.tools.clone())
            };
            Some(json!({
                "jsonrpc": "2.0",
                "id": id,
                "result": { "tools": tools.unwrap_or_else(|| json!([])) }
            }))
        }
        "tools/call" => {
            let name = params
                .get("name")
                .and_then(Value::as_str)
                .unwrap_or_default()
                .to_owned();
            let args = params.get("arguments").cloned().unwrap_or(json!({}));
            match hand_to_tab(rooms, room, &name, args).await {
                Ok(result) => Some(json!({"jsonrpc": "2.0", "id": id, "result": result})),
                // A tool that refuses is not a protocol error: the agent is
                // told inside the result, which is what lets it try again.
                Err(why) => Some(json!({
                    "jsonrpc": "2.0",
                    "id": id,
                    "result": {
                        "content": [{"type": "text", "text": why}],
                        "isError": true,
                    }
                })),
            }
        }
        _ if notification => None,
        other => Some(error_body(
            &id,
            -32601,
            &format!("this server does not do {other}"),
        )),
    }
}

/// What the browser's MCP tells an agent about itself. The tools carry their
/// own descriptions; this is the part about where it is.
const INSTRUCTIONS: &str = "\
Home design editor running in someone's browser tab, reached through a relay. \
Units: cm. Plan axes: x right, y down. Everything you change appears on their \
screen as you do it and can be undone with Ctrl+Z. The project lives in that \
tab: it is not saved anywhere here, and closing the tab ends the session.";

/// Sends one call down to the tab and waits for its answer.
async fn hand_to_tab(rooms: &Rooms, room: &str, name: &str, args: Value) -> Result<Value, String> {
    let (tx, rx) = oneshot::channel();
    {
        let mut held = rooms.0.lock().expect("rooms");
        let entry = held
            .rooms
            .get_mut(room)
            .ok_or_else(|| "this address is not open any more".to_owned())?;
        if entry.waiting.len() >= MAX_IN_FLIGHT {
            return Err(format!(
                "this window already has {MAX_IN_FLIGHT} calls in the air — wait for one to answer"
            ));
        }
        let Some(outbox) = entry.outbox.clone() else {
            return Err(
                "the editor's tab is not connected — open the project again in the browser"
                    .to_owned(),
            );
        };
        entry.next_call += 1;
        let id = entry.next_call;
        entry.waiting.insert(id, tx);
        outbox
            .send(ToTab::Call {
                id,
                name: name.to_owned(),
                args,
            })
            .map_err(|_| "the editor's tab went away".to_owned())?;
    }
    match tokio::time::timeout(TOOL_TIMEOUT, rx).await {
        Ok(Ok(result)) => result,
        Ok(Err(_)) => Err("the editor's tab went away".to_owned()),
        Err(_) => Err(format!(
            "the window took longer than {} s to answer",
            TOOL_TIMEOUT.as_secs()
        )),
    }
}

fn error_body(id: &Value, code: i32, message: &str) -> Value {
    json!({"jsonrpc": "2.0", "id": id, "error": {"code": code, "message": message}})
}

fn jsonrpc_error(id: &Value, code: i32, message: &str) -> Response {
    axum::Json(error_body(id, code, message)).into_response()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_room_is_two_secrets_and_neither_is_the_other() {
        let rooms = Rooms::default();
        let (id, tab_key, client_token) = rooms.open("test").expect("a room");
        assert_eq!(id.len(), 32);
        assert_ne!(id, tab_key);
        assert_ne!(tab_key, client_token);
        assert_eq!(rooms.count(), 1);
    }

    /// A room only answers to its own token — and a wrong one learns nothing
    /// about whether the room is there.
    #[tokio::test]
    async fn a_wrong_token_is_turned_away() {
        let rooms = Rooms::default();
        let (id, _tab, token) = rooms.open("test").expect("a room");
        let list = json!({"jsonrpc": "2.0", "id": 1, "method": "tools/list"});
        let good = answer(&rooms, &id, &token, &list).await.expect("an answer");
        assert!(good.get("result").is_some());

        let bad = answer(&rooms, &id, "0".repeat(32).as_str(), &list)
            .await
            .expect("an answer");
        assert_eq!(bad["error"]["code"], -32001);

        let nowhere = answer(&rooms, &"f".repeat(32), &token, &list)
            .await
            .expect("an answer");
        assert_eq!(
            nowhere["error"]["message"], bad["error"]["message"],
            "a room that is not there and a wrong token must read the same"
        );
    }

    /// With nobody on the other end, a call fails in words an agent can act
    /// on, and not by hanging.
    #[tokio::test]
    async fn a_call_without_a_tab_says_so() {
        let rooms = Rooms::default();
        let (id, _tab, token) = rooms.open("test").expect("a room");
        let call = json!({
            "jsonrpc": "2.0", "id": 7, "method": "tools/call",
            "params": {"name": "get_home", "arguments": {}}
        });
        let answer = answer(&rooms, &id, &token, &call).await.expect("an answer");
        assert_eq!(answer["result"]["isError"], true);
        let text = answer["result"]["content"][0]["text"].as_str().unwrap();
        assert!(text.contains("not connected"), "{text}");
    }

    /// The handshake answers like a server, and says what it is.
    #[tokio::test]
    async fn it_shakes_hands() {
        let rooms = Rooms::default();
        let (id, _tab, token) = rooms.open("test").expect("a room");
        let hello = json!({
            "jsonrpc": "2.0", "id": 1, "method": "initialize",
            "params": {
                "protocolVersion": "2025-06-18",
                "capabilities": {},
                "clientInfo": {"name": "claude-code", "version": "2.0.0"}
            }
        });
        let answer = answer(&rooms, &id, &token, &hello)
            .await
            .expect("an answer");
        assert_eq!(answer["result"]["protocolVersion"], "2025-06-18");
        assert_eq!(answer["result"]["serverInfo"]["name"], "3d-new-era-ai");
        assert!(answer["result"]["capabilities"]["tools"].is_object());
    }

    /// One address cannot take the whole relay: after the hour's worth of
    /// rooms, it is told no — and somebody else is not.
    #[test]
    fn one_caller_cannot_open_every_room() {
        let rooms = Rooms::default();
        for _ in 0..ROOMS_PER_HOUR {
            rooms.open("203.0.113.7").expect("within the hour's share");
        }
        assert!(rooms.open("203.0.113.7").is_err(), "the share runs out");
        rooms
            .open("198.51.100.4")
            .expect("somebody else is not affected");
    }

    /// Secrets are compared in a way that does not leak how much was right.
    #[test]
    fn secrets_are_compared_whole() {
        assert!(same_secret("abc", "abc"));
        assert!(!same_secret("abc", "abd"));
        assert!(!same_secret("abc", "abcd"));
        assert!(!same_secret("", "a"));
    }

    /// The socket is not open to any page that asks — CORS does not apply to a
    /// WebSocket handshake, so the origin is checked where it arrives.
    #[test]
    fn a_socket_from_somewhere_else_is_refused() {
        assert!(from_the_editor(Some("https://3dneweraai.com")));
        assert!(from_the_editor(Some("http://127.0.0.1:8801")));
        assert!(!from_the_editor(Some("https://evil.example")));
        assert!(
            !from_the_editor(Some("https://3dneweraai.com.evil.example")),
            "a name that merely starts the same is somewhere else"
        );
        assert!(
            from_the_editor(None),
            "not a browser: the key is the lock there"
        );
    }

    /// A notification is not answered at all, which is what the protocol says.
    #[tokio::test]
    async fn a_notification_gets_no_answer() {
        let rooms = Rooms::default();
        let (id, _tab, token) = rooms.open("test").expect("a room");
        let note = json!({"jsonrpc": "2.0", "method": "notifications/initialized"});
        assert!(answer(&rooms, &id, &token, &note).await.is_none());
    }
}
