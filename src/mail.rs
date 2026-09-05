use std::sync::{Arc, Mutex};

use serde_json::json;

use crate::error::Result;

/// Outbound mail. Core never speaks SMTP.
#[derive(Clone)]
pub struct Mailer {
    inner: MailerKind,
    /// Last verify URL (tests / when no HTTP endpoint is configured).
    pub last_verify_url: Arc<Mutex<Option<String>>>,
}

#[derive(Clone)]
enum MailerKind {
    Log,
    Http { endpoint: String, token: String },
}

impl Mailer {
    pub fn from_env() -> Self {
        let last_verify_url = Arc::new(Mutex::new(None));
        match (
            std::env::var("OPENHUB_MAIL_ENDPOINT").ok(),
            std::env::var("OPENHUB_MAIL_TOKEN").ok(),
        ) {
            (Some(endpoint), Some(token)) if !endpoint.is_empty() && !token.is_empty() => Self {
                inner: MailerKind::Http { endpoint, token },
                last_verify_url,
            },
            _ => Self {
                inner: MailerKind::Log,
                last_verify_url,
            },
        }
    }

    pub fn log() -> Self {
        Self {
            inner: MailerKind::Log,
            last_verify_url: Arc::new(Mutex::new(None)),
        }
    }

    pub async fn send_verify_email(&self, to: &str, verify_url: &str) -> Result<()> {
        if let Ok(mut g) = self.last_verify_url.lock() {
            *g = Some(verify_url.to_string());
        }
        match &self.inner {
            MailerKind::Log => {
                tracing::info!(to, verify_url, "verify email (OPENHUB_MAIL_* not set)");
                Ok(())
            }
            MailerKind::Http { endpoint, token } => {
                let client = reqwest::Client::new();
                let res = client
                    .post(endpoint)
                    .bearer_auth(token)
                    .json(&json!({
                        "to": to,
                        "subject": "Verify your OpenHub account",
                        "text": format!("Set your password: {verify_url}"),
                    }))
                    .send()
                    .await
                    .map_err(|e| anyhow::anyhow!("mail request failed: {e}"))?;
                if !res.status().is_success() {
                    let status = res.status();
                    let body = res.text().await.unwrap_or_default();
                    return Err(anyhow::anyhow!("mail provider {status}: {body}").into());
                }
                Ok(())
            }
        }
    }
}
