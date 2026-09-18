//! The browser's side of the MCP: the tab asks a relay for an address, holds a
//! socket to it, and does the work an AI asks for — in the project on screen.
//!
//! A tab cannot listen on a port, so it cannot be an MCP server the way the
//! desktop is. What it can do is hold one connection outwards. The relay (see
//! `newera-relay`) gives out an address, speaks the protocol to the AI, and
//! hands each tool call down this socket; the answers go back the same way.
//! Nothing of the project is stored there: it passes through.
//!
//! Two tools are kept off the list here. `render_photo` and `video` are the
//! path tracer, which runs for minutes on a CPU — in a tab that means a frozen
//! window, so the browser advertises what it can actually do.

use std::cell::RefCell;
use std::rc::Rc;

use newera_core::SharedDocument;
use wasm_bindgen::JsCast;
use wasm_bindgen::prelude::*;

/// Where the relay lives, unless the address bar says otherwise
/// (`?relay=http://127.0.0.1:7979` while working on it).
const RELAY: &str = "https://mcp.3dneweraai.com";

/// Where the room this tab holds is written down, so a reload walks back into
/// it instead of asking for a new address — the old one is already pasted into
/// somebody's AI client, and losing it on a refresh is losing the setup.
const REMEMBERED: &str = "newera-mcp-room";

/// Tools the browser does not offer: they would hold the window for minutes.
const TOO_SLOW_HERE: [&str; 2] = ["render_photo", "video"];

/// Where this tab is in the business of being reachable.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) enum Link {
    /// Nobody asked for an address yet.
    #[default]
    Off,
    /// Asking the relay for one.
    Opening,
    /// Reachable: this is what goes into the AI client.
    On { url: String },
    /// It did not work, and this is what went wrong.
    Failed { why: String },
}

/// What the window says, and the socket that makes it true.
///
/// The socket is kept beside the state and not inside it: a link that fell
/// over still has one to close, and forgetting that left the old address
/// answering after the switch had been thrown — the relay was still holding a
/// tab nobody had hung up.
#[derive(Debug, Default)]
pub(crate) struct Held {
    pub(crate) link: Link,
    socket: Option<web_sys::WebSocket>,
}

impl Held {
    /// Closes whatever is open, if anything is.
    fn hang_up(&mut self) {
        if let Some(socket) = self.socket.take() {
            let _ = socket.close();
        }
    }
}

/// The link's state, shared with the socket's callbacks.
pub(crate) type Shared = Rc<RefCell<Held>>;

/// The relay this tab should use.
fn relay_base() -> String {
    let from_address = web_sys::window()
        .and_then(|w| w.location().search().ok())
        .and_then(|search| web_sys::UrlSearchParams::new_with_str(&search).ok())
        .and_then(|params| params.get("relay"))
        .filter(|value| !value.trim().is_empty());
    from_address.unwrap_or_else(|| RELAY.to_owned())
}

/// Closes the address. Whoever held it loses the tab at once — the socket is
/// dropped and the relay has nothing to hand work to.
pub(crate) fn disconnect(state: &Shared) {
    let mut held = state.borrow_mut();
    held.hang_up();
    held.link = Link::Off;
    drop(held);
    published(None);
    // Switched off on purpose: there is nothing to walk back into.
    forget();
}

/// Writes the address into the page (`body[data-mcp]`), or takes it away.
fn published(url: Option<&str>) {
    let Some(body) = web_sys::window()
        .and_then(|w| w.document())
        .and_then(|d| d.body())
    else {
        return;
    };
    match url {
        Some(url) => {
            let _ = body.set_attribute("data-mcp", url);
        }
        None => {
            let _ = body.remove_attribute("data-mcp");
        }
    }
}

/// Opens a room on the relay and holds it, doing the work that arrives.
///
/// Returns immediately: everything after the first request happens in the
/// browser's own event loop, and `state` is what the window reads to know how
/// it went.
pub(crate) fn connect(document: SharedDocument, ctx: eframe::egui::Context, state: Shared) {
    {
        // Whatever was open before is hung up first: two sockets would mean two
        // live addresses, and only one of them on screen.
        let mut held = state.borrow_mut();
        held.hang_up();
        held.link = Link::Opening;
    }
    let base = relay_base();
    wasm_bindgen_futures::spawn_local(async move {
        match open_room(&base).await {
            Ok(room) => {
                remember(&base, &room);
                hold(&document, &ctx, &state, &base, &room);
            }
            Err(why) => {
                state.borrow_mut().link = Link::Failed { why };
                ctx.request_repaint();
            }
        }
    });
}

