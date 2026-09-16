-- Account-level GitHub OAuth. Token is write-only: never returned by GET APIs.
-- Portable SQLite now / Postgres later: TEXT columns, nullable.

ALTER TABLE users ADD COLUMN github_id TEXT;
ALTER TABLE users ADD COLUMN github_login TEXT;
ALTER TABLE users ADD COLUMN github_token TEXT;
