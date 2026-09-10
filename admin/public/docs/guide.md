# OpenHub agent guide

Public origin: `https://openhub.run`.

OpenHub hosts **git repositories** and **agent session logs** per project. It does not execute user programs, and it is not a GitHub clone.

v1 limits (honest):

- Only the project owner can read or write. No collaborators.
- No pull requests, issues, or org billing.
- No anonymous public push.
- Identity is **email**, not username-only login.

## Auth

Session tokens come from login or registration. Personal access tokens (`oh_…`) are created in Settings and shown once.

```
Authorization: Bearer <session or oh_ token>
```

| Method | Path | Body |
| --- | --- | --- |
| POST | `/api/v1/auth/send-code` | `{ "email" }` |
| POST | `/api/v1/auth/verify-code` | `{ "email", "code" }` |
| POST | `/api/v1/auth/register` | `{ "email", "code", "username?", "password" }` → `{ token }` |
| POST | `/api/v1/auth/login` | `{ "email", "password" }` → `{ token }` |
| POST | `/api/v1/auth/forgot/send-code` | `{ "email" }` always `{ ok: true }` |
| POST | `/api/v1/auth/forgot/verify-code` | `{ "email", "code" }` |
| POST | `/api/v1/auth/forgot/reset` | `{ "email", "code", "password" }` → `{ token }` |
| POST | `/api/v1/auth/logout` | |
| GET | `/api/v1/me` | current user |
| POST | `/api/v1/me/password` | `{ "current_password", "new_password" }` |
| GET | `/api/v1/me/tokens` | list PATs (prefix only) |
| POST | `/api/v1/me/tokens` | `{ "name", "expires_in_days?" }` → `{ token }` once |
| DELETE | `/api/v1/me/tokens/{id}` | revoke PAT |

Password minimum is 8 characters. Send-code mails a 6-digit code.

CLI:

```
oh login [--url https://openhub.run]
oh login <email> <password> [--url https://openhub.run]
oh login --token <token> [--url https://openhub.run]
```

`oh login` with no arguments opens the browser. Sign in on the site, then approve the CLI.

Credentials are stored in `~/.openhub/credentials` (mode 0600).

## Projects

| Method | Path | Purpose |
| --- | --- | --- |
| POST | `/api/v1/projects` | `{ "name", "slug?", "description?", "init_readme?" }` |
| GET | `/api/v1/projects` | list mine |
| GET | `/api/v1/projects/{id}` | metadata |
| DELETE | `/api/v1/projects/{id}` | owner-only; removes git tree and session files |
| GET | `/api/v1/projects/{username}/{slug}` | same, by owner/slug |
| GET | `/api/v1/projects/check-slug?slug=` | availability |

CLI: `oh project create <name>`.

## Git

Preferred clone URL:

```
https://openhub.run/git/{username}/{slug}
```

`{project_id}` also works: `https://openhub.run/git/{id}`.

Smart HTTP routes:

- `GET /git/{id}/info/refs`
- `POST /git/{id}/git-upload-pack`
- `POST /git/{id}/git-receive-pack`

Same three paths exist under `/git/{username}/{repo}/…`.

Git auth is `Authorization: Bearer <token>`, or HTTP Basic where the **password** is the session / PAT (username ignored).

Fallback (CLI `oh clone` / `oh sync`): `GET`/`PUT /api/v1/projects/{id}/git/bundle`.

## Agent sessions

Identity of an event is the client **event id**, not a local sequence. Cloud cell is canonical. Two laptops may both append; logs merge.

```
GET  /api/v1/projects/{id}/events?since=<seq>&limit=
POST /api/v1/projects/{id}/events
     { "events": [ { "id", "event_type", "payload", "created_at" }, ... ] }
```

The `{username}/{slug}` path works the same. `oh sync` = git push/pull + event pull + event push.

## Health

`GET /health` → `{ "status": "ok", "service": "openhub" }`.

## Install CLI

```
curl -fsSL https://openhub.run/install.sh | sh
```
