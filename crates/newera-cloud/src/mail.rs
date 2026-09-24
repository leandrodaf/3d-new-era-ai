//! Sending the sign-in link.

use crate::config::Mail;
use crate::pages::{Lang, escape};

/// Emails a sign-in link to `to`.
///
/// # Errors
///
/// When the mail service refuses or cannot be reached.
pub async fn send_login_link(
    http: &reqwest::Client,
    mail: &Mail,
    to: &str,
    link: &str,
    lang: Lang,
) -> anyhow::Result<()> {
    let subject = lang.pick(
        "Seu link para entrar no 3D New Era AI",
        "Your 3D New Era AI sign-in link",
    );
    let intro = lang.pick(
        "Use o botão abaixo para entrar. O link vale por 15 minutos e uma vez só.",
        "Use the button below to sign in. The link works once, for 15 minutes.",
    );
    let button = lang.pick("Entrar", "Sign in");
    let ignore = lang.pick(
        "Se não foi você que pediu, pode ignorar este e-mail.",
        "If you did not ask for it, you can ignore this email.",
    );
    let text = format!("{intro}\n\n{link}\n\n{ignore}\n");
    let html = format!(
        r#"<p>{intro}</p><p><a href="{href}" style="display:inline-block;padding:10px 18px;background:#161513;color:#f4f1ea;border-radius:8px;text-decoration:none;font-weight:600">{button}</a></p><p style="color:#6d6a62;font-size:13px">{ignore}</p>"#,
        href = escape(link),
    );
    match mail {
        Mail::Off => anyhow::bail!("sign-in by email is not set up"),
        Mail::Log => {
            tracing::warn!("sign-in link for {to} (NEWERA_MAIL=log): {link}");
            Ok(())
        }
        Mail::Resend { api_key, from } => {
            let response = http
                .post("https://api.resend.com/emails")
                .bearer_auth(api_key)
                .json(&serde_json::json!({
                    "from": from,
                    "to": [to],
                    "subject": subject,
                    "text": text,
                    "html": html,
                }))
                .send()
                .await?;
            if !response.status().is_success() {
                let status = response.status();
                let body = response.text().await.unwrap_or_default();
                anyhow::bail!("resend answered {status}: {body}");
            }
            Ok(())
        }
    }
}
