use axum::Json;
use axum::extract::State;
use axum::http::{HeaderMap, header};
use serde::Deserialize;
use serde_json::{Value, json};
use sha2::{Digest, Sha256};

use crate::error::{Error, Result};
use crate::server::HubState;
use crate::store;

#[derive(Debug, Deserialize)]
pub struct SendCodeRequest {
    pub email: String,
}

#[derive(Debug, Deserialize)]
pub struct VerifyCodeRequest {
    pub email: String,
    pub code: String,
}

#[derive(Debug, Deserialize)]
pub struct RegisterRequest {
    pub email: String,
    pub code: Option<String>,
    pub username: Option<String>,
    pub password: String,
}

#[derive(Debug, Deserialize)]
pub struct PasswordRequest {
    pub token: String,
    pub password: String,
}

#[derive(Debug, Deserialize)]
pub struct LoginRequest {
    pub email: String,
    pub password: String,
}

#[derive(Debug, Deserialize)]
pub struct ChangePasswordRequest {
    pub current_password: String,
    pub new_password: String,
}

#[derive(Debug, Deserialize)]
pub struct CreatePatRequest {
    pub name: String,
    pub expires_in_days: Option<i64>,
}

/// Sends a 6-digit verification code to the given email address.
pub async fn send_code(
    State(hub): State<HubState>,
    Json(body): Json<SendCodeRequest>,
) -> Result<Json<Value>> {
    let email = normalize_email(&body.email)?;
    if let Some(user) = store::find_user_by_email(&hub.db, &email).await?
        && user.email_verified_at.is_some()
        && user.password_hash.is_some()
    {
        return Err(Error::InvalidRequest("email already registered".into()));
    }

    let user = match store::find_user_by_email(&hub.db, &email).await? {
        Some(u) => u,
        None => store::insert_user(&hub.db, &email).await?,
    };

    // Generate 6-digit code
    let code: u32 = rand::random::<u32>() % 900_000 + 100_000;
    let code_str = code.to_string();

    // Store in auth_tokens with 10-minute expiry
    store::insert_token_minutes(&hub.db, &user.id, "email_code", &hash_token(&code_str), 10)
        .await?;

    // Send code via configured mailer
    hub.mail.send_verification_code(&email, &code_str).await?;

    Ok(Json(json!({ "ok": true })))
}

/// Validates that the verification code entered for the email is valid before advancing.
pub async fn verify_code(
    State(hub): State<HubState>,
    Json(body): Json<VerifyCodeRequest>,
) -> Result<Json<Value>> {
    let email = normalize_email(&body.email)?;
    let user = store::find_user_by_email(&hub.db, &email)
        .await?
        .ok_or_else(|| Error::InvalidRequest("invalid or expired verification code".into()))?;

    if hub.mail.skips_email() {
        return Ok(Json(json!({ "ok": true })));
    }

    let code = body.code.trim();
    if code.is_empty() {
        return Err(Error::InvalidRequest("verification code required".into()));
    }

    // Check code in db
    let row = sqlx::query(
        "SELECT id, user_id, expires_at FROM auth_tokens WHERE purpose = ? AND token_hash = ?",
    )
    .bind("email_code")
    .bind(hash_token(code))
    .fetch_optional(&hub.db)
    .await?;

    let Some(row) = row else {
        return Err(Error::Unauthorized(
            "invalid or expired verification code".into(),
        ));
    };

    use sqlx::Row;
    let user_id: String = row.get("user_id");
    let expires_at: String = row.get("expires_at");
    if user_id != user.id || expires_at < crate::store::now() {
        return Err(Error::Unauthorized(
            "invalid or expired verification code".into(),
        ));
    }

    Ok(Json(json!({ "ok": true })))
}