/// What the relay answers when a tab asks for somewhere to be reached.
#[derive(Debug, serde::Deserialize)]
struct Room {
    mcp_path: String,
    tab_path: String,
}

/// The room this tab holds, as it is kept between visits. Same origin only,
/// and no more secret than what the panel already shows on screen.
#[derive(Debug, serde::Serialize, serde::Deserialize)]
struct Kept {
    relay: String,
    mcp_path: String,
    tab_path: String,
}

fn storage() -> Option<web_sys::Storage> {
    web_sys::window().and_then(|w| w.local_storage().ok().flatten())
}

fn remember(base: &str, room: &Room) {
    let kept = Kept {
        relay: base.to_owned(),
        mcp_path: room.mcp_path.clone(),
        tab_path: room.tab_path.clone(),
    };
    if let (Some(storage), Ok(text)) = (storage(), serde_json::to_string(&kept)) {
        let _ = storage.set_item(REMEMBERED, &text);
    }
}

fn forget() {
    if let Some(storage) = storage() {
        let _ = storage.remove_item(REMEMBERED);
    }
}

/// The room from the last visit, if it was on this same relay.
fn recall(base: &str) -> Option<Room> {
    let text = storage().and_then(|s| s.get_item(REMEMBERED).ok().flatten())?;
    let kept: Kept = serde_json::from_str(&text).ok()?;
    (kept.relay == base).then_some(Room {
        mcp_path: kept.mcp_path,
        tab_path: kept.tab_path,
    })
}

/// Walks back into the room this tab held before the page was reloaded.
///
/// The relay keeps a room for a while after its tab goes, precisely so a
/// refresh is not a new address: whoever pasted it into their AI client keeps
/// a working setup. If the room is gone, so is the note of it, and the window
/// is simply off — never pointing at an address nobody answers.
pub(crate) fn resume(document: &SharedDocument, ctx: &eframe::egui::Context, state: &Shared) {
    let base = relay_base();
    let Some(room) = recall(&base) else {
        return;
    };
    {
        let mut held = state.borrow_mut();
        if !matches!(held.link, Link::Off) {
            return;
        }
        held.link = Link::Opening;
    }
    hold(document, ctx, state, &base, &room);
}

async fn open_room(base: &str) -> Result<Room, String> {
    let options = web_sys::RequestInit::new();
    options.set_method("POST");
    options.set_mode(web_sys::RequestMode::Cors);
    let request = web_sys::Request::new_with_str_and_init(&format!("{base}/rooms"), &options)
        .map_err(|e| told(&e))?;
    let window = web_sys::window().ok_or("no window")?;
    // A relay that is not there refuses at the network, before any status: say
    // that plainly instead of handing somebody a browser's error text.
    let answer = wasm_bindgen_futures::JsFuture::from(window.fetch_with_request(&request))
        .await
        .map_err(|_| {
            crate::i18n::tr(
                "o servidor que leva a sua IA até esta aba não respondeu — no aplicativo do computador o MCP não precisa dele",
            )
            .to_owned()
        })?;
    let response = answer
        .dyn_into::<web_sys::Response>()
        .map_err(|_| "the relay answered something odd".to_owned())?;
    if !response.ok() {
        return Err(format!("the relay said {}", response.status()));
    }
    let text = wasm_bindgen_futures::JsFuture::from(response.text().map_err(|e| told(&e))?)
        .await
        .map_err(|e| told(&e))?
        .as_string()
        .unwrap_or_default();
    serde_json::from_str(&text).map_err(|e| format!("the relay answered something odd: {e}"))
}

