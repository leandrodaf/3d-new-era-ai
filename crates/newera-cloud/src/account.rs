//! The account: its page (plan, sign out, close), and what the editor in a
//! browser tab asks of it — who is signed in, and "this tab is mine".

use axum::Json;
use axum::extract::{Form, State};
use axum::http::{HeaderMap, StatusCode, header};
use axum::response::{IntoResponse, Redirect, Response};
use serde::Deserialize;
use serde_json::json;

use crate::login::{self, SESSION};
use crate::pages::{Lang, escape, page};
use crate::{AppState, accounts, secret};

fn close_token(session: &str) -> String {
    secret::hash(&format!("close:{session}"))
}

/// The account page.
pub async fn show(State(app): State<AppState>, headers: HeaderMap) -> Response {
    let lang = Lang::of(&headers);
    let Some(account) = login::signed_in(&app, &headers).await else {
        return Redirect::to("/login?return_to=/account").into_response();
    };
    let plan = accounts::plan_of(&app.db, &account.id).await.ok();
    let session = login::cookie(&headers, SESSION).unwrap_or_default();
    let body = format!(
        r#"<h1>{title}</h1>
<p>{email}</p>
<p class="muted">{plan_label}: <b>{plan}</b></p>
<a class="button" href="https://3dneweraai.com/app/">{editor}</a>
<a class="button ghost" href="/logout">{out}</a>
<details><summary class="muted">{close}</summary>
<form method="post" action="/account/close">
<p class="muted">{close_text}</p>
<input type="hidden" name="csrf" value="{csrf}">
<button class="ghost" type="submit">{close_button}</button>
</form>
</details>"#,
        title = lang.pick("Sua conta", "Your account"),
        email = escape(&account.email),
        plan_label = lang.pick("Plano", "Plan"),
        plan = escape(&plan.map_or_else(|| "Free".to_owned(), |p| p.name)),
        editor = lang.pick("Abrir o editor", "Open the editor"),
        out = lang.pick("Sair", "Sign out"),
        close = lang.pick("Apagar a conta", "Close the account"),
        close_text = lang.pick(
            "Encerra o acesso na hora, desconecta as IAs ligadas a ela e apaga tudo em até 30 dias. Uma assinatura se cancela no Polar.",
            "Ends access at once, disconnects the AIs linked to it and deletes everything within 30 days. A subscription is cancelled in Polar."
        ),
        csrf = close_token(&session),
        close_button = lang.pick("Apagar minha conta", "Close my account"),
    );
    page(lang, lang.pick("Sua conta", "Your account"), &body).into_response()
}

#[derive(Debug, Deserialize)]
pub struct CloseForm {
    csrf: String,
}

/// Closes the account.
pub async fn close(
    State(app): State<AppState>,
    headers: HeaderMap,
    Form(f): Form<CloseForm>,
) -> Response {
    let lang = Lang::of(&headers);
    let Some(account) = login::signed_in(&app, &headers).await else {
        return Redirect::to("/login").into_response();
    };
    let session = login::cookie(&headers, SESSION).unwrap_or_default();
    if !secret::same(&f.csrf, &close_token(&session)) {
        return (StatusCode::BAD_REQUEST, "the form expired").into_response();
    }
    if let Err(err) = accounts::close(&app.db, &account.id).await {
        tracing::error!("closing an account: {err}");
        return StatusCode::INTERNAL_SERVER_ERROR.into_response();
    }
    let body = format!(
        "<h1>{}</h1><p class=\"muted\">{}</p>",
        lang.pick("Conta apagada", "Account closed"),
        lang.pick(
            "Tudo o que era dela some em até 30 dias.",
            "Everything that was in it goes within 30 days."
        )
    );
    let mut response = page(lang, "Account", &body).into_response();
    response.headers_mut().append(
        header::SET_COOKIE,
        header::HeaderValue::from_static(
            "newera_session=; Path=/; HttpOnly; SameSite=Lax; Max-Age=0",
        ),
    );
    response
}

/// A request from the site with the person's cookie must come from the
/// site: the browser sets `Origin` and a page cannot forge it.
fn from_site(app: &AppState, headers: &HeaderMap) -> bool {
    headers
        .get(header::ORIGIN)
        .and_then(|v| v.to_str().ok())
        .is_some_and(|origin| app.config.site_origins.iter().any(|o| o == origin))
}

/// Who is signed in, for the editor's AI panel.
pub async fn me(State(app): State<AppState>, headers: HeaderMap) -> Response {
    let Some(account) = login::signed_in(&app, &headers).await else {
        return (StatusCode::UNAUTHORIZED, Json(json!({"signed_in": false}))).into_response();
    };
    let plan = accounts::plan_of(&app.db, &account.id).await.ok();
    Json(json!({
        "signed_in": true,
        "email": account.email,
        "plan": plan,
        "mcp_url": format!("{}/mcp", app.config.public_url),
    }))
    .into_response()
}

#[derive(Debug, Deserialize)]
pub struct Claim {
    room: String,
    tab_key: String,
}

/// "This tab is mine": the editor, signed in, hands its relay room to the
/// account, so the account's AI clients reach it at the fixed address.
pub async fn claim(
    State(app): State<AppState>,
    headers: HeaderMap,
    Json(c): Json<Claim>,
) -> Response {
    if !from_site(&app, &headers) {
        return (StatusCode::FORBIDDEN, "only the editor can claim a room").into_response();
    }
    let Some(account) = login::signed_in(&app, &headers).await else {
        return StatusCode::UNAUTHORIZED.into_response();
    };
    match app.rooms.claim(&c.room, &c.tab_key, &account.id) {
        Ok(()) => Json(json!({"ok": true, "mcp_url": format!("{}/mcp", app.config.public_url)}))
            .into_response(),
        Err(why) => (StatusCode::FORBIDDEN, why).into_response(),
    }
}
