use std::future::Future;
use std::path::PathBuf;
use std::pin::Pin;
use std::task::{Context, Poll};

use axum::body::Body;
use axum::extract::State;
use axum::http::{Request, StatusCode, Uri, header};
use axum::response::{IntoResponse, Response};
use axum::{
    Json, Router,
    routing::{delete, get, post},
};
use gitcell::server::{AppState as GitcellState, api_router};
use serde_json::{Value, json};
use sqlx::SqlitePool;
use tower::{Layer, Service};
use tower_http::cors::CorsLayer;
use tower_http::trace::TraceLayer;

use crate::auth;
use crate::cli_auth;
use crate::error::Error;
use crate::git_http;
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
    pub cells_dir: PathBuf,
    pub cells_storage_dir: PathBuf,
}

pub fn create_router(hub: HubState) -> Router {
    create_router_with_static(hub, None)
}

pub fn create_router_with_static(hub: HubState, static_dir: Option<PathBuf>) -> Router {
    let gitcell_state = hub.gitcell.clone();
    let protected_gitcell = api_router()
        .with_state(gitcell_state)
        .layer(GitcellAuthLayer(hub.clone()));

    let api = Router::new()
        .route("/api/v1/auth/send-code", post(auth::send_code))
        .route("/api/v1/auth/verify-code", post(auth::verify_code))
        .route("/api/v1/auth/register", post(auth::register))
        .route("/api/v1/auth/password", post(auth::set_password))
        .route("/api/v1/auth/login", post(auth::login))
        .route("/api/v1/auth/cli/start", post(cli_auth::start))
        .route("/api/v1/auth/cli/poll", post(cli_auth::poll))
        .route("/api/v1/auth/cli/approve", post(cli_auth::approve))
        .route("/api/v1/auth/forgot/send-code", post(auth::send_reset_code))
        .route(
            "/api/v1/auth/forgot/verify-code",
            post(auth::verify_reset_code),
        )
        .route("/api/v1/auth/forgot/reset", post(auth::reset_password))
        .route("/api/v1/auth/logout", post(auth::logout))
        .route("/api/v1/me", get(auth::me))
        .route("/api/v1/me/password", post(auth::change_password))
        .route(
            "/api/v1/me/tokens",
            get(auth::list_pats).post(auth::create_pat),
        )
        .route("/api/v1/me/tokens/{id}", delete(auth::delete_pat))
        .route(
            "/api/v1/projects",
            get(projects::list).post(projects::create),
        )
        .route("/api/v1/projects/check-slug", get(projects::check_slug))
        .route(
            "/api/v1/projects/{id}",
            get(projects::get).delete(projects::delete),
        )
        .route(
            "/api/v1/projects/{username}/{slug}",
            get(projects::get_user_repo),
        )
        .route(
            "/api/v1/projects/{id}/events",
            get(sessions::pull).post(sessions::push),
        )
        .route(
            "/api/v1/projects/{username}/{slug}/events",
            get(sessions::pull_user_repo).post(sessions::push_user_repo),
        )
        .route(
            "/api/v1/projects/{id}/git/bundle",
            get(git_sync::download).put(git_sync::upload),
        )
        .route("/git/{id}/info/refs", get(git_http::info_refs))
        .route("/git/{id}/git-upload-pack", post(git_http::upload_pack))
        .route("/git/{id}/git-receive-pack", post(git_http::receive_pack))
        .route(
            "/git/{username}/{repo}/info/refs",
            get(git_http::info_refs_user_repo),
        )
        .route(
            "/git/{username}/{repo}/git-upload-pack",
            post(git_http::upload_pack_user_repo),
        )
        .route(
            "/git/{username}/{repo}/git-receive-pack",
            post(git_http::receive_pack_user_repo),
        )
        .with_state(hub);

    let mut app = Router::new()
        .route("/health", get(health_check))
        .route("/api/v1/ping", get(ping))
        .merge(api)
        .merge(protected_gitcell);
    if let Some(dir) = static_dir {
        app = app.fallback_service(Router::new().fallback(serve_spa).with_state(dir));
    }
    app.layer(TraceLayer::new_for_http())
        .layer(CorsLayer::permissive())
}

async fn serve_spa(State(dir): State<PathBuf>, uri: Uri) -> Response {
    let path = uri.path();
    if path.starts_with("/api") || path.starts_with("/git") {
        return Error::NotFound("not found".into()).into_response();
    }
    let rel = path.trim_start_matches('/');
    let candidate = if rel.is_empty() {
        dir.join("index.html")
    } else {
        dir.join(rel)
    };
    let root = dir.canonicalize().unwrap_or_else(|_| dir.clone());
    let file = match candidate.canonicalize() {
        Ok(p) if p.starts_with(&root) && p.is_file() => p,
        _ => dir.join("index.html"),
    };
    match tokio::fs::read(&file).await {
        Ok(bytes) => {
            let ctype = static_content_type(&file);
            ([(header::CONTENT_TYPE, ctype)], bytes).into_response()
        }
        Err(_) => StatusCode::NOT_FOUND.into_response(),
    }
}

fn static_content_type(path: &std::path::Path) -> &'static str {
    match path.extension().and_then(|e| e.to_str()) {
        Some("html") => "text/html; charset=utf-8",
        Some("js") => "text/javascript; charset=utf-8",
        Some("css") => "text/css; charset=utf-8",
        Some("txt") => "text/plain; charset=utf-8",
        Some("md") => "text/markdown; charset=utf-8",
        Some("svg") => "image/svg+xml",
        Some("json") => "application/json",
        Some("woff2") => "font/woff2",
        Some("png") => "image/png",
        Some("ico") => "image/x-icon",
        _ => "application/octet-stream",
    }
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
                    Ok(true) => {
                        let repo = hub.gitcell.data_dir.join(&project_id);
                        if !repo.exists() {
                            if let Err(err) = gitcell::git_ops::init(&repo) {
                                tracing::warn!(
                                    project_id,
                                    error = %err,
                                    "missing git tree; init failed"
                                );
                            } else if let Err(err) = crate::git_http::ensure_http_enabled(&repo) {
                                tracing::warn!(
                                    project_id,
                                    error = %err,
                                    "enable git http failed"
                                );
                            }
                        }
                        inner.call(req).await
                    }
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
