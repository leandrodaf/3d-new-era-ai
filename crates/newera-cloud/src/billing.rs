//! Paid plans, sold through Paddle.
//!
//! Paddle is the merchant of record: it charges, handles the tax and the
//! invoice, and pays the money out in dollars to a seller in Brazil. This
//! service does three things around it:
//!
//! - the checkout page (`/billing/checkout`) opens Paddle's checkout for the
//!   signed-in account, carrying the account id in `custom_data`;
//! - the webhook (`/billing/paddle`) hears each subscription event and makes
//!   the account's plan follow it;
//! - "manage my subscription" (`/billing/manage`) opens Paddle's customer
//!   portal, where the person changes the card or cancels.
//!
//! The tools never ask "does this account pay?": they read the plan's
//! limits, and this is what changes the plan. Nothing here is shown inside an
//! AI chat: the directories forbid selling there.

use axum::body::Bytes;
use axum::extract::{Query, State};
use axum::http::{HeaderMap, StatusCode};
use axum::response::{IntoResponse, Redirect, Response};
use serde::Deserialize;
use serde_json::{Value, json};
use sha2::Sha256;

use crate::config::Paddle;
use crate::login;
use crate::pages::{Lang, page};
use crate::{AppState, accounts, secret};

/// The one paid plan; every Paddle price of this service gives it.
pub const PAID_PLAN: &str = "supporter";

/// How old a delivery may be, in seconds, before it is refused as a replay.
/// Paddle signs every attempt afresh, so a retry is never this old.
const TOLERANCE_SECS: i64 = 300;

/// HMAC-SHA256, by the book (RFC 2104), on the hash the crate already uses.
fn hmac(key: &[u8], message: &[u8]) -> [u8; 32] {
    use sha2::Digest as _;
    let mut block = [0u8; 64];
    if key.len() > 64 {
        block[..32].copy_from_slice(&Sha256::digest(key));
    } else {
        block[..key.len()].copy_from_slice(key);
    }
    let inner_pad: Vec<u8> = block.iter().map(|b| b ^ 0x36).collect();
    let outer_pad: Vec<u8> = block.iter().map(|b| b ^ 0x5c).collect();
    let inner = Sha256::new()
        .chain_update(&inner_pad)
        .chain_update(message)
        .finalize();
    Sha256::new()
        .chain_update(&outer_pad)
        .chain_update(inner)
        .finalize()
        .into()
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().fold(String::new(), |mut out, b| {
        use std::fmt::Write as _;
        let _ = write!(out, "{b:02x}");
        out
    })
}

/// Paddle's `h1`: HMAC-SHA256 of `ts:body` under the destination's secret.
fn h1(secret: &str, ts: &str, body: &[u8]) -> String {
    let mut message = format!("{ts}:").into_bytes();
    message.extend_from_slice(body);
    hex(&hmac(secret.as_bytes(), &message))
}

/// The `Paddle-Signature` a delivery of `body` at `ts` would carry: what
/// Paddle computes, here for tests and for trying the endpoint by hand.
pub fn signature(secret: &str, ts: i64, body: &[u8]) -> String {
    format!("ts={ts};h1={}", h1(secret, &ts.to_string(), body))
}

/// Whether a delivery is Paddle's: `Paddle-Signature: ts=…;h1=…` over
/// `ts:body`, recent enough not to be a replay. While a secret is being
/// rotated Paddle sends one `h1` per secret; any of them may match.
pub fn verified(secret: &str, header: &str, body: &[u8], now: i64) -> bool {
    let mut ts = None;
    let mut signatures = Vec::new();
    for part in header.split(';') {
        match part.trim().split_once('=') {
            Some(("ts", value)) => ts = Some(value.trim()),
            Some(("h1", value)) => signatures.push(value.trim()),
            _ => {}
        }
    }
    let Some(ts) = ts else { return false };
    let Ok(at) = ts.parse::<i64>() else {
        return false;
    };
    if (now - at).abs() > TOLERANCE_SECS {
        return false;
    }
    let expected = h1(secret, ts, body);
    signatures.iter().any(|s| secret::same(s, &expected))
}

fn now() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| i64::try_from(d.as_secs()).unwrap_or(0))
}

