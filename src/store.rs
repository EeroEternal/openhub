//! SQLite today; SQL is kept portable for a later Postgres cutover.

use chrono::{Duration, Utc};
use sqlx::sqlite::{SqliteConnectOptions, SqliteJournalMode, SqlitePoolOptions};
use sqlx::{Row, SqlitePool};
use std::path::Path;

use crate::error::{Error, Result};

#[derive(Debug, Clone)]
pub struct User {
    pub id: String,
    pub email: String,
    pub password_hash: Option<String>,
    pub email_verified_at: Option<String>,
}

#[derive(Debug, Clone)]
pub struct Project {
    pub id: String,
    pub owner_id: String,
    pub slug: String,
    pub name: String,
    pub created_at: String,
}

pub async fn connect(path: &Path) -> Result<SqlitePool> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|e| Error::Config(format!("create db dir: {e}")))?;
    }
    let options = SqliteConnectOptions::new()
        .filename(path)
        .create_if_missing(true)
        .foreign_keys(true)
        .journal_mode(SqliteJournalMode::Wal);
    let pool = SqlitePoolOptions::new()
        .max_connections(5)
        .connect_with(options)
        .await?;
    Ok(pool)
}

pub async fn migrate(pool: &SqlitePool) -> Result<()> {
    sqlx::migrate!("./migrations").run(pool).await?;
    Ok(())
}

pub fn new_id() -> String {
    uuid::Uuid::new_v4().simple().to_string()
}

pub fn now() -> String {
    Utc::now().to_rfc3339()
}

pub async fn find_user_by_email(pool: &SqlitePool, email: &str) -> Result<Option<User>> {
    let row = sqlx::query(
        "SELECT id, email, password_hash, email_verified_at FROM users WHERE email = ?",
    )
    .bind(email)
    .fetch_optional(pool)
    .await?;
    Ok(row.map(row_to_user))
}

pub async fn find_user_by_id(pool: &SqlitePool, id: &str) -> Result<Option<User>> {
    let row =
        sqlx::query("SELECT id, email, password_hash, email_verified_at FROM users WHERE id = ?")
            .bind(id)
            .fetch_optional(pool)
            .await?;
    Ok(row.map(row_to_user))
}

pub async fn insert_user(pool: &SqlitePool, email: &str) -> Result<User> {
    let id = new_id();
    let ts = now();
    sqlx::query(
        "INSERT INTO users (id, email, password_hash, email_verified_at, created_at, updated_at)
         VALUES (?, ?, NULL, NULL, ?, ?)",
    )
    .bind(&id)
    .bind(email)
    .bind(&ts)
    .bind(&ts)
    .execute(pool)
    .await?;
    Ok(User {
        id,
        email: email.to_string(),
        password_hash: None,
        email_verified_at: None,
    })
}

pub async fn set_password_and_verify(
    pool: &SqlitePool,
    user_id: &str,
    password_hash: &str,
) -> Result<()> {
    let ts = now();
    sqlx::query(
        "UPDATE users SET password_hash = ?, email_verified_at = ?, updated_at = ? WHERE id = ?",
    )
    .bind(password_hash)
    .bind(&ts)
    .bind(&ts)
    .bind(user_id)
    .execute(pool)
    .await?;
    Ok(())
}

pub async fn insert_token(
    pool: &SqlitePool,
    user_id: &str,
    purpose: &str,
    token_hash: &str,
    ttl_hours: i64,
) -> Result<()> {
    let id = new_id();
    let ts = now();
    let expires = (Utc::now() + Duration::hours(ttl_hours)).to_rfc3339();
    sqlx::query(
        "INSERT INTO auth_tokens (id, user_id, purpose, token_hash, expires_at, created_at)
         VALUES (?, ?, ?, ?, ?, ?)",
    )
    .bind(&id)
    .bind(user_id)
    .bind(purpose)
    .bind(token_hash)
    .bind(&expires)
    .bind(&ts)
    .execute(pool)
    .await?;
    Ok(())
}