/// Registers the user by validating the email verification code, updating username, and setting password.
pub async fn register(
    State(hub): State<HubState>,
    Json(body): Json<RegisterRequest>,
) -> Result<Json<Value>> {
    let email = normalize_email(&body.email)?;
    if body.password.len() < 8 {
        return Err(Error::InvalidRequest(
            "password must be at least 8 characters".into(),
        ));
    }

    let user = match store::find_user_by_email(&hub.db, &email).await? {
        Some(u) => u,
        None => store::insert_user(&hub.db, &email).await?,
    };

    // If user already registered with password and verified
    if user.email_verified_at.is_some() && user.password_hash.is_some() {
        return Err(Error::InvalidRequest("email already registered".into()));
    }

    if !hub.mail.skips_email() {
        let code = body.code.as_deref().unwrap_or("").trim();
        if code.is_empty() {
            return Err(Error::InvalidRequest("verification code required".into()));
        }

        let user_id = store::take_token(&hub.db, "email_code", &hash_token(code))
            .await?
            .ok_or_else(|| Error::Unauthorized("invalid or expired verification code".into()))?;

        if user_id != user.id {
            return Err(Error::Unauthorized("verification code mismatch".into()));
        }
    }

    // Validate and sanitize custom username if supplied
    let chosen_username = if let Some(raw_u) = body.username.as_deref().map(str::trim) {
        if !raw_u.is_empty() {
            let sanitized = normalize_username(raw_u)?;
            if let Some(existing) = store::find_user_by_username(&hub.db, &sanitized).await?
                && existing.id != user.id
            {
                return Err(Error::InvalidRequest("username is already taken".into()));
            }
            sanitized
        } else {
            user.username
        }
    } else {
        user.username
    };

    let hash = hash_password(&body.password)?;
    store::set_user_credentials(&hub.db, &user.id, &chosen_username, &hash).await?;
    let session = issue_session(&hub, &user.id).await?;
    Ok(Json(json!({ "ok": true, "token": session })))
}

pub async fn set_password(
    State(hub): State<HubState>,
    Json(body): Json<PasswordRequest>,
) -> Result<Json<Value>> {
    if body.password.len() < 8 {
        return Err(Error::InvalidRequest(
            "password must be at least 8 characters".into(),
        ));
    }
    let user_id = store::take_token(&hub.db, "verify", &hash_token(&body.token))
        .await?
        .ok_or_else(|| Error::Unauthorized("invalid or expired token".into()))?;
    let hash = hash_password(&body.password)?;
    store::set_password_and_verify(&hub.db, &user_id, &hash).await?;
    let session = issue_session(&hub, &user_id).await?;
    Ok(Json(json!({ "token": session })))
}

pub async fn login(
    State(hub): State<HubState>,
    Json(body): Json<LoginRequest>,
) -> Result<Json<Value>> {
    let email = normalize_email(&body.email)?;
    let user = store::find_user_by_email(&hub.db, &email)
        .await?
        .ok_or_else(|| Error::Unauthorized("invalid email or password".into()))?;
    if user.email_verified_at.is_none() {
        return Err(Error::Unauthorized("email not verified".into()));
    }
    let Some(ref stored) = user.password_hash else {
        return Err(Error::Unauthorized("invalid email or password".into()));
    };
    if !verify_password(&body.password, stored) {
        return Err(Error::Unauthorized("invalid email or password".into()));
    }
    let session = issue_session(&hub, &user.id).await?;
    Ok(Json(json!({ "token": session })))
}

pub async fn logout(State(hub): State<HubState>, headers: HeaderMap) -> Result<Json<Value>> {
    if let Some(raw) = bearer_from_headers(&headers) {
        store::delete_session(&hub.db, &hash_token(&raw)).await?;
    }
    Ok(Json(json!({ "ok": true })))
}

pub async fn me(State(hub): State<HubState>, headers: HeaderMap) -> Result<Json<Value>> {
    let user = user_from_headers(&hub, &headers).await?;
    Ok(Json(
        json!({ "id": user.id, "email": user.email, "username": user.username }),
    ))
}

pub async fn change_password(
    State(hub): State<HubState>,
    headers: HeaderMap,
    Json(body): Json<ChangePasswordRequest>,
) -> Result<Json<Value>> {
    if body.new_password.len() < 8 {
        return Err(Error::InvalidRequest(
            "password must be at least 8 characters".into(),
        ));
    }
    let user = user_from_headers(&hub, &headers).await?;
    let Some(ref stored) = user.password_hash else {
        return Err(Error::Unauthorized("invalid email or password".into()));
    };
    if !verify_password(&body.current_password, stored) {
        return Err(Error::Unauthorized("invalid email or password".into()));
    }
    let hash = hash_password(&body.new_password)?;
    store::set_password_and_verify(&hub.db, &user.id, &hash).await?;
    if let Some(raw) = bearer_from_headers(&headers) {
        store::delete_other_sessions(&hub.db, &user.id, &hash_token(&raw)).await?;
    }
    Ok(Json(json!({ "ok": true })))
}

pub async fn list_pats(State(hub): State<HubState>, headers: HeaderMap) -> Result<Json<Value>> {
    let user = user_from_headers(&hub, &headers).await?;
    let pats = store::list_personal_access_tokens(&hub.db, &user.id).await?;
    let list: Vec<Value> = pats
        .into_iter()
        .map(|p| {
            json!({
                "id": p.id,
                "name": p.name,
                "token_prefix": p.token_prefix,
                "expires_at": p.expires_at,
                "created_at": p.created_at,
                "last_used_at": p.last_used_at,
            })
        })
        .collect();
    Ok(Json(json!({ "tokens": list })))
}

