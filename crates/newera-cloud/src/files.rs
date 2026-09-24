//! Exports, served at their secret link for a day.

use axum::body::Body;
use axum::extract::{Path, State};
use axum::http::{StatusCode, header};
use axum::response::{IntoResponse, Response};

use crate::{AppState, secret};

/// The file behind a link, while the link works.
pub async fn get(State(app): State<AppState>, Path(token): Path<String>) -> Response {
    let found: Option<(String, String, Vec<u8>)> = sqlx::query_as(
        "select name, content_type, data from files where token_hash = $1 and expires_at > now()",
    )
    .bind(secret::hash(&token))
    .fetch_optional(&app.db)
    .await
    .ok()
    .flatten();
    let Some((name, content_type, data)) = found else {
        return (StatusCode::NOT_FOUND, "this link expired").into_response();
    };
    download(&name, &content_type, data)
}

/// A file to save, under its own name.
pub fn download(name: &str, content_type: &str, data: Vec<u8>) -> Response {
    let safe: String = name
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || matches!(c, '.' | '-' | '_' | ' ') {
                c
            } else {
                '_'
            }
        })
        .collect();
    (
        [
            (header::CONTENT_TYPE, content_type.to_owned()),
            (
                header::CONTENT_DISPOSITION,
                format!("attachment; filename=\"{safe}\""),
            ),
            (header::CACHE_CONTROL, "private, no-store".to_owned()),
            (header::X_CONTENT_TYPE_OPTIONS, "nosniff".to_owned()),
        ],
        Body::from(data),
    )
        .into_response()
}