/// Paddle's webhook endpoint.
pub async fn webhook(State(app): State<AppState>, headers: HeaderMap, body: Bytes) -> StatusCode {
    let Some(paddle) = app.config.paddle.as_ref() else {
        return StatusCode::NOT_FOUND;
    };
    let header = headers
        .get("paddle-signature")
        .and_then(|v| v.to_str().ok())
        .unwrap_or_default();
    if !verified(&paddle.webhook_secret, header, &body, now()) {
        return StatusCode::UNAUTHORIZED;
    }
    let Ok(event) = serde_json::from_slice::<Value>(&body) else {
        return StatusCode::BAD_REQUEST;
    };
    let kind = event["event_type"].as_str().unwrap_or_default();
    if !kind.starts_with("subscription.") {
        // Transactions, customers, payouts: not what the plan depends on.
        return StatusCode::ACCEPTED;
    }
    match subscription(&app, paddle, &event["data"]).await {
        Ok(()) => StatusCode::OK,
        Err(err) => {
            tracing::error!("paddle {kind}: {err:#}");
            // Paddle retries a failure; a shape this does not know would fail
            // forever, so only the database's own failures ask for a retry.
            if err.downcast_ref::<sqlx::Error>().is_some() {
                StatusCode::INTERNAL_SERVER_ERROR
            } else {
                StatusCode::ACCEPTED
            }
        }
    }
}

/// Makes the account's plan what the subscription says.
async fn subscription(app: &AppState, paddle: &Paddle, data: &Value) -> anyhow::Result<()> {
    let customer = data["customer_id"].as_str().unwrap_or_default();
    let account = account_for(app, paddle, data, customer).await?;
    // Paddle's statuses already mean what the plan needs: `active` and
    // `trialing` pay; `past_due`, `paused` and `canceled` do not.
    let status = data["status"].as_str().unwrap_or("active");
    let period_end = data["current_billing_period"]["ends_at"].as_str();
    sqlx::query(
        "insert into subscriptions (account_id, plan, status, provider, provider_customer, provider_subscription, current_period_end, updated_at)
         values ($1, $2, $3, 'paddle', $4, $5, $6::timestamptz, now())
         on conflict (account_id) do update set plan = $2, status = $3, provider = 'paddle',
             provider_customer = $4, provider_subscription = $5, current_period_end = $6::timestamptz,
             updated_at = now()",
    )
    .bind(&account.id)
    .bind(PAID_PLAN)
    .bind(status)
    .bind(Some(customer).filter(|c| !c.is_empty()))
    .bind(data["id"].as_str())
    .bind(period_end)
    .execute(&app.db)
    .await?;
    tracing::info!("plan {PAID_PLAN} ({status}) for an account");
    Ok(())
}

/// Whose subscription this is: the account id the checkout carried; else
/// the account this Paddle customer already paid for; else, with an API key,
/// the customer's email (a checkout opened from elsewhere).
async fn account_for(
    app: &AppState,
    paddle: &Paddle,
    data: &Value,
    customer: &str,
) -> anyhow::Result<accounts::Account> {
    if let Some(id) = data["custom_data"]["account_id"].as_str()
        && let Some(account) = accounts::get(&app.db, id).await?
    {
        return Ok(account);
    }
    if !customer.is_empty() {
        let known: Option<String> = sqlx::query_scalar(
            "select account_id from subscriptions where provider = 'paddle' and provider_customer = $1",
        )
        .bind(customer)
        .fetch_optional(&app.db)
        .await?;
        if let Some(id) = known
            && let Some(account) = accounts::get(&app.db, &id).await?
        {
            return Ok(account);
        }
        if paddle.api_key.is_some() {
            let found = api(
                app,
                paddle,
                reqwest::Method::GET,
                &format!("/customers/{customer}"),
                None,
            )
            .await?;
            if let Some(email) = found["data"]["email"].as_str() {
                return Ok(accounts::find_or_create(&app.db, email).await?);
            }
        }
    }
    anyhow::bail!("a subscription with no account to give it to")
}

