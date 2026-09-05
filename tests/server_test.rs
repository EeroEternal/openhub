use axum::{
    body::Body,
    http::{Request, StatusCode},
};
use gitcell::server::AppState as GitcellState;
use http_body_util::BodyExt;
use openhub::mail::Mailer;
use openhub::server::{HubState, create_router};
use openhub::store;
use tower::ServiceExt;

async fn test_hub() -> (HubState, tempfile::TempDir) {
    let tmp = tempfile::tempdir().unwrap();
    let gitcell = GitcellState::new(
        tmp.path().join("repos"),
        tmp.path().join("cells"),
        tmp.path().join("cells-storage"),
        60,
    )
    .unwrap();
    let db_path = tmp.path().join("openhub.db");
    let db = store::connect(&db_path).await.unwrap();
    store::migrate(&db).await.unwrap();
    let hub = HubState {
        gitcell,
        db,
        mail: Mailer::log(),
        public_origin: "http://127.0.0.1:8080".into(),
    };
    (hub, tmp)
}

async fn json_request(
    app: axum::Router,
    method: &str,
    uri: &str,
    token: Option<&str>,
    body: Option<serde_json::Value>,
) -> (StatusCode, serde_json::Value) {
    let mut builder = Request::builder().method(method).uri(uri);
    if let Some(t) = token {
        builder = builder.header("authorization", format!("Bearer {t}"));
    }
    let body = if let Some(payload) = body {
        builder = builder.header("content-type", "application/json");
        Body::from(payload.to_string())
    } else {
        Body::empty()
    };
    let response = app.oneshot(builder.body(body).unwrap()).await.unwrap();
    let status = response.status();
    let bytes = response.into_body().collect().await.unwrap().to_bytes();
    let json = if bytes.is_empty() {
        serde_json::Value::Null
    } else {
        serde_json::from_slice(&bytes).unwrap_or(serde_json::Value::Null)
    };
    (status, json)
}

#[tokio::test]
async fn test_health_check() {
    let (hub, _tmp) = test_hub().await;
    let app = create_router(hub);
    let (status, json) = json_request(app, "GET", "/health", None, None).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(json["service"], "openhub");
}

#[tokio::test]
async fn repos_require_auth() {
    let (hub, _tmp) = test_hub().await;
    let app = create_router(hub);
    let (status, _) = json_request(app, "POST", "/api/v1/repos/demo/init", None, None).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn register_set_password_create_project_and_status() {
    let (hub, _tmp) = test_hub().await;
    let mail = hub.mail.clone();
    let app = create_router(hub);

    let (status, _) = json_request(
        app.clone(),
        "POST",
        "/api/v1/auth/register",
        None,
        Some(serde_json::json!({"email": "Ada@Example.com"})),
    )
    .await;
    assert_eq!(status, StatusCode::OK);

    let url = mail.last_verify_url.lock().unwrap().clone().unwrap();
    let token = url.split("token=").nth(1).unwrap().to_string();

    let (status, body) = json_request(
        app.clone(),
        "POST",
        "/api/v1/auth/password",
        None,
        Some(serde_json::json!({"token": token, "password": "password1"})),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let session = body["token"].as_str().unwrap().to_string();

    let (status, me) = json_request(app.clone(), "GET", "/api/v1/me", Some(&session), None).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(me["email"], "ada@example.com");

    let (status, project) = json_request(
        app.clone(),
        "POST",
        "/api/v1/projects",
        Some(&session),
        Some(serde_json::json!({"name": "Demo App"})),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{project}");
    let id = project["id"].as_str().unwrap();
    assert_eq!(project["slug"], "demo-app");

    let (status, _) = json_request(
        app.clone(),
        "GET",
        &format!("/api/v1/repos/{id}/status"),
        Some(&session),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK);

    let (status, _) = json_request(
        app.clone(),
        "GET",
        &format!("/api/v1/repos/{id}/status"),
        None,
        None,
    )
    .await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);

    let (status, pushed) = json_request(
        app.clone(),
        "POST",
        &format!("/api/v1/projects/{id}/events"),
        Some(&session),
        Some(serde_json::json!({
            "events": [{
                "id": "evt-1",
                "event_type": "gitcell.prompt",
                "payload": { "role": "user", "content": "hello" },
                "created_at": "2026-09-05T00:00:00Z"
            }]
        })),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{pushed}");
    assert_eq!(pushed["accepted"], 1);

    let (status, pulled) = json_request(
        app.clone(),
        "GET",
        &format!("/api/v1/projects/{id}/events"),
        Some(&session),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{pulled}");
    assert_eq!(pulled["events"].as_array().unwrap().len(), 1);
    assert_eq!(pulled["events"][0]["id"], "evt-1");

    let (status, _) = json_request(
        app.clone(),
        "GET",
        &format!("/git/{id}/info/refs?service=git-upload-pack"),
        None,
        None,
    )
    .await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);

    let (status, refs) = json_request(
        app,
        "GET",
        &format!("/git/{id}/info/refs?service=git-upload-pack"),
        Some(&session),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{refs}");
}
