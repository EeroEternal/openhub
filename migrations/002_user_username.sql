-- Migration 002: Add username column to users table
ALTER TABLE users ADD COLUMN username TEXT;

-- Backfill username from email prefix (everything before '@')
UPDATE users
SET username = lower(substr(email, 1, instr(email, '@') - 1))
WHERE username IS NULL;

-- Create unique index on username
CREATE UNIQUE INDEX IF NOT EXISTS idx_users_username ON users(username);
