CREATE TABLE IF NOT EXISTS cli_logins (
    id TEXT PRIMARY KEY,
    device_code_hash TEXT NOT NULL UNIQUE,
    user_code TEXT NOT NULL UNIQUE,
    user_id TEXT REFERENCES users (id),
    granted_token TEXT,
    expires_at TEXT NOT NULL,
    created_at TEXT NOT NULL
);
