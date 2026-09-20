//! Confirmed browser snapshots and metadata available even after a WASM trap.
use base64::Engine as _;
use newera_core::Document;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use wasm_bindgen::JsValue;

const KEY: &str = "newera-autosave";
const LIMIT: usize = 3 * 1024 * 1024;

#[derive(Serialize, Deserialize)]
pub(crate) struct Snapshot {
    version: u32,
    pub name: String,
    pub revision: u64,
    saved_at: f64,
    pub data: String,
}

fn storage() -> Result<web_sys::Storage, String> {
    web_sys::window()
        .ok_or("no browser window")?
        .local_storage()
        .map_err(|_| "browser storage unavailable")?
        .ok_or_else(|| "browser storage unavailable".into())
}

pub(crate) fn status() -> Value {
    web_sys::window()
        .and_then(|w| js_sys::Reflect::get(&w, &JsValue::from_str("neweraRecovery")).ok())
        .and_then(|v| js_sys::JSON::stringify(&v).ok())
        .and_then(|v| v.as_string())
        .and_then(|v| serde_json::from_str(&v).ok())
        .unwrap_or_else(|| json!({"state":"not_saved"}))
}

fn publish(value: &Value) {
    if let Some(w) = web_sys::window()
        && let Ok(value) = js_sys::JSON::parse(&value.to_string())
    {
        let _ = js_sys::Reflect::set(&w, &JsValue::from_str("neweraRecovery"), &value);
    }
}

pub(crate) fn failed(reason: &str, revision: Option<u64>) {
    let mut value = status();
    value["state"] = json!("failed");
    value["reason"] = json!(reason);
    value["attempted_revision"] = json!(revision);
    publish(&value);
}

pub(crate) fn restored(snapshot: &Snapshot, revision: u64) {
    publish(
        &json!({"state":"restored", "restored_from_revision":if snapshot.saved_at > 0.0 { Some(snapshot.revision) } else { None },
        "restored_from_saved_at":snapshot.saved_at, "legacy":snapshot.saved_at == 0.0, "current_revision":revision}),
    );
}

pub(crate) fn load() -> Result<Option<Snapshot>, String> {
    let Some(raw) = storage()?
        .get_item(KEY)
        .map_err(|_| "cannot read browser snapshot")?
    else {
        return Ok(None);
    };
    if raw.starts_with('{') {
        let snapshot: Snapshot =
            serde_json::from_str(&raw).map_err(|_| "invalid browser snapshot")?;
        if snapshot.version != 1 {
            return Err("unsupported browser snapshot version".into());
        }
        Ok(Some(snapshot))
    } else {
        let (name, data) = raw
            .split_once('\n')
            .ok_or("invalid legacy browser snapshot")?;
        Ok(Some(Snapshot {
            version: 1,
            name: name.into(),
            revision: 0,
            saved_at: 0.0,
            data: data.into(),
        }))
    }
}

pub(crate) fn save(doc: &Document) -> Result<(), String> {
    store(doc, &newera_core::to_project_bytes(doc))
}

pub(crate) fn store(doc: &Document, bytes: &[u8]) -> Result<(), String> {
    let result: Result<(), String> = (|| {
        if bytes.len() > LIMIT {
            return Err("browser snapshot exceeds 3 MiB; download a project backup".into());
        }
        let snapshot = Snapshot {
            version: 1,
            name: format!("{}.newera", doc.home().name),
            revision: doc.revision(),
            saved_at: js_sys::Date::now(),
            data: base64::engine::general_purpose::STANDARD.encode(bytes),
        };
        let encoded = serde_json::to_string(&snapshot).map_err(|e| e.to_string())?;
        // A single atomic write keeps metadata tied to the exact saved bytes.
        // Quota failure preserves the previous snapshot and never advances saved_revision.
        storage()?
            .set_item(KEY, &encoded)
            .map_err(|_| "browser refused snapshot (quota or storage disabled)".to_owned())?;
        let mut value = status();
        value["state"] = json!("saved");
        value["saved_revision"] = json!(snapshot.revision);
        value["saved_at"] = json!(snapshot.saved_at);
        value["bytes"] = json!(bytes.len());
        if let Some(o) = value.as_object_mut() {
            o.remove("reason");
            o.remove("attempted_revision");
        }
        publish(&value);
        Ok(())
    })();
    if let Err(reason) = &result {
        failed(reason, Some(doc.revision()));
    }
    result
}
