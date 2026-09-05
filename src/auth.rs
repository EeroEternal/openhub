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
pub struct RegisterRequest {
    pub email: String,
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

pub async fn register(
    State(hub): State<HubState>,
    Json(body): Json<RegisterRequest>,
) -> Result<Json<Value>> {
    let email = normalize_email(&body.email)?;
    let user = match store::find_user_by_email(&hub.db, &email).await? {
        Some(user) if user.email_verified_at.is_some() => {
            return Ok(Json(json!({ "ok": true })));
        }
        Some(user) => user,
        None => store::insert_user(&hub.db, &email).await?,
    };
    let raw = random_token();
    store::insert_token(&hub.db, &user.id, "verify", &hash_token(&raw), 24).await?;
    let url = format!(
        "{}/verify?token={raw}",
        hub.public_origin.trim_end_matches('/')
    );
    hub.mail.send_verify_email(&email, &url).await?;
    Ok(Json(json!({ "ok": true })))
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
    Ok(Json(json!({ "id": user.id, "email": user.email })))
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
