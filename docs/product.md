# OpenHub Product Design

Public origin: **https://openhub.run**  
Audience: many users, each with their own projects. Not a GitHub clone.  
Local tool: a CLI that talks a documented HTTPS protocol to OpenHub, syncing **git history** and **agent sessions** (cellz).

This document is the product contract. Implementation follows it; skeleton APIs are not “done”.

---

## 1. What OpenHub is

A hosted account + project service. After signup, a user creates projects. Each project is:

| Piece | Store | Syncs to laptop? |
| --- | --- | --- |
| Git working tree + commits | On-disk git repo (`OPENHUB_DATA_DIR/<project_id>`) | Yes — code |
| Agent session log | One cellz cell (`OPENHUB_CELLS_DIR/<project_id>`) | Yes — prompts, tool traces, workflow events |
| Who owns it | SQLite (later Postgres) | No — server only |

`gitcell` stays the **repo/session engine**. OpenHub adds **users, projects, auth, and the sync protocol**. OpenHub does not reimplement git and does not execute user programs.

```text
Laptop                              openhub.run
┌─────────────────────┐             ┌──────────────────────────────┐
│ working tree        │   git       │ users / projects  (SQLite)   │
│ local cellz cell    │   + ohp/1   │ git repo per project         │
│ `oh` CLI            │ ──────────► │ cellz cell per project       │
└─────────────────────┘             └──────────────────────────────┘
```

---

## 2. Non-goals (v1)

- GitHub-style PR / Issue / org billing
- Implementing git object format ourselves
- Workers / D1 as the git store
- Jumping straight to Postgres
- Public anonymous push

---

## 3. Accounts

### 3.1 Flow

1. `POST /api/v1/auth/register` `{ "email": "a@b.c" }`  
   Create a user with `email_verified_at = NULL`, `password_hash = NULL`.  
   Send a one-time link via the mail adapter (Cloudflare mail token).
2. User opens `https://openhub.run/auth/verify?token=...`  
   Token is single-use, TTL ~24h.
3. `POST /api/v1/auth/password` `{ "token", "password" }`  
   Sets password, marks email verified, issues a session.
4. Later: `POST /api/v1/auth/login` `{ "email", "password" }` → session.  
   CLI: `oh login` opens the browser to authorize, then stores a bearer token in `~/.openhub/credentials` (0600). Email/password and `--token` still work.

Password: salted hash (Argon2id). Session: random token hashed at rest, TTL ~30 days, revoke on logout.

### 3.2 Mail

Do not hardcode a vendor in the core. `Mailer` trait:

- `send_verify_email(to, verify_url)`
- First adapter: HTTP + bearer token (the Cloudflare mail token you provide)

Core never talks SMTP directly.

---

## 4. Database

**Now: SQLite** (one file, e.g. `OPENHUB_DATA_DIR/../openhub.db`).  
**Later: Postgres** — same logical schema, `sqlx` + migrations, no Postgres-only SQL in v1 (`RETURNING` is OK; no `JSONB` operators, no `ARRAY`).

### 4.1 Tables (v1)

```sql
CREATE TABLE users (
    id            TEXT PRIMARY KEY,          -- ulid/uuid
    email         TEXT NOT NULL UNIQUE,
    password_hash TEXT,
    email_verified_at TEXT,
    created_at    TEXT NOT NULL,
    updated_at    TEXT NOT NULL
);

CREATE TABLE auth_tokens (
    id         TEXT PRIMARY KEY,
    user_id    TEXT NOT NULL REFERENCES users(id),
    purpose    TEXT NOT NULL,               -- verify | reset | session
    token_hash TEXT NOT NULL UNIQUE,
    expires_at TEXT NOT NULL,
    created_at TEXT NOT NULL
);

CREATE TABLE projects (
    id         TEXT PRIMARY KEY,
    owner_id   TEXT NOT NULL REFERENCES users(id),
    slug       TEXT NOT NULL,               -- [a-z0-9-]{1,40}
    name       TEXT NOT NULL,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL,
    UNIQUE (owner_id, slug)
);
```

v1: only the owner can read/write a project. `project_members` is v2.

Project id is the **gitcell cell id** and the **directory name** (hex/uuid without `/`). That keeps gitcell’s `^[A-Za-z0-9_-]+$` rule.

---

## 5. Protocol: `ohp/1`

