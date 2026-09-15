//! Git Smart HTTP via `git http-backend` (CGI). Repos stay normal git dirs.

use std::path::Path;
use std::process::{Command, Stdio};

use axum::body::Bytes;
use axum::extract::{Path as AxumPath, Query, State};
use axum::http::{HeaderMap, HeaderName, HeaderValue, Method, StatusCode, header};
use axum::response::Response;
use serde::Deserialize;

use crate::auth;
use crate::error::{Error, Result};
use crate::server::HubState;
use crate::store;

/// axum's default body limit is 2 MiB, which rejects ordinary `git push` packs.
pub const MAX_PACK_BODY: usize = 512 * 1024 * 1024;

#[derive(Debug, Deserialize)]
pub struct InfoRefsQuery {
    pub service: Option<String>,
}

pub async fn info_refs(
    State(hub): State<HubState>,
    AxumPath(project_id): AxumPath<String>,
    Query(query): Query<InfoRefsQuery>,
    headers: HeaderMap,
) -> Result<Response> {
    let project = authorize(&hub, &headers, &project_id).await?;
    let repo = hub.gitcell.data_dir.join(&project.id);
    ensure_http_enabled(&repo)?;
    let qs = query
        .service
        .as_deref()
        .map(|s| format!("service={s}"))
        .unwrap_or_default();
    cgi(
        &hub.gitcell.data_dir,
        &project.id,
        Method::GET,
        "/info/refs",
        &qs,
        &headers,
        &[],
    )
}

pub async fn upload_pack(
    State(hub): State<HubState>,
    AxumPath(project_id): AxumPath<String>,
    headers: HeaderMap,
    body: Bytes,
) -> Result<Response> {
    let project = authorize(&hub, &headers, &project_id).await?;
    let repo = hub.gitcell.data_dir.join(&project.id);
    ensure_http_enabled(&repo)?;
    cgi(
        &hub.gitcell.data_dir,
        &project.id,
        Method::POST,
        "/git-upload-pack",
        "",
        &headers,
        &body,
    )
}

pub async fn receive_pack(
    State(hub): State<HubState>,
    AxumPath(project_id): AxumPath<String>,
    headers: HeaderMap,
    body: Bytes,
) -> Result<Response> {
    let project = authorize(&hub, &headers, &project_id).await?;
    let repo = hub.gitcell.data_dir.join(&project.id);
    ensure_http_enabled(&repo)?;
    cgi(
        &hub.gitcell.data_dir,
        &project.id,
        Method::POST,
        "/git-receive-pack",
        "",
        &headers,
        &body,
    )
}

pub async fn info_refs_user_repo(
    State(hub): State<HubState>,
    AxumPath((username, repo_name)): AxumPath<(String, String)>,
    Query(query): Query<InfoRefsQuery>,
    headers: HeaderMap,
) -> Result<Response> {
    let clean_repo = repo_name.strip_suffix(".git").unwrap_or(&repo_name);
    let project = authorize_user_repo(&hub, &headers, &username, clean_repo).await?;
    let repo = hub.gitcell.data_dir.join(&project.id);
    ensure_http_enabled(&repo)?;
    let qs = query
        .service
        .as_deref()
        .map(|s| format!("service={s}"))
        .unwrap_or_default();
    cgi(
        &hub.gitcell.data_dir,
        &project.id,
        Method::GET,
        "/info/refs",
        &qs,
        &headers,
        &[],
    )
}

pub async fn upload_pack_user_repo(
    State(hub): State<HubState>,
    AxumPath((username, repo_name)): AxumPath<(String, String)>,
    headers: HeaderMap,
    body: Bytes,
) -> Result<Response> {
    let clean_repo = repo_name.strip_suffix(".git").unwrap_or(&repo_name);
    let project = authorize_user_repo(&hub, &headers, &username, clean_repo).await?;
    let repo = hub.gitcell.data_dir.join(&project.id);
    ensure_http_enabled(&repo)?;
    cgi(
        &hub.gitcell.data_dir,
        &project.id,
        Method::POST,
        "/git-upload-pack",
        "",
        &headers,
        &body,
    )
}

