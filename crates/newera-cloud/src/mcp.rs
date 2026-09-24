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
Plan axes: x right, y down. When the person has the editor open at \
3dneweraai.com/app and signed in, calls reach it and every change appears there; \
otherwise they work on the account's active project in the cloud, kept after every \
change (projects lists them). Reads never change the plan; what changes it is a tool \
of its own. show_plan shows the plan to the person.";

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

/// What the tools that deal in files mean here, where the files are the
/// account's projects and exports come back as links.
const CLOUD_MEANING: [(&str, &str); 5] = [
    (
        "open_home",
        "Open one of the account's projects by name (projects lists them); it becomes the active one.",
    ),
    (
        "save_home",
        "Keep the active project now — it is also kept after every change. path renames it.",
    ),
    (
        "new_home",
        "Start a new, empty project in the account and make it the active one; name is optional.",
    ),
    (
        "export_plan",
        "Export the active project by the extension of path: plan .pdf (A3; scale=50/100 or fit), .svg or .png; 3D model .glb or .obj. Reply: a link to the file, good for a day.",
    ),
    (
        "export_cut_list",
        "Write the cut list of joinery builds as .csv, or .dxf/.svg sheets (path gives the name and the format). Reply: a link to the file, good for a day.",
    ),
];

/// The tools a client sees: the editor's, minus what is only for a desktop,
/// with the file tools saying what they do here, and the projects list.
fn hosted_tools() -> Vec<Value> {
    let mut tools: Vec<Value> = newera_mcp::tools()
        .into_iter()
        .filter(|t| !LOCAL_ONLY.contains(&t.name.as_ref()))
        .filter_map(|t| serde_json::to_value(t).ok())
        .collect();
    for tool in &mut tools {
        if let Some((_, meaning)) = CLOUD_MEANING.iter().find(|(n, _)| tool["name"] == *n) {
            tool["description"] = json!(meaning);
        }
    }
    tools.push(json!({
        "name": "projects",
        "title": "List your projects",
        "description": "The projects kept in the account: rows [name, id, kb, updated, active], the space used and the plan's limits. open_home switches the active one; new_home starts one.",
        "inputSchema": {"type": "object", "properties": {}, "additionalProperties": false},
        "annotations": {"title": "List your projects", "readOnlyHint": true, "openWorldHint": false},
    }));
    tools
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
            // The person's own editor, when it is open and signed in; the
            // cloud project otherwise. `projects` is always the cloud's.
            if name != "projects"
                && let Some(room) = app.rooms.owned_by(&account.id)
            {
                return newera_relay::answer_in(&app.rooms, &room, message).await;
            }
            let args = params.get("arguments").cloned().unwrap_or(Value::Null);
            let plan = match crate::accounts::plan_of(&app.db, &account.id).await {
                Ok(plan) => plan,
                Err(err) => {
                    tracing::error!("plan: {err}");
                    return Some(failure(
                        &id,
                        -32603,
                        "the service could not read the account's plan",
                    ));
                }
            };
            match app
                .engine
                .call(
                    &app.db,
                    &app.config.public_url,
                    &account.id,
                    &plan,
                    name,
                    args,
                )
                .await
            {
                Ok(answer) => Some(result(&id, answer)),
                Err(err) => {
                    tracing::error!("engine {name}: {err:#}");
                    Some(result(
                        &id,
                        json!({"content": [{"type": "text", "text": format!("{name} failed on the server: {err}")}], "isError": true}),
                    ))
                }
            }
        }
        other => Some(failure(
            &id,
            -32601,
            &format!("this server does not do {other}"),
        )),
    }
}
