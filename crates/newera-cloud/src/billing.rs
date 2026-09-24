//! Paid plans, as Polar tells them.
//!
//! Polar sells the subscription (it is the merchant of record, so the tax
//! and the invoice are its); this only listens. Each subscription event
//! says who, which product and until when, and the account's plan follows.
//! The tools never ask "does this account pay?": they read the plan's
//! limits, and this is what changes the plan.
//!
//! Nothing here is shown inside an AI chat: the directories forbid selling
//! there, and the only way to subscribe is the account page.

use axum::body::Bytes;
use axum::extract::State;
use axum::http::{HeaderMap, StatusCode};
use base64::Engine as _;
use serde_json::Value;
use sha2::Sha256;

use crate::{AppState, accounts, secret};

/// How old a delivery may be, in seconds, before it is refused as a replay.
const TOLERANCE_SECS: i64 = 300;

/// The key a Polar webhook secret signs with: a `whsec_` secret carries it
/// in base64; an older one is used as its own bytes.
fn key(secret: &str) -> Vec<u8> {
    match secret.strip_prefix("whsec_") {
        Some(encoded) => base64::engine::general_purpose::STANDARD
            .decode(encoded)
            .unwrap_or_else(|_| secret.as_bytes().to_vec()),
        None => secret.as_bytes().to_vec(),
    }
}

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

/// Whether a delivery is Polar's: the Standard Webhooks signature over
/// `id.timestamp.body`, recent enough not to be a replay.
pub fn verified(secret: &str, headers: &HeaderMap, body: &[u8], now: i64) -> bool {
    let header = |name: &str| headers.get(name).and_then(|v| v.to_str().ok());
    let (Some(id), Some(timestamp), Some(signatures)) = (
        header("webhook-id"),
        header("webhook-timestamp"),
        header("webhook-signature"),
    ) else {
        return false;
    };
    let Ok(sent) = timestamp.parse::<i64>() else {
        return false;
    };
    if (now - sent).abs() > TOLERANCE_SECS {
        return false;
    }
    let mut message = format!("{id}.{timestamp}.").into_bytes();
    message.extend_from_slice(body);
    let expected = base64::engine::general_purpose::STANDARD.encode(hmac(&key(secret), &message));
    // The header may carry several, space-separated, each `v1,<base64>`.
    signatures
        .split_whitespace()
        .filter_map(|s| s.strip_prefix("v1,"))
        .any(|given| secret::same(given, &expected))
}

/// The `webhook-signature` a delivery of `body` would carry: what Polar
/// computes, here for tests and for trying the endpoint by hand.
pub fn signature(secret: &str, id: &str, timestamp: i64, body: &[u8]) -> String {
    let mut message = format!("{id}.{timestamp}.").into_bytes();
    message.extend_from_slice(body);
    format!(
        "v1,{}",
        base64::engine::general_purpose::STANDARD.encode(hmac(&key(secret), &message))
    )
}