/// A call to Paddle's API with the server-side key.
async fn api(
    app: &AppState,
    paddle: &Paddle,
    method: reqwest::Method,
    path: &str,
    body: Option<Value>,
) -> anyhow::Result<Value> {
    let key = paddle
        .api_key
        .as_deref()
        .ok_or_else(|| anyhow::anyhow!("no PADDLE_API_KEY"))?;
    let mut request = app
        .http
        .request(method, format!("{}{path}", paddle.api_url))
        .bearer_auth(key)
        .timeout(std::time::Duration::from_secs(15));
    if let Some(body) = body {
        request = request.json(&body);
    }
    let response = request.send().await?;
    let status = response.status();
    let answer: Value = response.json().await.unwrap_or(Value::Null);
    anyhow::ensure!(
        status.is_success(),
        "paddle {path}: {status} {}",
        answer["error"]["detail"].as_str().unwrap_or_default()
    );
    Ok(answer)
}

/// The account's Paddle subscription, if it ever had one:
/// `(customer, subscription, status)`.
async fn paddle_subscription(
    app: &AppState,
    account: &str,
) -> sqlx::Result<Option<(Option<String>, Option<String>, String)>> {
    sqlx::query_as(
        "select provider_customer, provider_subscription, status from subscriptions
         where account_id = $1 and provider = 'paddle'",
    )
    .bind(account)
    .fetch_optional(&app.db)
    .await
}

/// What the account page offers about paying: subscribe, or manage the
/// subscription the account has. Empty when the paid plan is off.
pub async fn account_links(app: &AppState, account: &accounts::Account, lang: Lang) -> String {
    let Some(paddle) = app.config.paddle.as_ref() else {
        return String::new();
    };
    let current = paddle_subscription(app, &account.id).await.ok().flatten();
    match current {
        Some((Some(_), _, status)) if status != "canceled" => {
            if paddle.api_key.is_some() {
                format!(
                    r#"<a class="button ghost" href="/billing/manage">{}</a>"#,
                    lang.pick("Gerenciar assinatura", "Manage subscription")
                )
            } else {
                format!(
                    r#"<p class="muted">{}</p>"#,
                    lang.pick(
                        "Para trocar o cartão ou cancelar, use o link do recibo que o Paddle mandou por e-mail.",
                        "To change the card or cancel, use the link in the receipt Paddle emailed you."
                    )
                )
            }
        }
        _ => format!(
            r#"<a class="button ghost" href="/billing/checkout">{}</a>"#,
            lang.pick(
                "Apoiar com um cafezinho (plano pago)",
                "Support with a coffee (paid plan)"
            )
        ),
    }
}

/// A JavaScript string literal for `text`, safe inside a `<script>`.
fn js(text: &str) -> String {
    serde_json::to_string(text)
        .unwrap_or_else(|_| "\"\"".to_owned())
        .replace("</", "<\\/")
}

