use axum::{Json, Router, routing::get};
use gitcell::server::{AppState, api_router};
use serde_json::{Value, json};
use tower_http::cors::CorsLayer;
use tower_http::trace::TraceLayer;

pub fn create_router(state: AppState) -> Router {
    Router::new()
        .route("/health", get(health_check))
        .route("/api/v1/ping", get(ping))
        .merge(api_router())
        .layer(TraceLayer::new_for_http())
        .layer(CorsLayer::permissive())
        .with_state(state)
}

async fn health_check() -> Json<Value> {
    Json(json!({
        "status": "ok",
        "service": "openhub"
    }))
}

async fn ping() -> Json<Value> {
    Json(json!({
        "message": "pong"
    }))
}
