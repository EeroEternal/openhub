use axum::{
    body::Body,
    http::{Request, StatusCode},
};
use gitcell::server::AppState as GitcellState;
use http_body_util::BodyExt;
use openhub::mail::Mailer;
use openhub::server::{HubState, create_router, create_router_with_static};
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
        cells_dir: tmp.path().join("cells"),
        cells_storage_dir: tmp.path().join("cells-storage"),
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
    let app = create_router(hub);

    let (status, body) = json_request(
        app.clone(),
        "POST",
        "/api/v1/auth/register",
        None,
        Some(serde_json::json!({
            "email": "Ada@Example.com",
            "password": "password1"
        })),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let session = body["token"].as_str().unwrap().to_string();

    let (status, me) = json_request(app.clone(), "GET", "/api/v1/me", Some(&session), None).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(me["email"], "ada@example.com");
    assert!(me.get("password_hash").is_none());

    let (status, _) = json_request(
        app.clone(),
        "POST",
        "/api/v1/me/password",
        Some(&session),
        Some(serde_json::json!({
            "current_password": "password1",
            "new_password": "password2"
        })),
    )
    .await;
    assert_eq!(status, StatusCode::OK);

    let (status, _) = json_request(
        app.clone(),
        "POST",
        "/api/v1/auth/login",
        None,
        Some(serde_json::json!({
            "email": "ada@example.com",
            "password": "password1"
        })),
    )
    .await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);

    let (status, login) = json_request(
        app.clone(),
        "POST",
        "/api/v1/auth/login",
        None,
        Some(serde_json::json!({
            "email": "ada@example.com",
            "password": "password2"
        })),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{login}");

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

#[tokio::test]
async fn test_send_code_and_register() {
    let (hub, _tmp) = test_hub().await;
    let app = create_router(hub);

    // 1. Send code
    let (status, res) = json_request(
        app.clone(),
        "POST",
        "/api/v1/auth/send-code",
        None,
        Some(serde_json::json!({
            "email": "coder@example.com"
        })),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{res}");
    assert_eq!(res["ok"], true);

    // 2. Register without code in non-mail environment still succeeds or requires code
    let (status, body) = json_request(
        app.clone(),
        "POST",
        "/api/v1/auth/register",
        None,
        Some(serde_json::json!({
            "email": "coder@example.com",
            "password": "mypassword123"
        })),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert!(body["token"].as_str().is_some());
    let token = body["token"].as_str().unwrap();

    // 3. Check slug before project created: should be available
    let (status, slug_res) = json_request(
        app.clone(),
        "GET",
        "/api/v1/projects/check-slug?slug=my-cool-project",
        Some(token),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(slug_res["available"], true);

    // 4. Create project with that slug
    let (status, create_res) = json_request(
        app.clone(),
        "POST",
        "/api/v1/projects",
        Some(token),
        Some(serde_json::json!({
            "name": "My Cool Project",
            "slug": "my-cool-project"
        })),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{create_res}");

    // 5. Check slug again: should be unavailable
    let (status, slug_res2) = json_request(
        app.clone(),
        "GET",
        "/api/v1/projects/check-slug?slug=my-cool-project",
        Some(token),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(slug_res2["available"], false);
}

#[tokio::test]
async fn serves_static_files_and_spa_fallback() {
    let (hub, _tmp) = test_hub().await;
    let web = tempfile::tempdir().unwrap();
    std::fs::write(web.path().join("llms.txt"), "# OpenHub\n").unwrap();
    std::fs::write(
        web.path().join("index.html"),
        "<!doctype html><title>OpenHub</title>",
    )
    .unwrap();
    let app = create_router_with_static(hub, Some(web.path().to_path_buf()));

    let (status, body) = raw_request(app.clone(), "/llms.txt").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(String::from_utf8(body).unwrap(), "# OpenHub\n");

    let (status, body) = raw_request(app.clone(), "/help").await;
    assert_eq!(status, StatusCode::OK);
    assert!(String::from_utf8_lossy(&body).contains("<title>OpenHub</title>"));

    let (status, _) = raw_request(app, "/api/v1/does-not-exist").await;
    assert_eq!(status, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn forgot_password_resets_and_revokes_old_login() {
    let (hub, _tmp) = test_hub().await;
    let app = create_router(hub);

    let (status, body) = json_request(
        app.clone(),
        "POST",
        "/api/v1/auth/register",
        None,
        Some(serde_json::json!({
            "email": "reset@example.com",
            "password": "oldpass12"
        })),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");

    let (status, sent) = json_request(
        app.clone(),
        "POST",
        "/api/v1/auth/forgot/send-code",
        None,
        Some(serde_json::json!({ "email": "nobody@example.com" })),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{sent}");
    assert_eq!(sent["ok"], true);

    let (status, sent) = json_request(
        app.clone(),
        "POST",
        "/api/v1/auth/forgot/send-code",
        None,
        Some(serde_json::json!({ "email": "reset@example.com" })),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{sent}");

    let (status, reset) = json_request(
        app.clone(),
        "POST",
        "/api/v1/auth/forgot/reset",
        None,
        Some(serde_json::json!({
            "email": "reset@example.com",
            "password": "newpass12"
        })),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{reset}");
    assert!(reset["token"].as_str().is_some());

    let (status, _) = json_request(
        app.clone(),
        "POST",
        "/api/v1/auth/login",
        None,
        Some(serde_json::json!({
            "email": "reset@example.com",
            "password": "oldpass12"
        })),
    )
    .await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);

    let (status, login) = json_request(
        app.clone(),
        "POST",
        "/api/v1/auth/login",
        None,
        Some(serde_json::json!({
            "email": "reset@example.com",
            "password": "newpass12"
        })),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{login}");
}

#[tokio::test]
async fn delete_project_removes_it() {
    let (hub, _tmp) = test_hub().await;
    let app = create_router(hub);

    let (status, body) = json_request(
        app.clone(),
        "POST",
        "/api/v1/auth/register",
        None,
        Some(serde_json::json!({
            "email": "owner@example.com",
            "password": "password1"
        })),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let token = body["token"].as_str().unwrap();

    let (status, project) = json_request(
        app.clone(),
        "POST",
        "/api/v1/projects",
        Some(token),
        Some(serde_json::json!({ "name": "To Delete" })),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{project}");
    let id = project["id"].as_str().unwrap();

    let (status, deleted) = json_request(
        app.clone(),
        "DELETE",
        &format!("/api/v1/projects/{id}"),
        Some(token),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{deleted}");

    let (status, _) = json_request(
        app.clone(),
        "GET",
        &format!("/api/v1/projects/{id}"),
        Some(token),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND);

    let (status, list) = json_request(app, "GET", "/api/v1/projects", Some(token), None).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(list["projects"].as_array().map(|a| a.len()), Some(0));
}

#[tokio::test]
async fn cli_browser_login_issues_session() {
    let (hub, _tmp) = test_hub().await;
    let app = create_router(hub);

    let (status, start) =
        json_request(app.clone(), "POST", "/api/v1/auth/cli/start", None, None).await;
    assert_eq!(status, StatusCode::OK, "{start}");
    let device_code = start["device_code"].as_str().unwrap();
    let user_code = start["user_code"].as_str().unwrap();
    assert!(
        start["verification_uri"]
            .as_str()
            .unwrap()
            .contains("/cli?code=")
    );

    let (status, poll) = json_request(
        app.clone(),
        "POST",
        "/api/v1/auth/cli/poll",
        None,
        Some(serde_json::json!({ "device_code": device_code })),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{poll}");
    assert_eq!(poll["status"], "pending");

    let (status, _) = json_request(
        app.clone(),
        "POST",
        "/api/v1/auth/cli/approve",
        None,
        Some(serde_json::json!({ "code": user_code })),
    )
    .await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);

    let (status, registered) = json_request(
        app.clone(),
        "POST",
        "/api/v1/auth/register",
        None,
        Some(serde_json::json!({
            "email": "cli@example.com",
            "password": "password1"
        })),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{registered}");
    let session = registered["token"].as_str().unwrap();

    let (status, approved) = json_request(
        app.clone(),
        "POST",
        "/api/v1/auth/cli/approve",
        Some(session),
        Some(serde_json::json!({ "code": user_code })),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{approved}");

    let (status, granted) = json_request(
        app.clone(),
        "POST",
        "/api/v1/auth/cli/poll",
        None,
        Some(serde_json::json!({ "device_code": device_code })),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{granted}");
    assert_eq!(granted["status"], "ok");
    let cli_token = granted["token"].as_str().unwrap();
    assert!(!cli_token.is_empty());

    let (status, me) = json_request(app.clone(), "GET", "/api/v1/me", Some(cli_token), None).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(me["email"], "cli@example.com");

    let (status, again) = json_request(
        app,
        "POST",
        "/api/v1/auth/cli/poll",
        None,
        Some(serde_json::json!({ "device_code": device_code })),
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND, "{again}");
}

async fn raw_request(app: axum::Router, uri: &str) -> (StatusCode, Vec<u8>) {
    let response = app
        .oneshot(
            Request::builder()
                .method("GET")
                .uri(uri)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let status = response.status();
    let bytes = response.into_body().collect().await.unwrap().to_bytes();
    (status, bytes.to_vec())
}
