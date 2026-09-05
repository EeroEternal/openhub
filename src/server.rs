use std::future::Future;
use std::pin::Pin;
use std::task::{Context, Poll};

use axum::body::Body;
use axum::http::Request;
use axum::response::{IntoResponse, Response};
use axum::{
    Json, Router,
    routing::{get, post},
};
use gitcell::server::{AppState as GitcellState, api_router};
use serde_json::{Value, json};
use sqlx::SqlitePool;
use tower::{Layer, Service};
use tower_http::cors::CorsLayer;
use tower_http::trace::TraceLayer;

use crate::auth;
use crate::error::Error;
use crate::git_sync;
use crate::mail::Mailer;
use crate::projects;
use crate::sessions;
use crate::store;

#[derive(Clone)]
pub struct HubState {
    pub gitcell: GitcellState,
    pub db: SqlitePool,
    pub mail: Mailer,
    pub public_origin: String,
}

pub fn create_router(hub: HubState) -> Router {
    let gitcell_state = hub.gitcell.clone();
    let protected_gitcell = api_router()
        .with_state(gitcell_state)
        .layer(GitcellAuthLayer(hub.clone()));

    let api = Router::new()
        .route("/api/v1/auth/register", post(auth::register))
        .route("/api/v1/auth/password", post(auth::set_password))
        .route("/api/v1/auth/login", post(auth::login))
        .route("/api/v1/auth/logout", post(auth::logout))
        .route("/api/v1/me", get(auth::me))
        .route(
            "/api/v1/projects",
            get(projects::list).post(projects::create),
        )
        .route("/api/v1/projects/{id}", get(projects::get))
        .route(
            "/api/v1/projects/{id}/events",
            get(sessions::pull).post(sessions::push),
        )
        .route(
            "/api/v1/projects/{id}/git/bundle",
            get(git_sync::download).put(git_sync::upload),
        )
        .with_state(hub);

    Router::new()
        .route("/health", get(health_check))
        .route("/api/v1/ping", get(ping))
        .merge(api)
        .merge(protected_gitcell)
        .layer(TraceLayer::new_for_http())
        .layer(CorsLayer::permissive())
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

#[derive(Clone)]
struct GitcellAuthLayer(HubState);

impl<S> Layer<S> for GitcellAuthLayer {
    type Service = GitcellAuthService<S>;

    fn layer(&self, inner: S) -> Self::Service {
        GitcellAuthService {
            inner,
            hub: self.0.clone(),
        }
    }
}

#[derive(Clone)]
struct GitcellAuthService<S> {
    inner: S,
    hub: HubState,
}

impl<S> Service<Request<Body>> for GitcellAuthService<S>
where
    S: Service<Request<Body>, Response = Response> + Clone + Send + 'static,
    S::Future: Send + 'static,
    S::Error: Send + 'static,
{
    type Response = Response;
    type Error = S::Error;
    type Future = Pin<Box<dyn Future<Output = Result<Self::Response, Self::Error>> + Send>>;

    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.inner.poll_ready(cx)
    }

    fn call(&mut self, req: Request<Body>) -> Self::Future {
        let mut inner = self.inner.clone();
        let hub = self.hub.clone();
        Box::pin(async move {
            let path = req.uri().path().to_string();
            if path == "/api/v1/repos" || path == "/api/v1/repos/" {
                return Ok(Error::NotFound("use /api/v1/projects".into()).into_response());
            }
            let Some(project_id) = project_id_from_path(&path).map(str::to_string) else {
                return Ok(Error::NotFound("not found".into()).into_response());
            };
            let headers = req.headers().clone();
            match auth::user_from_headers(&hub, &headers).await {
                Ok(user) => match store::project_owned(&hub.db, &project_id, &user.id).await {
                    Ok(true) => inner.call(req).await,
                    Ok(false) => Ok(Error::NotFound("project not found".into()).into_response()),
                    Err(e) => Ok(e.into_response()),
                },
                Err(e) => Ok(e.into_response()),
            }
        })
    }
}

fn project_id_from_path(path: &str) -> Option<&str> {
    let rest = path.strip_prefix("/api/v1/repos/")?;
    rest.split('/').next().filter(|s| !s.is_empty())
}