/// The checkout page: the plan, its two prices as Paddle shows them to this
/// buyer (currency and tax of their country), and Paddle's checkout on top.
pub async fn checkout(State(app): State<AppState>, headers: HeaderMap) -> Response {
    let lang = Lang::of(&headers);
    let Some(paddle) = app.config.paddle.as_ref() else {
        return StatusCode::NOT_FOUND.into_response();
    };
    let Some(account) = login::signed_in(&app, &headers).await else {
        return Redirect::to("/login?return_to=/billing/checkout").into_response();
    };
    let body = format!(
        r#"<h1>{title}</h1>
<p>{intro}</p>
<ul class="muted">
<li>{photos}</li>
<li>{projects}</li>
<li>{drafts}</li>
</ul>
<button id="monthly" type="button">{monthly}&nbsp;<span data-price="monthly">· US$ 5</span></button>
<button id="yearly" class="ghost" type="button">{yearly}&nbsp;<span data-price="yearly">· US$ 48</span></button>
<p class="muted" style="margin-top:14px">{fine}</p>
<p id="failed" class="muted" hidden>{failed}</p>
<a class="button ghost" href="/account">{back}</a>
<script src="https://cdn.paddle.com/paddle/v2/paddle.js"></script>
<script>
(function () {{
  var prices = {{ monthly: {monthly_id}, yearly: {yearly_id} }};
  var failed = function () {{ document.getElementById("failed").hidden = false; }};
  if (!window.Paddle) {{ failed(); return; }}
  if ({sandbox}) Paddle.Environment.set("sandbox");
  Paddle.Initialize({{ token: {token} }});
  Object.keys(prices).forEach(function (kind) {{
    document.getElementById(kind).onclick = function () {{
      Paddle.Checkout.open({{
        items: [{{ priceId: prices[kind], quantity: 1 }}],
        customer: {{ email: {email} }},
        customData: {{ account_id: {account_id} }},
        settings: {{ displayMode: "overlay", locale: {locale}, successUrl: {success} }}
      }});
    }};
  }});
  Paddle.PricePreview({{ items: [
    {{ priceId: prices.monthly, quantity: 1 }}, {{ priceId: prices.yearly, quantity: 1 }}
  ] }}).then(function (preview) {{
    var lines = preview.data.details.lineItems;
    ["monthly", "yearly"].forEach(function (kind, i) {{
      var total = lines[i] && lines[i].formattedTotals && lines[i].formattedTotals.total;
      if (total) document.querySelector('[data-price="' + kind + '"]').textContent = "· " + total;
    }});
  }}).catch(function () {{}});
}})();
</script>"#,
        title = lang.pick("Plano Supporter", "Supporter plan"),
        intro = lang.pick(
            "O plano gratuito continua valendo. O Supporter ajuda a manter o servidor e amplia o que custa processamento:",
            "The free plan stays. Supporter helps pay for the server and raises what takes server time:"
        ),
        photos = lang.pick(
            "Fotos em alta qualidade (100 por mês)",
            "High-quality photos (100 a month)"
        ),
        projects = lang.pick(
            "100 projetos e 2 GB na nuvem",
            "100 projects and 2 GB in the cloud"
        ),
        drafts = lang.pick("200 fotos rascunho por dia", "200 draft photos a day"),
        monthly = lang.pick("Mensal", "Monthly"),
        yearly = lang.pick("Anual", "Yearly"),
        fine = lang.pick(
            "O pagamento é feito pelo Paddle, nosso revendedor e vendedor oficial (merchant of record), que cuida da cobrança, dos impostos e do recibo. Cancele quando quiser: o plano vale até o fim do período pago.",
            "Payment is handled by Paddle, our reseller and merchant of record, which takes care of billing, tax and the receipt. Cancel any time: the plan lasts until the end of the paid period."
        ),
        failed = lang.pick(
            "O checkout não carregou. Desative o bloqueador de anúncios para esta página e recarregue.",
            "The checkout did not load. Turn off the ad blocker for this page and reload."
        ),
        back = lang.pick("Voltar para a conta", "Back to the account"),
        monthly_id = js(&paddle.monthly_price),
        yearly_id = js(&paddle.yearly_price),
        sandbox = if paddle.sandbox { "true" } else { "false" },
        token = js(&paddle.client_token),
        email = js(&account.email),
        account_id = js(&account.id),
        locale = js(lang.pick("pt", "en")),
        success = js(&format!("{}/account?paid=1", app.config.public_url)),
    );
    page(lang, lang.pick("Plano Supporter", "Supporter plan"), &body).into_response()
}

/// Paddle's customer portal for the account's subscription: change the card,
/// see receipts, cancel.
pub async fn manage(State(app): State<AppState>, headers: HeaderMap) -> Response {
    let lang = Lang::of(&headers);
    let Some(paddle) = app.config.paddle.as_ref() else {
        return StatusCode::NOT_FOUND.into_response();
    };
    let Some(account) = login::signed_in(&app, &headers).await else {
        return Redirect::to("/login?return_to=/billing/manage").into_response();
    };
    let Ok(Some((Some(customer), subscription, _))) = paddle_subscription(&app, &account.id).await
    else {
        return Redirect::to("/account").into_response();
    };
    let body = json!({ "subscription_ids": subscription.into_iter().collect::<Vec<_>>() });
    match api(
        &app,
        paddle,
        reqwest::Method::POST,
        &format!("/customers/{customer}/portal-sessions"),
        Some(body),
    )
    .await
    {
        Ok(session) => match session["data"]["urls"]["general"]["overview"].as_str() {
            Some(url) if url.starts_with("https://") => Redirect::to(url).into_response(),
            _ => unavailable(lang),
        },
        Err(err) => {
            tracing::error!("paddle portal: {err:#}");
            unavailable(lang)
        }
    }
}