/// Polar's webhook endpoint.
pub async fn webhook(State(app): State<AppState>, headers: HeaderMap, body: Bytes) -> StatusCode {
    let Some(secret) = app.config.polar_webhook_secret.as_deref() else {
        return StatusCode::NOT_FOUND;
    };
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| i64::try_from(d.as_secs()).unwrap_or(0));
    if !verified(secret, &headers, &body, now) {
        return StatusCode::UNAUTHORIZED;
    }
    let Ok(event) = serde_json::from_slice::<Value>(&body) else {
        return StatusCode::BAD_REQUEST;
    };
    let kind = event["type"].as_str().unwrap_or_default();
    if !kind.starts_with("subscription.") {
        // Orders, refunds, checkouts: not what the plan depends on.
        return StatusCode::ACCEPTED;
    }
    match subscription(&app, &event["data"]).await {
        Ok(()) => StatusCode::OK,
        Err(err) => {
            tracing::error!("polar {kind}: {err:#}");
            // Polar retries a failure; a shape this does not know would fail
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
async fn subscription(app: &AppState, data: &Value) -> anyhow::Result<()> {
    let customer = &data["customer"];
    let email = customer["email"].as_str().unwrap_or_default();
    // The account id travels as the customer's external id when the checkout
    // started from the account page; the address is the fallback.
    let by_id = match customer["external_id"].as_str() {
        Some(id) => accounts::get(&app.db, id).await?,
        None => None,
    };
    let account = match by_id {
        Some(account) => account,
        None if !email.is_empty() => accounts::find_or_create(&app.db, email).await?,
        None => anyhow::bail!("a subscription with no customer to give it to"),
    };
    let product = data["product_id"]
        .as_str()
        .or_else(|| data["product"]["id"].as_str())
        .unwrap_or_default();
    let plan = app
        .config
        .polar_plans
        .iter()
        .find(|(id, _)| id == product)
        .map_or("supporter", |(_, plan)| plan.as_str());
    let status = data["status"].as_str().unwrap_or("active");
    sqlx::query(
        "insert into subscriptions (account_id, plan, status, provider, provider_customer, provider_subscription, current_period_end, updated_at)
         values ($1, $2, $3, 'polar', $4, $5, $6::timestamptz, now())
         on conflict (account_id) do update set plan = $2, status = $3, provider_customer = $4,
             provider_subscription = $5, current_period_end = $6::timestamptz, updated_at = now()",
    )
    .bind(&account.id)
    .bind(plan)
    .bind(status)
    .bind(customer["id"].as_str())
    .bind(data["id"].as_str())
    .bind(data["current_period_end"].as_str())
    .execute(&app.db)
    .await?;
    tracing::info!("plan {plan} ({status}) for an account");
    Ok(())
}

/// Where the account page sends someone who wants the paid plan, with who
/// they are filled in so the subscription finds its account.
pub fn checkout_link(app: &AppState, account: &accounts::Account) -> Option<String> {
    let base = app.config.polar_checkout_url.as_deref()?;
    let join = if base.contains('?') { '&' } else { '?' };
    Some(format!(
        "{base}{join}customer_email={}&customer_external_id={}",
        crate::login::url_encode(&account.email),
        crate::login::url_encode(&account.id)
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// RFC 4231 test case 2.
    #[test]
    fn hmac_follows_the_rfc() {
        let mac = hmac(b"Jefe", b"what do ya want for nothing?");
        let hex = mac.iter().fold(String::new(), |mut out, b| {
            use std::fmt::Write as _;
            let _ = write!(out, "{b:02x}");
            out
        });
        assert_eq!(
            hex,
            "5bdcc146bf60754e6a042426089575c75a003f089d2739839dec58b964ec3843"
        );
    }

    fn signed(secret: &str, id: &str, at: i64, body: &[u8]) -> HeaderMap {
        let mut message = format!("{id}.{at}.").into_bytes();
        message.extend_from_slice(body);
        let signature =
            base64::engine::general_purpose::STANDARD.encode(hmac(&key(secret), &message));
        let mut headers = HeaderMap::new();
        headers.insert("webhook-id", id.parse().unwrap());
        headers.insert("webhook-timestamp", at.to_string().parse().unwrap());
        headers.insert(
            "webhook-signature",
            format!("v1,{signature}").parse().unwrap(),
        );
        headers
    }

    #[test]
    fn only_polars_recent_deliveries_pass() {
        // Made up here, in the shape of a Polar secret: nothing real is
        // written in the code, and a secret scanner has nothing to flag.
        let secret = &format!(
            "whsec_{}",
            base64::engine::general_purpose::STANDARD.encode("test key, not a secret")
        );
        let body = br#"{"type":"subscription.active"}"#;
        let headers = signed(secret, "msg_1", 1_000_000, body);
        assert!(verified(secret, &headers, body, 1_000_010));
        assert!(
            !verified(secret, &headers, b"{\"type\":\"other\"}", 1_000_010),
            "the body is signed"
        );
        assert!(
            !verified(
                &format!(
                    "whsec_{}",
                    base64::engine::general_purpose::STANDARD.encode("another key")
                ),
                &headers,
                body,
                1_000_010
            ),
            "another secret"
        );
        assert!(
            !verified(secret, &headers, body, 1_000_000 + 3600),
            "an old delivery is a replay"
        );
        // An older, plain secret signs with its own bytes.
        let plain = "polar_whs_plain";
        assert!(verified(plain, &signed(plain, "m", 5, body), body, 5));
    }
}