pub async fn create_pat(
    State(hub): State<HubState>,
    headers: HeaderMap,
    Json(body): Json<CreatePatRequest>,
) -> Result<Json<Value>> {
    let name = body.name.trim();
    if name.is_empty() {
        return Err(Error::InvalidRequest("token name required".into()));
    }
    let user = user_from_headers(&hub, &headers).await?;

    // Generate secure token with oh_ prefix
    let raw_secret = random_token();
    let token = format!("oh_{raw_secret}");
    let token_hash = hash_token(&token);
    let prefix = format!("oh_{}...", &raw_secret[..6]);

    let expires_at = body
        .expires_in_days
        .filter(|d| *d > 0)
        .map(|days| (chrono::Utc::now() + chrono::Duration::days(days)).to_rfc3339());

    let pat = store::insert_personal_access_token(
        &hub.db,
        &user.id,
        name,
        &token_hash,
        &prefix,
        expires_at.as_deref(),
    )
    .await?;

    Ok(Json(json!({
        "id": pat.id,
        "name": pat.name,
        "token": token,
        "token_prefix": pat.token_prefix,
        "expires_at": pat.expires_at,
        "created_at": pat.created_at,
    })))
}

pub async fn delete_pat(
    State(hub): State<HubState>,
    headers: HeaderMap,
    axum::extract::Path(token_id): axum::extract::Path<String>,
) -> Result<Json<Value>> {
    let user = user_from_headers(&hub, &headers).await?;
    store::delete_personal_access_token(&hub.db, &user.id, &token_id).await?;
    Ok(Json(json!({ "ok": true })))
}

pub async fn user_from_headers(hub: &HubState, headers: &HeaderMap) -> Result<store::User> {
    let raw =
        bearer_from_headers(headers).ok_or_else(|| Error::Unauthorized("missing bearer".into()))?;
    store::find_session_user(&hub.db, &hash_token(&raw))
        .await?
        .ok_or_else(|| Error::Unauthorized("invalid session".into()))
}

pub async fn user_from_request<B>(
    hub: &HubState,
    req: &axum::http::Request<B>,
) -> Result<store::User> {
    user_from_headers(hub, req.headers()).await
}

pub fn bearer_from_headers(headers: &HeaderMap) -> Option<String> {
    headers
        .get(header::AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.strip_prefix("Bearer "))
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
}

async fn issue_session(hub: &HubState, user_id: &str) -> Result<String> {
    let raw = random_token();
    store::insert_token(&hub.db, user_id, "session", &hash_token(&raw), 24 * 30).await?;
    Ok(raw)
}

fn normalize_email(email: &str) -> Result<String> {
    let email = email.trim().to_lowercase();
    if email.len() < 3 || !email.contains('@') || email.contains(' ') {
        return Err(Error::InvalidRequest("invalid email".into()));
    }
    Ok(email)
}

fn normalize_username(username: &str) -> Result<String> {
    let username = username.trim().to_lowercase();
    if username.len() < 2 || username.len() > 39 {
        return Err(Error::InvalidRequest(
            "username must be between 2 and 39 characters".into(),
        ));
    }
    if !username
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
    {
        return Err(Error::InvalidRequest(
            "username may only contain alphanumeric characters, single hyphens or underscores"
                .into(),
        ));
    }
    Ok(username)
}

fn random_token() -> String {
    let bytes: [u8; 32] = rand::random();
    hex::encode(bytes)
}

pub fn hash_token(raw: &str) -> String {
    hex::encode(Sha256::digest(raw.as_bytes()))
}

fn hash_password(password: &str) -> Result<String> {
    use argon2::password_hash::{SaltString, rand_core::OsRng};
    use argon2::{Argon2, PasswordHasher};
    let salt = SaltString::generate(&mut OsRng);
    let hash = Argon2::default()
        .hash_password(password.as_bytes(), &salt)
        .map_err(|e| Error::Internal(anyhow::anyhow!(e.to_string())))?;
    Ok(hash.to_string())
}

fn verify_password(password: &str, stored: &str) -> bool {
    use argon2::{Argon2, PasswordHash, PasswordVerifier};
    let Ok(parsed) = PasswordHash::new(stored) else {
        return false;
    };
    Argon2::default()
        .verify_password(password.as_bytes(), &parsed)
        .is_ok()
}
