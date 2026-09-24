//! The browser's side of the MCP: the tab asks a relay for an address, holds a
//! socket to it, and does the work an AI asks for — in the project on screen.
//!
//! A tab cannot listen on a port, so it cannot be an MCP server the way the
//! desktop is. What it can do is hold one connection outwards. The relay (see
//! `newera-relay`) gives out an address, speaks the protocol to the AI, and
//! hands each tool call down this socket; the answers go back the same way.
//! Nothing of the project is stored there: it passes through.
//!
//! Images run in an isolated render worker. The file-based MCP `video` tool
//! remains desktop-only; the browser's video window downloads an AVI instead.

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
const TOO_SLOW_HERE: [&str; 2] = ["video", "edit_video"];

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
    /// Who is signed in at the relay's service, if anyone: then this tab's
    /// room is theirs, and their AI reaches it at the service's one address.
    pub(crate) account: Account,
    /// The room held now, to claim it for the account.
    kept: Option<Kept>,
    /// When the account was last asked about (ms since the epoch), so the
    /// panel can keep asking while it waits without flooding the service.
    asked_at: f64,
    /// Reconnections tried since the link was last up, for the backoff.
    retries: u32,
}

/// The account the person in this tab is signed into, as the service said.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) enum Account {
    /// Not asked yet, or a relay with no accounts (one run locally).
    #[default]
    Unknown,
    /// Nobody signed in.
    Out,
    /// Signed in; `mcp_url` is the one address for every AI client.
    In { email: String, mcp_url: String },
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
    held.retries = 0;
    drop(held);
    published(None);
    // The room is kept on purpose: switching on again gives back the same
    // address, and the one already pasted into an AI client goes on working.
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
        match open_room(&base, recall(&base)).await {
            Ok(kept) => {
                remember(&kept);
                hold(&document, &ctx, &state, &base, &kept);
            }
            // While coming back from a link that fell, a failure is just the
            // service not being up yet: try again, later.
            Err(_) if state.borrow().retries > 0 => {
                retry_later(&document, &ctx, &state);
            }
            Err(why) => {
                state.borrow_mut().link = Link::Failed { why };
                ctx.request_repaint();
            }
        }
    });
}

/// Tries the link again after a while — 2, 4, 8… up to 32 seconds — keeping
/// the same room, unless somebody switched it off meanwhile.
fn retry_later(document: &SharedDocument, ctx: &eframe::egui::Context, state: &Shared) {
    let wait = {
        let mut held = state.borrow_mut();
        held.link = Link::Opening;
        held.retries += 1;
        1000 * 2_i32.pow(held.retries.min(5))
    };
    ctx.request_repaint();
    let (document, ctx, state) = (document.clone(), ctx.clone(), state.clone());
    let again = Closure::once_into_js(move || {
        if matches!(state.borrow().link, Link::Opening) {
            connect(document, ctx, state);
        }
    });
    if let Some(window) = web_sys::window() {
        let _ = window
            .set_timeout_with_callback_and_timeout_and_arguments_0(again.unchecked_ref(), wait);
    }
}

/// What the relay answers when a tab asks for somewhere to be reached.
#[derive(Debug, serde::Deserialize)]
struct Room {
    mcp_path: String,
    tab_path: String,
}

/// The room this tab holds, kept between visits — and kept when the switch is
/// thrown, because an address that changes is an address somebody has to paste
/// into their AI client again.
///
/// Same origin only, and no more secret than what the panel shows on screen.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
struct Kept {
    relay: String,
    room: String,
    tab_key: String,
    client_token: String,
}

impl Kept {
    fn room(&self) -> Room {
        Room {
            mcp_path: format!("/r/{}/{}/mcp", self.room, self.client_token),
            tab_path: format!("/r/{}/tab?key={}", self.room, self.tab_key),
        }
    }
}

fn storage() -> Option<web_sys::Storage> {
    web_sys::window().and_then(|w| w.local_storage().ok().flatten())
}

fn remember(kept: &Kept) {
    if let (Some(storage), Ok(text)) = (storage(), serde_json::to_string(kept)) {
        let _ = storage.set_item(REMEMBERED, &text);
    }
}

fn forget() {
    if let Some(storage) = storage() {
        let _ = storage.remove_item(REMEMBERED);
    }
}

/// The room from the last visit, if it was on this same relay.
fn recall(base: &str) -> Option<Kept> {
    let text = storage().and_then(|s| s.get_item(REMEMBERED).ok().flatten())?;
    let kept: Kept = serde_json::from_str(&text).ok()?;
    (kept.relay == base).then_some(kept)
}

