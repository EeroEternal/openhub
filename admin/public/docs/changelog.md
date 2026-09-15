# OpenHub changelog

Product log for agents. Not git history.

## 2026-04 — Auth split + agent docs

- Login and register use the console-kit auth split (branding panel + one form card).
- Public `/help` renders this Markdown. Agents should prefer `/llms.txt` and `/docs/guide.md`.

## Shipped (v1)

- Email + password accounts. Register with a mailed 6-digit code.
- Personal access tokens (`oh_…`), hashed at rest, shown once on create.
- Projects: create/list/get by id or `username/slug`. Owner-only.
- Git Smart HTTP at `/git/{id}` and `/git/{username}/{repo}`.
- Git bundle fallback at `/api/v1/projects/{id}/git/bundle`.
- Agent session push/pull at `/api/v1/projects/{id}/events`.
- CLI `oh`: `login`, `project create`, `init`, `clone`, `sync`, `merge`.
- `oh project create` inside a git repo writes `.openhub/config.json` (no hand-edited config). `oh init <id>` links cwd to an existing project.
- `oh sync` no longer ignores a rejected git push. If the remote is only the OpenHub placeholder README commit, it replaces that with local history. `oh project create` in a git repo sends `init_readme: false` so the first push is a fast-forward.
- `oh github set [owner/repo]` stores a GitHub URL in `.openhub/config.json`. `oh sync` then pushes OpenHub, then GitHub (local git auth). Not a GitHub App; OpenHub does not store GitHub tokens.
- Project Settings: link `owner/repo` + GitHub PAT (never returned on GET) and **Push to GitHub** (`POST /api/v1/projects/{id}/github/push`).