/// Opens the socket and wires what arrives on it to the tools.
fn hold(
    document: &SharedDocument,
    ctx: &eframe::egui::Context,
    state: &Shared,
    base: &str,
    room: &Room,
) {
    let ws_url = format!(
        "{}{}",
        base.replacen("https://", "wss://", 1)
            .replacen("http://", "ws://", 1),
        room.tab_path
    );
    let socket = match web_sys::WebSocket::new(&ws_url) {
        Ok(socket) => socket,
        Err(err) => {
            state.borrow_mut().link = Link::Failed { why: told(&err) };
            ctx.request_repaint();
            return;
        }
    };

    // On open: say what this window can do.
    let on_open = {
        let socket = socket.clone();
        let state = state.clone();
        let ctx = ctx.clone();
        let url = format!("{base}{}", room.mcp_path);
        Closure::<dyn FnMut()>::new(move || {
            let tools: Vec<serde_json::Value> = newera_mcp::tools()
                .iter()
                .filter(|tool| !TOO_SLOW_HERE.contains(&tool.name.as_ref()))
                .map(|tool| {
                    serde_json::json!({
                        "name": tool.name,
                        "description": tool.description,
                        "inputSchema": tool.input_schema,
                    })
                })
                .collect();
            let hello = serde_json::json!({"type": "hello", "tools": tools});
            let _ = socket.send_with_str(&hello.to_string());
            {
                let mut held = state.borrow_mut();
                held.link = Link::On { url: url.clone() };
                held.socket = Some(socket.clone());
            }
            // The page keeps the address where the panel shows it: readable by
            // whoever is already looking at this tab, and by nobody else.
            published(Some(&url));
            ctx.request_repaint();
        })
    };
    socket.set_onopen(Some(on_open.as_ref().unchecked_ref()));
    on_open.forget();

    // On message: a tool to run, or news that an AI turned up.
    let on_message = {
        let socket = socket.clone();
        let ctx = ctx.clone();
        let document = document.clone();
        Closure::<dyn FnMut(web_sys::MessageEvent)>::new(move |event: web_sys::MessageEvent| {
            let Some(text) = event.data().as_string() else {
                return;
            };
            let Ok(message) = serde_json::from_str::<serde_json::Value>(&text) else {
                return;
            };
            match message.get("type").and_then(serde_json::Value::as_str) {
                Some("client") => {
                    let name = message
                        .get("name")
                        .and_then(serde_json::Value::as_str)
                        .unwrap_or("MCP");
                    let version = message.get("version").and_then(serde_json::Value::as_str);
                    document
                        .write()
                        .agents_mut()
                        .hello(None, name, version, crate::ai::now_ms());
                    ctx.request_repaint();
                }
                Some("call") => {
                    let id = message
                        .get("id")
                        .cloned()
                        .unwrap_or(serde_json::Value::Null);
                    let name = message
                        .get("name")
                        .and_then(serde_json::Value::as_str)
                        .unwrap_or_default()
                        .to_owned();
                    let args = message
                        .get("args")
                        .cloned()
                        .unwrap_or(serde_json::Value::Null);
                    let answer = run(&document, &name, args);
                    document
                        .write()
                        .agents_mut()
                        .called(None, &name, crate::ai::now_ms());
                    let reply = match answer {
                        Ok(result) => serde_json::json!({
                            "type": "result", "id": id, "ok": true, "result": result
                        }),
                        Err(why) => serde_json::json!({
                            "type": "result", "id": id, "ok": false, "error": why
                        }),
                    };
                    let _ = socket.send_with_str(&reply.to_string());
                    ctx.request_repaint();
                }
                _ => {}
            }
        })
    };
    socket.set_onmessage(Some(on_message.as_ref().unchecked_ref()));
    on_message.forget();

    // On close or error: say so, instead of leaving an address that answers
    // nobody.
    let on_close = {
        let state = state.clone();
        let ctx = ctx.clone();
        let socket = socket.clone();
        Closure::<dyn FnMut()>::new(move || {
            let mut held = state.borrow_mut();
            // A socket that never opened means the room is gone: forget it, or
            // every visit would try the same dead address.
            if matches!(held.link, Link::Opening) {
                forget();
            }
            // Only the socket that is current speaks for the link: one closed
            // on the way to a new address has nothing to say about it.
            if held.socket.as_ref().is_some_and(|open| *open == socket) {
                held.socket = None;
                // Switched off on purpose is not a fault; only a link that was
                // up and fell has anything to report.
                if matches!(held.link, Link::On { .. } | Link::Opening) {
                    held.link = Link::Failed {
                        why: crate::i18n::tr("a ligação com o relay caiu").to_owned(),
                    };
                    drop(held);
                    published(None);
                }
            }
            ctx.request_repaint();
        })
    };
    socket.set_onclose(Some(on_close.as_ref().unchecked_ref()));
    on_close.forget();
}

/// Runs one tool in this tab's project.
fn run(
    document: &SharedDocument,
    name: &str,
    args: serde_json::Value,
) -> Result<serde_json::Value, String> {
    if TOO_SLOW_HERE.contains(&name) {
        return Err(crate::i18n::fill(
            "{} só no aplicativo do computador: aqui ele renderiza na CPU e travaria a aba",
            &[&name],
        ));
    }
    let result = newera_mcp::call(document.clone(), name, args)?;
    serde_json::to_value(result).map_err(|e| e.to_string())
}

/// Whatever the browser said went wrong, as a line a person can read.
fn told(error: &JsValue) -> String {
    error
        .as_string()
        .or_else(|| {
            error
                .dyn_ref::<js_sys::Error>()
                .map(|e| e.message().as_string().unwrap_or_default())
        })
        .unwrap_or_else(|| "the browser refused the connection".to_owned())
}