/// Makes this tab reachable as it opens, without anybody being asked to.
///
/// The point of this editor is that an AI drives it; a switch somebody has to
/// find first is a step between them and that. So a window wide enough to be
/// somebody's desk comes up already reachable — at the same address as last
/// time, since the room is kept — and the panel shows the address and the
/// switch for whoever wants it off.
///
/// A phone is left alone: the address is of no use without a place to paste
/// it, and nothing should be opened on somebody's behalf for nothing.
pub(crate) fn start(document: &SharedDocument, ctx: &eframe::egui::Context, state: &Shared) {
    watch_focus(state, ctx);
    if ctx.content_rect().width() < crate::app::NARROW {
        return;
    }
    if !matches!(state.borrow().link, Link::Off) {
        return;
    }
    // The room this tab held last time is asked for again rather than walked
    // straight into: the relay may have restarted since (every deploy does),
    // and asking is what puts the same room back there, at the same address.
    connect(document.clone(), ctx.clone(), state.clone());
}

/// Asks the service who is signed in and, if someone is, hands them this
/// tab's room. Runs when the socket opens and whenever the tab comes back into
/// focus — which is when somebody who just signed in, in another tab, returns.
pub(crate) fn check_account(state: &Shared, ctx: &eframe::egui::Context) {
    let Some(kept) = state.borrow().kept.clone() else {
        return;
    };
    state.borrow_mut().asked_at = js_sys::Date::now();
    if !matches!(state.borrow().link, Link::On { .. }) {
        return;
    }
    let (state, ctx) = (state.clone(), ctx.clone());
    wasm_bindgen_futures::spawn_local(async move {
        let base = kept.relay.clone();
        let account = match with_cookie(&format!("{base}/account/me"), None).await {
            Ok((200, text)) => serde_json::from_str::<serde_json::Value>(&text)
                .ok()
                .and_then(|me| {
                    Some(Account::In {
                        email: me["email"].as_str()?.to_owned(),
                        mcp_url: me["mcp_url"].as_str()?.to_owned(),
                    })
                })
                .unwrap_or_default(),
            Ok((401, _)) => Account::Out,
            // A relay with no accounts (404), or no answer: say nothing.
            _ => Account::Unknown,
        };
        if matches!(account, Account::In { .. }) {
            let claim = serde_json::json!({"room": kept.room, "tab_key": kept.tab_key}).to_string();
            if !matches!(
                with_cookie(&format!("{base}/account/claim"), Some(&claim)).await,
                Ok((200, _))
            ) {
                web_sys::console::warn_1(&"could not hand this tab to the account".into());
            }
        }
        state.borrow_mut().account = account;
        ctx.request_repaint();
    });
}

/// Asks again while the panel waits for somebody to sign in, at most every
/// few seconds: the sign-in happens in another tab, and coming back to this
/// one does not always tell the page.
pub(crate) fn recheck_account(state: &Shared, ctx: &eframe::egui::Context) {
    let due = js_sys::Date::now() - state.borrow().asked_at > 3000.0;
    if due {
        check_account(state, ctx);
    }
    ctx.request_repaint_after(std::time::Duration::from_secs(3));
}

/// A request to the service with the person's cookie: GET, or POST with a
/// JSON body. Answers the status and the text.
async fn with_cookie(url: &str, body: Option<&str>) -> Result<(u16, String), String> {
    let options = web_sys::RequestInit::new();
    options.set_mode(web_sys::RequestMode::Cors);
    options.set_credentials(web_sys::RequestCredentials::Include);
    if let Some(body) = body {
        options.set_method("POST");
        options.set_body(&wasm_bindgen::JsValue::from_str(body));
        let headers = web_sys::Headers::new().map_err(|e| told(&e))?;
        headers
            .set("content-type", "application/json")
            .map_err(|e| told(&e))?;
        options.set_headers(&headers);
    }
    let request = web_sys::Request::new_with_str_and_init(url, &options).map_err(|e| told(&e))?;
    let window = web_sys::window().ok_or("no window")?;
    let answer = wasm_bindgen_futures::JsFuture::from(window.fetch_with_request(&request))
        .await
        .map_err(|e| told(&e))?;
    let response = answer
        .dyn_into::<web_sys::Response>()
        .map_err(|_| "an odd answer".to_owned())?;
    let status = response.status();
    let text = wasm_bindgen_futures::JsFuture::from(response.text().map_err(|e| told(&e))?)
        .await
        .map_err(|e| told(&e))?
        .as_string()
        .unwrap_or_default();
    Ok((status, text))
}

