//! The fixed MCP address, `/mcp`: the same for everyone, and each AI client
//! acts for the account its token belongs to.
//!
//! A call goes to that account's tab — the editor at 3dneweraai.com/app,
//! signed in — through the relay. The handshake, the tool list and the
//! pages are answered here, so a client can connect before the tab is open.

use std::time::Duration;

use axum::Json;
use axum::extract::State;
use axum::http::{HeaderMap, HeaderValue, StatusCode, header};
use axum::response::{IntoResponse, Response};
use serde_json::{Value, json};

use crate::{AppState, oauth};

/// Protocol versions this endpoint speaks, newest first.
const VERSIONS: [&str; 3] = ["2025-11-25", "2025-06-18", "2025-03-26"];

const INSTRUCTIONS: &str = "\
Home design editor (3D New Era AI), acting for the signed-in account. Units: cm. \
Plan axes: x right, y down. Calls reach the person's editor at 3dneweraai.com/app, \
where every change appears as it is made and can be undone with Ctrl+Z. \
Reads never change the plan; what changes it is a tool of its own.";

/// Tools that only make sense on the person's own machine, never here
/// (the directories' rules, and a server's filesystem): see D16.
const LOCAL_ONLY: [&str; 3] = ["feedback", "plugins", "run_plugin"];

/// The 401 that sends a client to the metadata, and from there to sign-in.
fn unauthorized(app: &AppState) -> Response {
    let mut response = (
        StatusCode::UNAUTHORIZED,
        Json(
            json!({"error": "invalid_token", "error_description": "sign in to use 3D New Era AI"}),
        ),
    )
        .into_response();
    let challenge = format!(
        r#"Bearer resource_metadata="{}/.well-known/oauth-protected-resource/mcp", scope="{}""#,
        app.config.public_url,
        oauth::SCOPE
    );
    response.headers_mut().insert(
        header::WWW_AUTHENTICATE,
        HeaderValue::from_str(&challenge).expect("ASCII"),
    );
    response
}

/// `POST /mcp`: one JSON-RPC message, or a batch.
pub async fn post(State(app): State<AppState>, headers: HeaderMap, body: String) -> Response {
    let Some(account) = oauth::bearer(&app, &headers).await else {
        return unauthorized(&app);
    };
    if !app.limits.allow(
        &format!("mcp:{}", account.id),
        2400,
        Duration::from_secs(3600),
    ) {
        return (StatusCode::TOO_MANY_REQUESTS, "too many calls this hour").into_response();
    }
    let Ok(parsed) = serde_json::from_str::<Value>(&body) else {
        return Json(
            json!({"jsonrpc": "2.0", "id": null, "error": {"code": -32700, "message": "not JSON"}}),
        )
        .into_response();
    };
    match parsed {
        Value::Array(batch) => {
            let mut answers = Vec::new();
            for message in &batch {
                if let Some(answer) = answer(&app, &account, message).await {
                    answers.push(answer);
                }
            }
            if answers.is_empty() {
                StatusCode::ACCEPTED.into_response()
            } else {
                Json(Value::Array(answers)).into_response()
            }
        }
        message => match answer(&app, &account, &message).await {
            Some(answer) => Json(answer).into_response(),
            None => StatusCode::ACCEPTED.into_response(),
        },
    }
}

/// No server-to-client stream here; clients carry on over POST.
pub async fn get() -> StatusCode {
    StatusCode::METHOD_NOT_ALLOWED
}

fn result(id: &Value, result: Value) -> Value {
    let mut answer = json!({"jsonrpc": "2.0", "id": id});
    answer["result"] = result;
    answer
}

fn failure(id: &Value, code: i32, message: &str) -> Value {
    json!({"jsonrpc": "2.0", "id": id, "error": {"code": code, "message": message}})
}

/// The tools a client sees: the editor's, minus what is only for a desktop.
fn hosted_tools() -> Vec<Value> {
    newera_mcp::tools()
        .into_iter()
        .filter(|t| !LOCAL_ONLY.contains(&t.name.as_ref()))
        .filter_map(|t| serde_json::to_value(t).ok())
        .collect()
}

async fn answer(
    app: &AppState,
    account: &crate::accounts::Account,
    message: &Value,
) -> Option<Value> {
    let id = message.get("id").cloned().unwrap_or(Value::Null);
    let notification = message.get("id").is_none();
    let method = message.get("method").and_then(Value::as_str).unwrap_or("");
    let params = message.get("params").cloned().unwrap_or(Value::Null);
    match method {
        "initialize" => {
            let asked = params
                .get("protocolVersion")
                .and_then(Value::as_str)
                .unwrap_or("");
            let version = VERSIONS
                .iter()
                .find(|v| **v == asked)
                .unwrap_or(&VERSIONS[1]);
            Some(result(
                &id,
                json!({
                    "protocolVersion": version,
                    "capabilities": {"tools": {}, "resources": {}},
                    "serverInfo": {
                        "name": "3d-new-era-ai",
                        "title": "3D New Era AI",
                        "version": env!("CARGO_PKG_VERSION"),
                    },
                    "instructions": INSTRUCTIONS,
                }),
            ))
        }
        _ if notification => None,
        "ping" => Some(result(&id, json!({}))),
        "tools/list" => Some(result(&id, json!({ "tools": hosted_tools() }))),
        "resources/list" => Some(result(&id, newera_mcp::app::resource_list())),
        "resources/read" => {
            let uri = params
                .get("uri")
                .and_then(Value::as_str)
                .unwrap_or_default();
            Some(match newera_mcp::app::read(uri) {
                Some(found) => result(&id, found),
                None => failure(&id, -32002, &format!("no resource {uri}")),
            })
        }
        "tools/call" => {
            let name = params
                .get("name")
                .and_then(Value::as_str)
                .unwrap_or_default();
            if LOCAL_ONLY.contains(&name) {
                return Some(result(
                    &id,
                    json!({"content": [{"type": "text", "text": format!("{name} runs only in the desktop app")}], "isError": true}),
                ));
            }
            match app.rooms.owned_by(&account.id) {
                Some(room) => newera_relay::answer_in(&app.rooms, &room, message).await,
                None => Some(result(
                    &id,
                    json!({
                        "content": [{"type": "text", "text": format!(
                            "No 3D New Era AI editor is open for {}. Ask the person to open https://3dneweraai.com/app, sign in from the AI panel with this address, and try again.",
                            account.email
                        )}],
                        "isError": true,
                    }),
                )),
            }
        }
        other => Some(failure(
            &id,
            -32601,
            &format!("this server does not do {other}"),
        )),
    }
}