pub async fn take_token(
    pool: &SqlitePool,
    purpose: &str,
    token_hash: &str,
) -> Result<Option<String>> {
    let row = sqlx::query(
        "SELECT id, user_id, expires_at FROM auth_tokens WHERE purpose = ? AND token_hash = ?",
    )
    .bind(purpose)
    .bind(token_hash)
    .fetch_optional(pool)
    .await?;
    let Some(row) = row else {
        return Ok(None);
    };
    let id: String = row.get("id");
    let user_id: String = row.get("user_id");
    let expires_at: String = row.get("expires_at");
    sqlx::query("DELETE FROM auth_tokens WHERE id = ?")
        .bind(&id)
        .execute(pool)
        .await?;
    if expires_at < now() {
        return Ok(None);
    }
    Ok(Some(user_id))
}

pub async fn find_session_user(pool: &SqlitePool, token_hash: &str) -> Result<Option<User>> {
    let row = sqlx::query(
        "SELECT u.id, u.email, u.password_hash, u.email_verified_at, t.expires_at
         FROM auth_tokens t
         JOIN users u ON u.id = t.user_id
         WHERE t.purpose = ? AND t.token_hash = ?",
    )
    .bind("session")
    .bind(token_hash)
    .fetch_optional(pool)
    .await?;
    let Some(row) = row else {
        return Ok(None);
    };
    let expires_at: String = row.get("expires_at");
    if expires_at < now() {
        return Ok(None);
    }
    Ok(Some(row_to_user(row)))
}

pub async fn delete_session(pool: &SqlitePool, token_hash: &str) -> Result<()> {
    sqlx::query("DELETE FROM auth_tokens WHERE purpose = ? AND token_hash = ?")
        .bind("session")
        .bind(token_hash)
        .execute(pool)
        .await?;
    Ok(())
}

pub async fn insert_project(
    pool: &SqlitePool,
    owner_id: &str,
    slug: &str,
    name: &str,
) -> Result<Project> {
    let id = new_id();
    let ts = now();
    sqlx::query(
        "INSERT INTO projects (id, owner_id, slug, name, created_at, updated_at)
         VALUES (?, ?, ?, ?, ?, ?)",
    )
    .bind(&id)
    .bind(owner_id)
    .bind(slug)
    .bind(name)
    .bind(&ts)
    .bind(&ts)
    .execute(pool)
    .await?;
    Ok(Project {
        id,
        owner_id: owner_id.to_string(),
        slug: slug.to_string(),
        name: name.to_string(),
        created_at: ts,
    })
}

pub async fn list_projects(pool: &SqlitePool, owner_id: &str) -> Result<Vec<Project>> {
    let rows = sqlx::query(
        "SELECT id, owner_id, slug, name, created_at FROM projects WHERE owner_id = ? ORDER BY created_at",
    )
    .bind(owner_id)
    .fetch_all(pool)
    .await?;
    Ok(rows.iter().map(row_to_project).collect())
}

pub async fn get_project(pool: &SqlitePool, id: &str) -> Result<Option<Project>> {
    let row = sqlx::query("SELECT id, owner_id, slug, name, created_at FROM projects WHERE id = ?")
        .bind(id)
        .fetch_optional(pool)
        .await?;
    Ok(row.as_ref().map(row_to_project))
}

pub async fn project_owned(pool: &SqlitePool, project_id: &str, owner_id: &str) -> Result<bool> {
    let row = sqlx::query("SELECT 1 AS ok FROM projects WHERE id = ? AND owner_id = ?")
        .bind(project_id)
        .bind(owner_id)
        .fetch_optional(pool)
        .await?;
    Ok(row.is_some())
}

fn row_to_user(row: sqlx::sqlite::SqliteRow) -> User {
    User {
        id: row.get("id"),
        email: row.get("email"),
        password_hash: row.get("password_hash"),
        email_verified_at: row.get("email_verified_at"),
    }
}

fn row_to_project(row: &sqlx::sqlite::SqliteRow) -> Project {
    Project {
        id: row.get("id"),
        owner_id: row.get("owner_id"),
        slug: row.get("slug"),
        name: row.get("name"),
        created_at: row.get("created_at"),
    }
}