/// Opens the service's sign-in in a new tab; this tab claims its room when
/// the person comes back to it.
pub(crate) fn sign_in(state: &Shared) {
    let base = state
        .borrow()
        .kept
        .as_ref()
        .map_or_else(relay_base, |k| k.relay.clone());
    let Some(window) = web_sys::window() else {
        return;
    };
    // Back to a page that says "done, return to the editor" rather than to
    // the editor itself: a second editor would take this tab's room.
    let target = format!("{base}/login?return_to=%2Faccount%2Fdone");
    let _ = window.open_with_url_and_target(&target, "_blank");
}

/// Checks the account again each time the tab gets focus back. Set up once.
pub(crate) fn watch_focus(state: &Shared, ctx: &eframe::egui::Context) {
    let Some(window) = web_sys::window() else {
        return;
    };
    let (state, ctx) = (state.clone(), ctx.clone());
    let on_focus = Closure::<dyn FnMut()>::new(move || check_account(&state, &ctx));
    let _ = window.add_event_listener_with_callback("focus", on_focus.as_ref().unchecked_ref());
    if let Some(document) = window.document() {
        let _ = document.add_event_listener_with_callback(
            "visibilitychange",
            on_focus.as_ref().unchecked_ref(),
        );
    }
    // Lives as long as the page.
    on_focus.forget();
}

/// What the relay answers when a room is opened or claimed.
#[derive(serde::Deserialize)]
struct Opened {
    room: String,
    tab_key: String,
    mcp_path: String,
}

async fn open_room(base: &str, keeping: Option<Kept>) -> Result<Kept, String> {
    let options = web_sys::RequestInit::new();
    options.set_method("POST");
    options.set_mode(web_sys::RequestMode::Cors);
    // Asking for the room this tab already had is what keeps its address the
    // same — across a reload, a switch off and on, and a relay that restarted.
    if let Some(keeping) = &keeping
        && let Ok(body) = serde_json::to_string(&serde_json::json!({
            "room": keeping.room,
            "tab_key": keeping.tab_key,
            "client_token": keeping.client_token,
        }))
    {
        options.set_body(&wasm_bindgen::JsValue::from_str(&body));
        let headers = web_sys::Headers::new().map_err(|e| told(&e))?;
        headers
            .set("content-type", "application/json")
            .map_err(|e| told(&e))?;
        options.set_headers(&headers);
    }
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
    if response.status() == 403 {
        // The room belongs to somebody else now: start over rather than keep
        // asking for a door that is not ours.
        forget();
        return Err(
            crate::i18n::tr("o endereço guardado não vale mais — ligue de novo").to_owned(),
        );
    }
    if !response.ok() {
        return Err(format!("the relay said {}", response.status()));
    }
    let text = wasm_bindgen_futures::JsFuture::from(response.text().map_err(|e| told(&e))?)
        .await
        .map_err(|e| told(&e))?
        .as_string()
        .unwrap_or_default();
    let opened: Opened = serde_json::from_str(&text)
        .map_err(|e| format!("the relay answered something odd: {e}"))?;
    // The token is in the path the relay hands back; keeping it apart is what
    // lets the tab ask for this very room again.
    let client_token = opened
        .mcp_path
        .split('/')
        .nth(3)
        .unwrap_or_default()
        .to_owned();
    Ok(Kept {
        relay: base.to_owned(),
        room: opened.room,
        tab_key: opened.tab_key,
        client_token,
    })
}