pub async fn receive_pack_user_repo(
    State(hub): State<HubState>,
    AxumPath((username, repo_name)): AxumPath<(String, String)>,
    headers: HeaderMap,
    body: Bytes,
) -> Result<Response> {
    let clean_repo = repo_name.strip_suffix(".git").unwrap_or(&repo_name);
    let project = authorize_user_repo(&hub, &headers, &username, clean_repo).await?;
    let repo = hub.gitcell.data_dir.join(&project.id);
    ensure_http_enabled(&repo)?;
    cgi(
        &hub.gitcell.data_dir,
        &project.id,
        Method::POST,
        "/git-receive-pack",
        "",
        &headers,
        &body,
    )
}

async fn authorize_user_repo(
    hub: &HubState,
    headers: &HeaderMap,
    username: &str,
    repo_name: &str,
) -> Result<store::Project> {
    let raw = token_from_headers(headers)
        .ok_or_else(|| Error::Unauthorized("missing git auth".into()))?;
    let user = store::find_session_user(&hub.db, &auth::hash_token(&raw))
        .await?
        .ok_or_else(|| Error::Unauthorized("invalid session".into()))?;
    let project = store::find_project_by_username_and_slug(&hub.db, username, repo_name)
        .await?
        .ok_or_else(|| Error::NotFound("project not found".into()))?;
    if project.owner_id != user.id {
        return Err(Error::NotFound("project not found".into()));
    }
    Ok(project)
}

async fn authorize(
    hub: &HubState,
    headers: &HeaderMap,
    identifier: &str,
) -> Result<store::Project> {
    let raw = token_from_headers(headers)
        .ok_or_else(|| Error::Unauthorized("missing git auth".into()))?;
    let user = store::find_session_user(&hub.db, &auth::hash_token(&raw))
        .await?
        .ok_or_else(|| Error::Unauthorized("invalid session".into()))?;
    let project = store::find_project_by_id_or_slug(&hub.db, &user.id, identifier)
        .await?
        .ok_or_else(|| Error::NotFound("project not found".into()))?;
    Ok(project)
}

fn token_from_headers(headers: &HeaderMap) -> Option<String> {
    if let Some(b) = auth::bearer_from_headers(headers) {
        return Some(b);
    }
    let value = headers.get(header::AUTHORIZATION)?.to_str().ok()?;
    let b64 = value.strip_prefix("Basic ")?;
    let decoded = decode_base64(b64)?;
    let pair = String::from_utf8(decoded).ok()?;
    pair.split_once(':').map(|(_, pass)| pass.to_string())
}

fn decode_base64(input: &str) -> Option<Vec<u8>> {
    use base64::Engine;
    base64::engine::general_purpose::STANDARD
        .decode(input.trim())
        .ok()
}

pub fn ensure_http_enabled(repo: &Path) -> Result<()> {
    for (key, val) in [
        ("http.receivepack", "true"),
        ("http.uploadpack", "true"),
        ("receive.denyCurrentBranch", "updateInstead"),
        ("uploadpack.allowFilter", "true"),
        ("uploadpackfilter.allow", "true"),
    ] {
        let st = git_command()
            .args(["config", key, val])
            .current_dir(repo)
            .status()
            .map_err(|e| Error::Internal(anyhow::anyhow!("git config: {e}")))?;
        if !st.success() {
            return Err(Error::Internal(anyhow::anyhow!("git config {key} failed")));
        }
    }
    Ok(())
}

fn git_command() -> Command {
    let mut cmd = Command::new("git");
    // Inherited GIT_* from the server process (e.g. running out of this repo)
    // would make http-backend operate on the wrong tree.
    cmd.env_remove("GIT_DIR")
        .env_remove("GIT_WORK_TREE")
        .env_remove("GIT_COMMON_DIR")
        .env_remove("GIT_OBJECT_DIRECTORY")
        .env_remove("GIT_INDEX_FILE")
        .env_remove("GIT_PROTOCOL");
    cmd
}

