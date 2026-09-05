//! cellz event push/pull. Identity is client event id, not local sequence.

use axum::Json;
use axum::extract::{Path, Query, State};
use axum::http::HeaderMap;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use crate::auth;
use crate::error::{Error, Result};
use crate::server::HubState;
use crate::store;

#[derive(Debug, Deserialize)]
pub struct EventsQuery {
    pub since: Option<i64>,
    pub limit: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SyncEvent {
    pub id: String,
    pub event_type: String,
    pub payload: Value,
    pub created_at: String,
}

#[derive(Debug, Deserialize)]
pub struct PushEventsRequest {
    pub events: Vec<SyncEvent>,
}

pub async fn pull(
    State(hub): State<HubState>,
    Path(project_id): Path<String>,
    Query(query): Query<EventsQuery>,
    headers: HeaderMap,
) -> Result<Json<Value>> {
    require_owner(&hub, &headers, &project_id).await?;
    let handle = hub
        .gitcell
        .cell_manager
        .get_or_activate(&project_id)
        .await?;
    let limit = query.limit.unwrap_or(200).clamp(1, 1000);
    let records = handle.get_events(query.since, Some(limit)).await?;
    let events: Vec<SyncEvent> = records
        .into_iter()
        .map(|rec| {
            let id = rec
                .payload
                .get("id")
                .and_then(|v| v.as_str())
                .unwrap_or(&rec.id)
                .to_string();
            SyncEvent {
                id,
                event_type: rec.event_type,
                payload: rec.payload,
                created_at: rec.created_at.to_rfc3339(),
            }
        })
        .collect();
    Ok(Json(json!({ "events": events })))
}

pub async fn push(
    State(hub): State<HubState>,
    Path(project_id): Path<String>,
    headers: HeaderMap,
    Json(body): Json<PushEventsRequest>,
) -> Result<Json<Value>> {
    require_owner(&hub, &headers, &project_id).await?;
    let handle = hub
        .gitcell
        .cell_manager
        .get_or_activate(&project_id)
        .await?;
    let existing = handle.get_events(None, Some(1000)).await?;
    let mut known = std::collections::HashSet::new();
    for rec in &existing {
        known.insert(rec.id.clone());
        if let Some(id) = rec.payload.get("id").and_then(|v| v.as_str()) {
            known.insert(id.to_string());
        }
    }
    let mut accepted = 0u32;
    let mut skipped = 0u32;
    for ev in body.events {
        if ev.id.is_empty() || known.contains(&ev.id) {
            skipped += 1;
            continue;
        }
        let mut payload = ev.payload;
        if !payload.is_object() {
            payload = json!({ "value": payload });
        }
        if let Some(obj) = payload.as_object_mut() {
            obj.insert("id".into(), json!(ev.id));
        }
        handle
            .append_event(None, ev.event_type, payload)
            .await
            .map_err(|e| Error::Internal(anyhow::anyhow!(e.to_string())))?;
        known.insert(ev.id);
        accepted += 1;
    }
    Ok(Json(json!({ "accepted": accepted, "skipped": skipped })))
}

async fn require_owner(hub: &HubState, headers: &HeaderMap, project_id: &str) -> Result<()> {
    let user = auth::user_from_headers(hub, headers).await?;
    if !store::project_owned(&hub.db, project_id, &user.id).await? {
        return Err(Error::NotFound("project not found".into()));
    }
    Ok(())
}
