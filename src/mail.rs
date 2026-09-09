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
    Http {
        endpoint: String,
        token: String,
    },
    Cloudflare {
        account_id: String,
        token: String,
        from: String,
    },
}

impl Mailer {
    pub fn from_env() -> Self {
        let last_verify_url = Arc::new(Mutex::new(None));

        // 1. Cloudflare Email Service API
        if let (Ok(token), Ok(account_id)) = (
            std::env::var("OPENHUB_CF_EMAIL_TOKEN"),
            std::env::var("OPENHUB_CF_ACCOUNT_ID"),
        ) && !token.is_empty()
            && !account_id.is_empty()
        {
            let from = std::env::var("OPENHUB_CF_EMAIL_FROM")
                .unwrap_or_else(|_| "welcome@openhub.run".to_string());
            return Self {
                inner: MailerKind::Cloudflare {
                    account_id,
                    token,
                    from,
                },
                last_verify_url,
            };
        }

        // 2. Generic HTTP mail endpoint
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

    pub fn skips_email(&self) -> bool {
        matches!(self.inner, MailerKind::Log)
    }

    pub async fn send_verification_code(&self, to: &str, code: &str) -> Result<()> {
        self.send_code_email(to, code, "complete your registration", "verification code")
            .await
    }

    pub async fn send_password_reset_code(&self, to: &str, code: &str) -> Result<()> {
        self.send_code_email(to, code, "reset your password", "password reset code")
            .await
    }

    async fn send_code_email(
        &self,
        to: &str,
        code: &str,
        action: &str,
        log_label: &str,
    ) -> Result<()> {
        let subject = format!("Your OpenHub verification code is {code}");
        let text = format!(
            "Your OpenHub verification code is: {code}\n\nUse it to {action}. This code expires in 10 minutes."
        );
        let html = format!(
            "<div style=\"font-family: -apple-system, BlinkMacSystemFont, 'Segoe UI', Roboto, sans-serif; max-width: 480px; margin: 0 auto; padding: 24px; border: 1px solid #e5e7eb; border-radius: 8px;\">\
                <h2 style=\"color: #111827; margin-bottom: 16px;\">OpenHub Verification Code</h2>\
                <p style=\"color: #4b5563; font-size: 15px;\">Please use the following verification code to {action}:</p>\
                <div style=\"font-size: 32px; font-weight: bold; letter-spacing: 6px; color: #2563eb; background: #f3f4f6; padding: 16px; text-align: center; border-radius: 6px; margin: 20px 0;\">\
                    {code}\
                </div>\
                <p style=\"color: #6b7280; font-size: 13px; margin-top: 16px;\">This code will expire in 10 minutes. If you did not request this, please ignore this email.</p>\
            </div>"
        );

        match &self.inner {
            MailerKind::Log => {
                tracing::info!(to, code, kind = log_label, "OPENHUB_MAIL_* not set");
                Ok(())
            }
            MailerKind::Cloudflare {
                account_id,
                token,
                from,
            } => {
                let client = reqwest::Client::new();
                let url = format!(
                    "https://api.cloudflare.com/client/v4/accounts/{account_id}/email/sending/send"
                );
                let res = client
                    .post(&url)
                    .bearer_auth(token)
                    .json(&json!({
                        "to": to,
                        "from": from,
                        "subject": subject,
                        "text": text,
                        "html": html,
                    }))
                    .send()
                    .await
                    .map_err(|e| anyhow::anyhow!("cloudflare email request failed: {e}"))?;

                if !res.status().is_success() {
                    let status = res.status();
                    let body = res.text().await.unwrap_or_default();
                    return Err(anyhow::anyhow!("cloudflare email error {status}: {body}").into());
                }
                Ok(())
            }
            MailerKind::Http { endpoint, token } => {
                let client = reqwest::Client::new();
                let res = client
                    .post(endpoint)
                    .bearer_auth(token)
                    .json(&json!({
                        "to": to,
                        "subject": subject,
                        "text": text,
                        "html": html,
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

    pub async fn send_verify_email(&self, to: &str, verify_url: &str) -> Result<()> {
        if let Ok(mut g) = self.last_verify_url.lock() {
            *g = Some(verify_url.to_string());
        }
        match &self.inner {
            MailerKind::Log => {
                tracing::info!(to, verify_url, "verify email (OPENHUB_MAIL_* not set)");
                Ok(())
            }
            MailerKind::Cloudflare {
                account_id,
                token,
                from,
            } => {
                let client = reqwest::Client::new();
                let url = format!(
                    "https://api.cloudflare.com/client/v4/accounts/{account_id}/email/sending/send"
                );
                let res = client
                    .post(&url)
                    .bearer_auth(token)
                    .json(&json!({
                        "to": to,
                        "from": from,
                        "subject": "Verify your OpenHub account",
                        "text": format!("Set your password: {verify_url}"),
                        "html": format!("<p>Set your password: <a href=\"{verify_url}\">{verify_url}</a></p>"),
                    }))
                    .send()
                    .await
                    .map_err(|e| anyhow::anyhow!("cloudflare email request failed: {e}"))?;
                if !res.status().is_success() {
                    let status = res.status();
                    let body = res.text().await.unwrap_or_default();
                    return Err(anyhow::anyhow!("cloudflare email error {status}: {body}").into());
                }
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