fn cgi(
    root: &Path,
    project_id: &str,
    method: Method,
    suffix: &str,
    query: &str,
    headers: &HeaderMap,
    body: &[u8],
) -> Result<Response> {
    let path_info = format!("/{project_id}{suffix}");
    let content_type = headers
        .get(header::CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .map(str::to_owned)
        .unwrap_or_else(|| {
            if suffix.contains("receive-pack") {
                "application/x-git-receive-pack-request".into()
            } else if suffix.contains("upload-pack") {
                "application/x-git-upload-pack-request".into()
            } else {
                String::new()
            }
        });
    let mut cmd = git_command();
    cmd.arg("http-backend")
        .current_dir(root)
        .env("GIT_HTTP_EXPORT_ALL", "1")
        .env("GIT_PROJECT_ROOT", root)
        .env("PATH_INFO", &path_info)
        .env("QUERY_STRING", query)
        .env("REQUEST_METHOD", method.as_str())
        .env("CONTENT_TYPE", &content_type)
        .env("CONTENT_LENGTH", body.len().to_string())
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    if let Some(proto) = headers
        .get("Git-Protocol")
        .and_then(|v| v.to_str().ok())
        .filter(|s| !s.is_empty())
    {
        cmd.env("GIT_PROTOCOL", proto);
        cmd.env("HTTP_GIT_PROTOCOL", proto);
    }
    if let Some(enc) = headers
        .get(header::CONTENT_ENCODING)
        .and_then(|v| v.to_str().ok())
        .filter(|s| !s.is_empty())
    {
        cmd.env("HTTP_CONTENT_ENCODING", enc);
    }
    let mut child = cmd
        .spawn()
        .map_err(|e| Error::Internal(anyhow::anyhow!("git http-backend: {e}")))?;

    if let Some(mut stdin) = child.stdin.take() {
        use std::io::Write;
        stdin
            .write_all(body)
            .map_err(|e| Error::Internal(anyhow::anyhow!("git http-backend stdin: {e}")))?;
    }

    let output = child
        .wait_with_output()
        .map_err(|e| Error::Internal(anyhow::anyhow!("git http-backend wait: {e}")))?;
    if !output.status.success() {
        let err = String::from_utf8_lossy(&output.stderr);
        tracing::warn!(%err, "git http-backend failed");
        let detail = err.trim();
        if detail.is_empty() {
            return Err(Error::Internal(anyhow::anyhow!("git http-backend failed")));
        }
        return Err(Error::Internal(anyhow::anyhow!(
            "git http-backend failed: {detail}"
        )));
    }
    parse_cgi(&output.stdout)
}

#[allow(clippy::collapsible_if)]
fn parse_cgi(stdout: &[u8]) -> Result<Response> {
    let sep = stdout
        .windows(4)
        .position(|w| w == b"\r\n\r\n")
        .map(|i| i + 4)
        .or_else(|| stdout.windows(2).position(|w| w == b"\n\n").map(|i| i + 2))
        .unwrap_or(stdout.len());
    let header_bytes = &stdout[..sep.min(stdout.len())];
    let body = if sep < stdout.len() {
        stdout[sep..].to_vec()
    } else {
        Vec::new()
    };
    let header_text = String::from_utf8_lossy(header_bytes);
    let mut status = StatusCode::OK;
    let mut builder = Response::builder();
    for line in header_text.lines() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        if let Some(rest) = line.strip_prefix("Status:") {
            let code = rest.split_whitespace().next().unwrap_or("200");
            status = StatusCode::from_u16(code.parse().unwrap_or(200)).unwrap_or(StatusCode::OK);
            continue;
        }
        if let Some((name, value)) = line.split_once(':') {
            if let (Ok(n), Ok(v)) = (
                HeaderName::from_bytes(name.trim().as_bytes()),
                HeaderValue::from_str(value.trim()),
            ) {
                builder = builder.header(n, v);
            }
        }
    }
    builder
        .status(status)
        .body(axum::body::Body::from(body))
        .map_err(|e| Error::Internal(anyhow::anyhow!(e.to_string())))
}
