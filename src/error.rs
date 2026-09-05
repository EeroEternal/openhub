use axum::{
    Json,
    http::StatusCode,
    response::{IntoResponse, Response},
};
use serde_json::json;

pub type Result<T> = std::result::Result<T, Error>;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("Configuration error: {0}")]
    Config(String),

    #[error("Invalid request: {0}")]
    InvalidRequest(String),

    #[error("Unauthorized: {0}")]
    Unauthorized(String),

    #[error("Not found: {0}")]
    NotFound(String),

    #[error("Conflict: {0}")]
    Conflict(String),

    #[error("Database error: {0}")]
    Database(#[from] sqlx::Error),

    #[error("Internal error: {0}")]
    Internal(#[from] anyhow::Error),
}

impl From<sqlx::migrate::MigrateError> for Error {
    fn from(value: sqlx::migrate::MigrateError) -> Self {
        Error::Internal(anyhow::anyhow!(value.to_string()))
    }
}

impl IntoResponse for Error {
    fn into_response(self) -> Response {
        let (status, message) = match &self {
            Error::NotFound(msg) => (StatusCode::NOT_FOUND, msg.clone()),
            Error::InvalidRequest(msg) | Error::Config(msg) => {
                (StatusCode::BAD_REQUEST, msg.clone())
            }
            Error::Unauthorized(msg) => (StatusCode::UNAUTHORIZED, msg.clone()),
            Error::Conflict(msg) => (StatusCode::CONFLICT, msg.clone()),
            Error::Database(err) => {
                let s = err.to_string();
                if s.contains("UNIQUE") {
                    (StatusCode::CONFLICT, "already exists".into())
                } else {
                    (StatusCode::INTERNAL_SERVER_ERROR, s)
                }
            }
            Error::Internal(err) => (StatusCode::INTERNAL_SERVER_ERROR, err.to_string()),
        };

        let body = Json(json!({
            "error": {
                "message": message,
                "code": status.as_u16(),
            }
        }));

        (status, body).into_response()
    }
}
