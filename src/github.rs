//! GitHub OAuth for the signed-in OpenHub user (account Settings).

use axum::Json;
use axum::extract::{Query, State};
use axum::http::HeaderMap;
use axum::response::{Html, IntoResponse, Response};
use serde::Deserialize;
use serde_json::{Value, json};

use crate::auth;
use crate::error::{Error, Result};
use crate::server::HubState;
use crate::store;

#[derive(Debug, Deserialize)]
pub struct GithubCallbackQuery {
    pub code: Option<String>,
    pub state: Option<String>,
    pub error: Option<String>,
}

pub async fn start(State(hub): State<HubState>, headers: HeaderMap) -> Result<Json<Value>> {
    let user = auth::user_from_headers(&hub, &headers).await?;
    if hub.github_client_id.is_empty() || hub.github_client_secret.is_empty() {
        return Err(Error::InvalidRequest(
            "GitHub OAuth is not configured".into(),
        ));
    }
    let state = auth::random_token();
    store::insert_token_minutes(
        &hub.db,
        &user.id,
        "github_oauth",
        &auth::hash_token(&state),
        10,
    )
    .await?;
    let redirect = format!(
        "{}/api/v1/auth/github/callback",
        hub.public_origin.trim_end_matches('/')
    );
    let mut url = reqwest::Url::parse("https://github.com/login/oauth/authorize")
        .map_err(|e| Error::Internal(anyhow::anyhow!(e.to_string())))?;
    url.query_pairs_mut()
        .append_pair("client_id", &hub.github_client_id)
        .append_pair("redirect_uri", &redirect)
        .append_pair("scope", "repo")
        .append_pair("state", &state)
        .append_pair("allow_signup", "true");
    Ok(Json(json!({ "url": url.to_string() })))
}

pub async fn callback(
    State(hub): State<HubState>,
    Query(query): Query<GithubCallbackQuery>,
) -> Response {
    let origin = hub.public_origin.trim_end_matches('/').to_string();
    if query.error.is_some() {
        return oauth_done_page(&origin, false).into_response();
    }
    let (Some(code), Some(state)) = (query.code, query.state) else {
        return oauth_done_page(&origin, false).into_response();
    };
    match finish_oauth(&hub, &code, &state).await {
        Ok(()) => oauth_done_page(&origin, true).into_response(),
        Err(err) => {
            tracing::warn!(%err, "github oauth callback failed");
            oauth_done_page(&origin, false).into_response()
        }
    }
}

fn oauth_done_page(origin: &str, ok: bool) -> Html<String> {
    let ok_js = if ok { "true" } else { "false" };
    let origin_js = serde_json::to_string(origin).unwrap_or_else(|_| "\"\"".to_string());
    let status = if ok { "complete" } else { "failed" };
    Html(format!(
        r#"<!DOCTYPE html><html><head><meta charset="utf-8"><title>GitHub</title></head><body>
<script>
(function () {{
  var ok = {ok_js};
  var origin = {origin_js};
  var flag = ok ? "connected" : "error";
  try {{
    if (window.opener && !window.opener.closed) {{
      window.opener.postMessage({{ source: "openhub-github", ok: ok }}, origin);
      window.close();
      return;
    }}
  }} catch (e) {{}}
  location.replace(origin + "/settings?section=github&github=" + flag);
}})();
</script>
<p>GitHub authorization {status}. You can close this window.</p>
</body></html>"#
    ))
}

pub async fn disconnect(State(hub): State<HubState>, headers: HeaderMap) -> Result<Json<Value>> {
    let user = auth::user_from_headers(&hub, &headers).await?;
    store::clear_user_github(&hub.db, &user.id).await?;
    Ok(Json(json!({ "ok": true, "github_connected": false })))
}

async fn finish_oauth(hub: &HubState, code: &str, state: &str) -> Result<()> {
    let user_id = store::take_token(&hub.db, "github_oauth", &auth::hash_token(state))
        .await?
        .ok_or_else(|| Error::Unauthorized("invalid GitHub OAuth state".into()))?;
    let client = reqwest::Client::new();
    let token_res = client
        .post("https://github.com/login/oauth/access_token")
        .header(reqwest::header::ACCEPT, "application/json")
        .header(reqwest::header::USER_AGENT, "openhub")
        .json(&json!({
            "client_id": hub.github_client_id,
            "client_secret": hub.github_client_secret,
            "code": code,
        }))
        .send()
        .await
        .map_err(|e| Error::Internal(anyhow::anyhow!("github token: {e}")))?;
    let token_body: Value = token_res
        .json()
        .await
        .map_err(|e| Error::Internal(anyhow::anyhow!("github token json: {e}")))?;
    let access = token_body["access_token"]
        .as_str()
        .filter(|s| !s.is_empty())
        .ok_or_else(|| {
            Error::InvalidRequest(
                token_body["error_description"]
                    .as_str()
                    .unwrap_or("GitHub did not return an access token")
                    .to_string(),
            )
        })?;
    let user_res = client
        .get("https://api.github.com/user")
        .header(reqwest::header::AUTHORIZATION, format!("Bearer {access}"))
        .header(reqwest::header::ACCEPT, "application/vnd.github+json")
        .header(reqwest::header::USER_AGENT, "openhub")
        .send()
        .await
        .map_err(|e| Error::Internal(anyhow::anyhow!("github user: {e}")))?;
    let gh: Value = user_res
        .json()
        .await
        .map_err(|e| Error::Internal(anyhow::anyhow!("github user json: {e}")))?;
    let login = gh["login"]
        .as_str()
        .filter(|s| !s.is_empty())
        .ok_or_else(|| Error::InvalidRequest("GitHub user has no login".into()))?;
    let github_id = gh["id"]
        .as_i64()
        .map(|n| n.to_string())
        .or_else(|| gh["id"].as_str().map(str::to_string))
        .ok_or_else(|| Error::InvalidRequest("GitHub user has no id".into()))?;
    store::set_user_github(&hub.db, &user_id, &github_id, login, access).await?;
    Ok(())
}