One HTTPS protocol, two payloads. Version header: `X-OpenHub-Protocol: ohp/1`.

Auth: `Authorization: Bearer <session>`.

### 5.1 Control plane (JSON)

| Method | Path | Purpose |
| --- | --- | --- |
| POST | `/api/v1/auth/register` | start signup |
| POST | `/api/v1/auth/password` | set password after mail |
| POST | `/api/v1/auth/login` | session |
| POST | `/api/v1/auth/logout` | revoke |
| GET | `/api/v1/me` | current user |
| POST | `/api/v1/projects` | `{ name, slug? }` |
| GET | `/api/v1/projects` | list mine |
| GET | `/api/v1/projects/{id}` | metadata |

Existing gitcell routes stay, but **must require auth** and resolve `{repo}` to a project the caller owns. Do not expose unauthenticated `/api/v1/repos/...` on the public host.

### 5.2 Code sync — git, not a new VCS

Laptop has a normal git repo. Cloud has the same. Sync **is git**.

v1 transport (pick one, document it):

- **Preferred:** git Smart HTTP at  
  `https://openhub.run/git/{project_id}`  
  (`git-http-backend` in front of the on-disk repo, Basic/Bearer mapped to the session).  
  Then `git push origin main` works. `oh push` can shell out to git.
- **Fallback if we delay Smart HTTP:** `POST /api/v1/projects/{id}/git/bundle` upload/download `git bundle`. CLI-only, no raw `git remote`. Acceptable for v1 if timeboxed.

Do **not** invent a third object store.

### 5.3 Session sync — cellz events

Git does not carry agent history. Session sync is append-only **by event id**.

Cloud cell is canonical.

```
GET  /api/v1/projects/{id}/events?since=<seq>&limit=
POST /api/v1/projects/{id}/events   { "events": [ {id, type, payload, created_at}, ... ] }
```

Push rules:

- Client sends events whose `id` the server does not have.
- Server **appends** them (new sequences on the cloud cell). Client ids are preserved in payload/`id`.
- Pull: client fetches `since` last applied cloud sequence, inserts missing ids locally.

Conflict: two laptops both append. That is OK — logs merge as two interleaved histories, not a git rebase. UI/CLI shows by `created_at` and commit SHA already stored on `gitcell.prompt`.

`oh sync` = `git push/pull` + event pull + event push, one command.

Local cellz remains a cache. After pull, local sequence may differ from cloud; identity of an interaction is **event id**, not local sequence.

---

## 6. CLI (`oh`)

Not a second daemon requirement. Optional local gitcell for offline; v1 CLI can:

```text
oh login
oh project create my-app
oh clone <project>          # git clone + initial event pull
oh sync                     # git + sessions
oh status                   # git status + unsynced event count
```

Working tree is a normal directory the user chooses (`~/src/my-app`), not forced under `./data/repos`. Cloud still uses `OPENHUB_DATA_DIR/<project_id>`.

---

## 7. Layering

```text
Transport     src/server.rs          HTTP, cookies/bearer, no SQL
Auth          src/auth/              register, mail, password, sessions
Projects      src/projects/          CRUD, ACL (owner-only v1)
Gitcell       gitcell::api_router    already exists; wrap with auth + id map
Mail          src/mail/              trait + Cloudflare-token adapter
Store         src/store/             sqlx SQLite; Postgres is a feature later
```

Transport must not query SQL. gitcell must not know about users.

---

## 8. Build order

1. **SQLite + users + mail verify + password + login**  
   Public `/api/v1/repos` closed unless authenticated.
2. **Projects** create/list; each project creates gitcell cell + git init.
3. **Session push/pull** (`/events`) + `oh sync` sessions half.
4. **Code sync** — git bundle first if faster, then Smart HTTP.
5. **CLI** `oh login | project | clone | sync`.
6. Postgres only when SQLite backup/ops hurt (measure; not a date).

Done when: two machines, same user, `oh sync` after local commits and prompts, the other machine sees **commits and those prompts**.

---

## 9. Security (v1)

- TLS only at Cloudflare; origin on loopback.
- Hash tokens and passwords; never store raw session tokens.
- Rate-limit register/login.
- Project id is unguessable (uuid) **and** still check owner_id.
- Mail token stays in env (`OPENHUB_MAIL_TOKEN`), never in git.
- Rotate the Origin certificate that was pasted in chat when convenient.
