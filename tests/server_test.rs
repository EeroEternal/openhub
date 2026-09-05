use axum::{
    body::Body,
    http::{Request, StatusCode},
};
use gitcell::server::AppState;
use http_body_util::BodyExt;
use openhub::server::create_router;
use tower::ServiceExt;

fn test_state() -> (AppState, tempfile::TempDir) {
    let tmp = tempfile::tempdir().unwrap();
    let state = AppState::new(
        tmp.path().join("repos"),
        tmp.path().join("cells"),
        tmp.path().join("cells-storage"),
        60,
    )
    .unwrap();
    (state, tmp)
}

#[tokio::test]
async fn test_health_check() {
    let (state, _tmp) = test_state();
    let app = create_router(state);

    let response = app
        .oneshot(
            Request::builder()
                .uri("/health")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let body = response.into_body().collect().await.unwrap().to_bytes();
    let json: serde_json::Value = serde_json::from_slice(&body).unwrap();

    assert_eq!(json["status"], "ok");
    assert_eq!(json["service"], "openhub");
}

#[tokio::test]
async fn test_embedded_gitcell_init() {
    let (state, _tmp) = test_state();
    let app = create_router(state);

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/repos/demo/init")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
}
