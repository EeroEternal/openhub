//! Git bundle upload/download. Real git objects; not a new VCS.

use std::path::Path;
use std::process::Command;

use axum::Json;
use axum::body::Bytes;
use axum::extract::{Path as AxumPath, State};
use axum::http::{HeaderMap, StatusCode, header};
use axum::response::{IntoResponse, Response};
use serde_json::{Value, json};

use crate::auth;
use crate::error::{Error, Result};
use crate::server::HubState;
use crate::store;

pub async fn download(
    State(hub): State<HubState>,
    AxumPath(project_id): AxumPath<String>,
    headers: HeaderMap,
) -> Result<Response> {
    require_owner(&hub, &headers, &project_id).await?;
    let repo = hub.gitcell.data_dir.join(&project_id);
    if !repo.join(".git").exists() && !repo.join("HEAD").exists() {
        return Err(Error::NotFound("git repo not found".into()));
    }
    let bytes = create_bundle(&repo)?;
    Ok((
        StatusCode::OK,
        [(header::CONTENT_TYPE, "application/vnd.git-bundle")],
        bytes,
    )
        .into_response())
}

pub async fn upload(
    State(hub): State<HubState>,
    AxumPath(project_id): AxumPath<String>,
    headers: HeaderMap,
    body: Bytes,
) -> Result<Json<Value>> {
    require_owner(&hub, &headers, &project_id).await?;
    if body.is_empty() {
        return Err(Error::InvalidRequest("empty bundle".into()));
    }
    let repo = hub.gitcell.data_dir.join(&project_id);
    fetch_bundle(&repo, &body)?;
    Ok(Json(json!({ "ok": true })))
}

async fn require_owner(hub: &HubState, headers: &HeaderMap, project_id: &str) -> Result<()> {
    let user = auth::user_from_headers(hub, headers).await?;
    if !store::project_owned(&hub.db, project_id, &user.id).await? {
        return Err(Error::NotFound("project not found".into()));
    }
    Ok(())
}

pub fn create_bundle(repo: &Path) -> Result<Vec<u8>> {
    let bundle = repo.join(".git").join("openhub.bundle");
    let output = Command::new("git")
        .args([
            "bundle",
            "create",
            bundle.to_str().unwrap_or("openhub.bundle"),
            "--all",
        ])
        .current_dir(repo)
        .output()
        .map_err(|e| Error::Internal(anyhow::anyhow!("git bundle: {e}")))?;
    if !output.status.success() {
        let err = String::from_utf8_lossy(&output.stderr);
        return Err(Error::InvalidRequest(format!("git bundle failed: {err}")));
    }
    let bytes =
        std::fs::read(&bundle).map_err(|e| Error::Internal(anyhow::anyhow!("read bundle: {e}")))?;
    let _ = std::fs::remove_file(&bundle);
    Ok(bytes)
}

pub fn fetch_bundle(repo: &Path, bytes: &[u8]) -> Result<()> {
    let bundle = repo.join(".git").join("openhub-in.bundle");
    if let Some(parent) = bundle.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|e| Error::Internal(anyhow::anyhow!(e.to_string())))?;
    }
    std::fs::write(&bundle, bytes)
        .map_err(|e| Error::Internal(anyhow::anyhow!("write bundle: {e}")))?;
    let output = Command::new("git")
        .args([
            "fetch",
            bundle.to_str().unwrap_or("openhub-in.bundle"),
            "+refs/heads/*:refs/heads/*",
        ])
        .current_dir(repo)
        .output()
        .map_err(|e| Error::Internal(anyhow::anyhow!("git fetch bundle: {e}")))?;
    let _ = std::fs::remove_file(&bundle);
    if !output.status.success() {
        let err = String::from_utf8_lossy(&output.stderr);
        return Err(Error::InvalidRequest(format!(
            "git fetch bundle failed: {err}"
        )));
    }
    Ok(())
}
