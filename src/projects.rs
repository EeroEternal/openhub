use axum::Json;
use axum::extract::{Query, State};
use axum::http::HeaderMap;
use serde::Deserialize;
use serde_json::{Value, json};

use crate::auth;
use crate::error::{Error, Result};
use crate::server::HubState;
use crate::store;

#[derive(Debug, Deserialize)]
pub struct CheckSlugQuery {
    pub slug: String,
}

pub async fn check_slug(
    State(hub): State<HubState>,
    headers: HeaderMap,
    Query(query): Query<CheckSlugQuery>,
) -> Result<Json<Value>> {
    let user = auth::user_from_headers(&hub, &headers).await?;
    let raw = query.slug.trim();
    if raw.is_empty() {
        return Ok(Json(json!({
            "available": false,
            "reason": "slug cannot be empty"
        })));
    }
    let normalized = match normalize_slug(raw) {
        Ok(s) => s,
        Err(e) => {
            return Ok(Json(json!({
                "available": false,
                "reason": e.to_string()
            })));
        }
    };
    let existing = store::find_project_by_id_or_slug(&hub.db, &user.id, &normalized).await?;
    if existing.is_some() {
        Ok(Json(json!({
            "available": false,
            "reason": "slug is already taken",
            "slug": normalized
        })))
    } else {
        Ok(Json(json!({
            "available": true,
            "slug": normalized
        })))
    }
}

#[derive(Debug, Deserialize)]
pub struct CreateProjectRequest {
    pub name: String,
    pub slug: Option<String>,
    pub description: Option<String>,
    pub init_readme: Option<bool>,
}

pub async fn create(
    State(hub): State<HubState>,
    headers: HeaderMap,
    Json(body): Json<CreateProjectRequest>,
) -> Result<Json<Value>> {
    let user = auth::user_from_headers(&hub, &headers).await?;
    let name = body.name.trim();
    if name.is_empty() {
        return Err(Error::InvalidRequest("name is required".into()));
    }
    let slug = match body
        .slug
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
    {
        Some(s) => normalize_slug(s)?,
        None => slug_from_name(name)?,
    };
    let project = store::insert_project(&hub.db, &user.id, &slug, name).await?;
    let repo_path = hub.gitcell.data_dir.join(&project.id);
    gitcell::git_ops::init(&repo_path)
        .map_err(|e| Error::Internal(anyhow::anyhow!(e.to_string())))?;
    crate::git_http::ensure_http_enabled(&repo_path)?;

    if body.init_readme.unwrap_or(true) {
        let desc = body
            .description
            .as_deref()
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .unwrap_or("A collaborative project on OpenHub.");
        let readme_content = format!(
            "# {}\n\n{}\n\n---\n*Created with [OpenHub](https://openhub.run) - Open Agent & Git Platform*\n",
            name, desc
        );
        let readme_path = repo_path.join("README.md");
        let _ = std::fs::write(&readme_path, readme_content);
        let _ = gitcell::git_ops::add(&repo_path, &["README.md".to_string()]);
        let _ = gitcell::git_ops::commit(&repo_path, "Initial commit");
    }

    Ok(Json(json!({
        "id": project.id,
        "slug": project.slug,
        "name": project.name,
        "owner_username": user.username,
        "full_name": format!("{}/{}", user.username, project.slug),
        "created_at": project.created_at,
    })))
}

pub async fn list(State(hub): State<HubState>, headers: HeaderMap) -> Result<Json<Value>> {
    let user = auth::user_from_headers(&hub, &headers).await?;
    let projects = store::list_projects(&hub.db, &user.id).await?;
    let items: Vec<Value> = projects
        .into_iter()
        .map(|p| {
            json!({
                "id": p.id,
                "slug": p.slug,
                "name": p.name,
                "owner_username": user.username,
                "full_name": format!("{}/{}", user.username, p.slug),
                "created_at": p.created_at,
            })
        })
        .collect();
    Ok(Json(json!({ "projects": items })))
}

pub async fn get(
    State(hub): State<HubState>,
    axum::extract::Path(id): axum::extract::Path<String>,
    headers: HeaderMap,
) -> Result<Json<Value>> {
    let user = auth::user_from_headers(&hub, &headers).await?;
    let project = store::find_project_by_id_or_slug(&hub.db, &user.id, &id)
        .await?
        .ok_or_else(|| Error::NotFound("project not found".into()))?;

    Ok(Json(json!({
        "id": project.id,
        "slug": project.slug,
        "name": project.name,
        "owner_username": user.username,
        "full_name": format!("{}/{}", user.username, project.slug),
        "created_at": project.created_at,
    })))
}

pub async fn get_user_repo(
    State(hub): State<HubState>,
    axum::extract::Path((username, slug)): axum::extract::Path<(String, String)>,
    headers: HeaderMap,
) -> Result<Json<Value>> {
    let user = auth::user_from_headers(&hub, &headers).await?;
    let project = store::find_project_by_username_and_slug(&hub.db, &username, &slug)
        .await?
        .ok_or_else(|| Error::NotFound("project not found".into()))?;

    Ok(Json(json!({
        "id": project.id,
        "slug": project.slug,
        "name": project.name,
        "owner_username": user.username,
        "full_name": format!("{}/{}", user.username, project.slug),
        "created_at": project.created_at,
    })))
}

pub async fn delete(
    State(hub): State<HubState>,
    axum::extract::Path(id): axum::extract::Path<String>,
    headers: HeaderMap,
) -> Result<Json<Value>> {
    let user = auth::user_from_headers(&hub, &headers).await?;
    let project = store::delete_project(&hub.db, &user.id, &id)
        .await?
        .ok_or_else(|| Error::NotFound("project not found".into()))?;

    let repo = hub.gitcell.data_dir.join(&project.id);
    if repo.exists()
        && let Err(err) = std::fs::remove_dir_all(&repo)
    {
        tracing::warn!(path = %repo.display(), error = %err, "failed to remove project git tree");
    }
    for name in [
        format!("{}.db", project.id),
        format!("{}.db-wal", project.id),
        format!("{}.db-shm", project.id),
    ] {
        let path = hub.cells_dir.join(name);
        if path.exists() {
            let _ = std::fs::remove_file(&path);
        }
    }
    let lease = hub
        .cells_storage_dir
        .join("leases")
        .join(format!("{}.lease", project.id));
    if lease.exists() {
        let _ = std::fs::remove_file(&lease);
    }

    Ok(Json(json!({ "ok": true, "id": project.id })))
}

fn slug_from_name(name: &str) -> Result<String> {
    let slug: String = name
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() {
                c.to_ascii_lowercase()
            } else {
                '-'
            }
        })
        .collect();
    let cleaned = slug.trim_matches('-');
    if cleaned.is_empty() {
        return Ok(format!("project-{}", &store::new_id()[..6]));
    }
    normalize_slug(cleaned)
}

fn normalize_slug(slug: &str) -> Result<String> {
    let slug = slug.trim().to_ascii_lowercase();
    let slug: String = slug
        .chars()
        .filter(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || *c == '-')
        .collect();
    let slug = slug.trim_matches('-').to_string();
    if slug.is_empty() || slug.len() > 40 {
        return Err(Error::InvalidRequest(
            "slug must be 1-40 of [a-z0-9-]".into(),
        ));
    }
    Ok(slug)
}
