//! Browser login for the `oh` CLI: start → user signs in on the site → poll token.

use axum::Json;
use axum::extract::State;
use axum::http::HeaderMap;
use serde::Deserialize;
use serde_json::{Value, json};

use crate::auth::{hash_token, issue_session, user_from_headers};
use crate::error::{Error, Result};
use crate::server::HubState;
use crate::store;

const TTL_SECS: i64 = 600;

#[derive(Debug, Deserialize)]
pub struct PollBody {
    pub device_code: String,
}

#[derive(Debug, Deserialize)]
pub struct ApproveBody {
    pub code: String,
}

pub async fn start(State(hub): State<HubState>) -> Result<Json<Value>> {
    let device_code = crate::auth::random_token();
    let user_code = user_code();
    store::insert_cli_login(&hub.db, &hash_token(&device_code), &user_code, TTL_SECS).await?;
    let origin = hub.public_origin.trim_end_matches('/');
    Ok(Json(json!({
        "device_code": device_code,
        "user_code": user_code,
        "verification_uri": format!("{origin}/cli?code={user_code}"),
        "expires_in": TTL_SECS,
        "interval": 1
    })))
}

pub async fn poll(State(hub): State<HubState>, Json(body): Json<PollBody>) -> Result<Json<Value>> {
    let device_code = body.device_code.trim();
    if device_code.is_empty() {
        return Err(Error::InvalidRequest("device_code required".into()));
    }
    match store::poll_cli_login(&hub.db, &hash_token(device_code)).await? {
        store::CliPoll::Pending => Ok(Json(json!({ "status": "pending" }))),
        store::CliPoll::Expired => Err(Error::InvalidRequest(
            "login expired; run oh login again".into(),
        )),
        store::CliPoll::Unknown => Err(Error::NotFound("unknown device_code".into())),
        store::CliPoll::Granted(token) => Ok(Json(json!({ "status": "ok", "token": token }))),
    }
}

pub async fn approve(
    State(hub): State<HubState>,
    headers: HeaderMap,
    Json(body): Json<ApproveBody>,
) -> Result<Json<Value>> {
    let user = user_from_headers(&hub, &headers).await?;
    let code = body.code.trim().to_uppercase().replace('-', "");
    if code.len() < 6 {
        return Err(Error::InvalidRequest("code required".into()));
    }
    let session = issue_session(&hub, &user.id).await?;
    let n = store::approve_cli_login(&hub.db, &code, &user.id, &session).await?;
    if n == 0 {
        return Err(Error::NotFound("unknown or expired code".into()));
    }
    Ok(Json(json!({ "ok": true })))
}

fn user_code() -> String {
    const ALPH: &[u8] = b"ABCDEFGHJKLMNPQRSTUVWXYZ23456789";
    let bytes: [u8; 8] = rand::random();
    bytes
        .iter()
        .map(|b| ALPH[*b as usize % ALPH.len()] as char)
        .collect()
}