/// Opens the socket and wires what arrives on it to the tools.
fn hold(
    document: &SharedDocument,
    ctx: &eframe::egui::Context,
    state: &Shared,
    base: &str,
    kept: &Kept,
) {
    state.borrow_mut().kept = Some(kept.clone());
    let room = &kept.room();
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

    // The emergency responder is owned by JS, not by a wasm_bindgen Closure.
    if let Some(window) = web_sys::window()
        && let Ok(callback) =
            js_sys::Reflect::get(&window, &JsValue::from_str("neweraAttachSocket"))
        && let Some(callback) = callback.dyn_ref::<js_sys::Function>()
    {
        let _ = callback.call1(&JsValue::NULL, socket.as_ref());
    }

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
                        "description": if tool.name == "save_home" {
                            "Download a .newera project backup in the browser. path supplies a filename only. Reports download_started (not disk confirmation) and autosave recovery status; no server file is written.".to_owned()
                        } else if tool.name == "get_home" {
                            format!("{} Browser replies also include recovery state, current_revision, saved_revision or restored_from_revision, timestamp and any storage failure.", tool.description.as_deref().unwrap_or(""))
                        } else { tool.description.as_deref().unwrap_or("").to_owned() },
                        "inputSchema": tool.input_schema,
                        "title": tool.title,
                        "annotations": tool.annotations,
                        "_meta": tool.meta,
                    })
                })
                .collect();
            let hello = serde_json::json!({
                "type": "hello",
                "tools": tools,
                "resources": newera_mcp::app::resources(),
            });
            let _ = socket.send_with_str(&hello.to_string());
            {
                let mut held = state.borrow_mut();
                held.link = Link::On { url: url.clone() };
                held.socket = Some(socket.clone());
                held.retries = 0;
            }
            // The page keeps the address where the panel shows it: readable by
            // whoever is already looking at this tab, and by nobody else.
            published(Some(&url));
            ctx.request_repaint();
            check_account(&state, &ctx);
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
                    let (document, socket, ctx) = (document.clone(), socket.clone(), ctx.clone());
                    wasm_bindgen_futures::spawn_local(async move {
                        let diagnostic =
                            operation("begin", &JsValue::NULL, &name, document.read().revision());
                        let answer = if ["render_plan", "render_3d", "render_photo"]
                            .contains(&name.as_str())
                        {
                            crate::render_web::mcp(&document, &name, args, &ctx).await
                        } else {
                            run(&document, &name, args)
                        };
                        operation("end", &diagnostic, &name, document.read().revision());
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
                    });
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
        let document = document.clone();
        Closure::<dyn FnMut()>::new(move || {
            let mut held = state.borrow_mut();
            let current = held.socket.as_ref().is_some_and(|open| *open == socket);
            // A socket that never opened, while this tab was opening: the room
            // was refused. Forget it, or every visit would try the same dead
            // address, and say so instead of waiting forever.
            if held.socket.is_none() && matches!(held.link, Link::Opening) {
                forget();
                held.link = Link::Failed {
                    why: crate::i18n::tr("o relay recusou esta aba — ligue de novo").to_owned(),
                };
                drop(held);
                published(None);
                ctx.request_repaint();
                return;
            }
            // Only the socket that is current speaks for the link: one closed
            // on the way to a new address has nothing to say about it.
            if current {
                held.socket = None;
                // A link that was up and fell — a new version of the service
                // going up, a network hiccup — comes back by itself, at the
                // same address, trying less often the longer it fails.
                if matches!(held.link, Link::On { .. }) {
                    drop(held);
                    published(None);
                    retry_later(&document, &ctx, &state);
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
        return Err("A ferramenta video salva arquivos no aplicativo. No navegador, use Criar vídeo para baixar o AVI.".into());
    }
    if name == "save_home" {
        let doc = document.read();
        let requested = args["path"].as_str().unwrap_or("projeto.newera");
        let basename = requested
            .rsplit(['/', '\\'])
            .next()
            .unwrap_or("projeto.newera");
        let filename = if basename.ends_with(".newera") {
            basename.to_owned()
        } else {
            format!("{basename}.newera")
        };
        let bytes = newera_core::to_project_bytes(&doc);
        let byte_count = bytes.len();
        let _ = crate::recovery_web::store(&doc, &bytes);
        crate::files::save_bytes("New Era", "newera", &filename, || Ok(bytes))?;
        let payload = serde_json::json!({"download_started":true,"name":filename,
            "revision":doc.revision(),"bytes":byte_count,"recovery":crate::recovery_web::status()});
        return Ok(serde_json::json!({"content":[{"type":"text","text":payload.to_string()}]}));
    }
    let result = newera_mcp::call(document.clone(), name, args)?;
    let mut result = serde_json::to_value(result).map_err(|e| e.to_string())?;
    if name == "get_home"
        && let Some(content) = result["content"].as_array_mut()
    {
        for item in content {
            if item["type"] != "text" {
                continue;
            }
            if let Some(text) = item["text"].as_str()
                && let Ok(mut home) = serde_json::from_str::<serde_json::Value>(text)
            {
                let mut recovery = crate::recovery_web::status();
                recovery["current_revision"] = serde_json::json!(document.read().revision());
                home["recovery"] = recovery;
                item["text"] = serde_json::json!(home.to_string());
            }
        }
    }

    Ok(result)
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

/// Keep diagnostic context in JS so it remains readable after a WASM trap.
fn operation(event: &str, token: &JsValue, name: &str, revision: u64) -> JsValue {
    let Some(window) = web_sys::window() else {
        return JsValue::NULL;
    };
    let Ok(callback) = js_sys::Reflect::get(&window, &JsValue::from_str("neweraOperation")) else {
        return JsValue::NULL;
    };
    let Some(callback) = callback.dyn_ref::<js_sys::Function>() else {
        return JsValue::NULL;
    };
    callback
        .call4(
            &JsValue::NULL,
            &JsValue::from_str(event),
            token,
            &JsValue::from_str(name),
            &JsValue::from_str(&revision.to_string()),
        )
        .unwrap_or(JsValue::NULL)
}
