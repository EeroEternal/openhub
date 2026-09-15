-- Portable SQLite now / Postgres later: TEXT columns, nullable.
-- github_token is write-only: never returned by GET APIs.

ALTER TABLE projects ADD COLUMN github_repo TEXT;
ALTER TABLE projects ADD COLUMN github_token TEXT;