fn unavailable(lang: Lang) -> Response {
    let body = format!(
        "<h1>{}</h1><p class=\"muted\">{}</p><a class=\"button ghost\" href=\"/account\">{}</a>",
        lang.pick("Não deu agora", "Not right now"),
        lang.pick(
            "O portal de assinatura não abriu. Tente de novo em instantes, ou use o link do recibo que o Paddle mandou por e-mail.",
            "The subscription portal did not open. Try again in a moment, or use the link in the receipt Paddle emailed you."
        ),
        lang.pick("Voltar para a conta", "Back to the account"),
    );
    (StatusCode::BAD_GATEWAY, page(lang, "Subscription", &body)).into_response()
}

/// Cancels the account's subscription at the end of the paid period, when
/// the account is closed: nobody is charged for an account that is gone.
/// Without an API key there is nothing to call; the page says how.
pub async fn cancel_for_closed(app: &AppState, account: &str) {
    let Some(paddle) = app.config.paddle.as_ref() else {
        return;
    };
    if paddle.api_key.is_none() {
        return;
    }
    let Ok(Some((_, Some(subscription), status))) = paddle_subscription(app, account).await else {
        return;
    };
    if status == "canceled" {
        return;
    }
    if let Err(err) = api(
        app,
        paddle,
        reqwest::Method::POST,
        &format!("/subscriptions/{subscription}/cancel"),
        Some(json!({ "effective_from": "next_billing_period" })),
    )
    .await
    {
        tracing::error!("cancelling a closed account's subscription: {err:#}");
    }
}

#[derive(Debug, Deserialize)]
pub struct Paid {
    paid: Option<String>,
}

/// The note the account page shows right after the checkout: the webhook
/// may take a few seconds to change the plan.
pub fn thanks(lang: Lang, query: &Query<Paid>) -> String {
    if query.paid.is_none() {
        return String::new();
    }
    format!(
        r#"<p class="muted"><b>{}</b> {}</p>"#,
        lang.pick("Obrigado pelo apoio!", "Thank you for the support!"),
        lang.pick(
            "O plano muda em alguns segundos; recarregue a página se ainda não mudou.",
            "The plan changes within seconds; reload the page if it has not yet."
        )
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    /// RFC 4231 test case 2.
    #[test]
    fn hmac_follows_the_rfc() {
        assert_eq!(
            hex(&hmac(b"Jefe", b"what do ya want for nothing?")),
            "5bdcc146bf60754e6a042426089575c75a003f089d2739839dec58b964ec3843"
        );
    }

    #[test]
    fn only_paddles_recent_deliveries_pass() {
        // Made up here, in the shape of a Paddle secret: nothing real is
        // written in the code, and a secret scanner has nothing to flag.
        let secret = &format!("pdl_ntfset_{}", "test-key-not-a-secret");
        let body = br#"{"event_type":"subscription.activated"}"#;
        let header = signature(secret, 1_000_000, body);
        assert!(verified(secret, &header, body, 1_000_010));
        assert!(
            !verified(secret, &header, br#"{"event_type":"other"}"#, 1_000_010),
            "the body is signed"
        );
        assert!(
            !verified("pdl_ntfset_another", &header, body, 1_000_010),
            "another secret"
        );
        assert!(
            !verified(secret, &header, body, 1_000_000 + 3600),
            "an old delivery is a replay"
        );
        assert!(!verified(secret, "", body, 1_000_000), "no header");
        assert!(!verified(secret, "h1=abc", body, 1_000_000), "no timestamp");
        // While the secret rotates, one of several h1 values matches.
        let rotating = format!(
            "ts=1000000;h1=deadbeef;{}",
            header.split_once(';').unwrap().1
        );
        assert!(verified(secret, &rotating, body, 1_000_000));
    }

    #[test]
    fn a_script_string_cannot_close_the_script() {
        assert_eq!(js("a</script>b"), r#""a<\/script>b""#);
        assert_eq!(js("x\"y"), r#""x\"y""#);
    }
}
