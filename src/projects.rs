use axum::Json;
use axum::extract::State;
use axum::http::HeaderMap;
use serde::Deserialize;
use serde_json::{Value, json};

use crate::auth;
use crate::error::{Error, Result};
use crate::server::HubState;
use crate::store;

#[derive(Debug, Deserialize)]
pub struct CreateProjectRequest {
    pub name: String,
    pub slug: Option<String>,
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
    let slug = match body.slug.as_deref() {
        Some(s) => normalize_slug(s)?,
        None => slug_from_name(name)?,
    };
    let project = store::insert_project(&hub.db, &user.id, &slug, name).await?;
    let repo_path = hub.gitcell.data_dir.join(&project.id);
    gitcell::git_ops::init(&repo_path)
        .map_err(|e| Error::Internal(anyhow::anyhow!(e.to_string())))?;
    Ok(Json(json!({
        "id": project.id,
        "slug": project.slug,
        "name": project.name,
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
    let project = store::get_project(&hub.db, &id)
        .await?
        .ok_or_else(|| Error::NotFound("project not found".into()))?;
    if project.owner_id != user.id {
        return Err(Error::NotFound("project not found".into()));
    }
    Ok(Json(json!({
        "id": project.id,
        "slug": project.slug,
        "name": project.name,
        "created_at": project.created_at,
    })))
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
    normalize_slug(&slug)
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
