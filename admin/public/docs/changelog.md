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
- CLI `oh`: `login`, `project create`, `clone`, `sync`, `merge`.
