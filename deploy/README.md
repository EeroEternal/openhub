# Deploy OpenHub (GCP + Cloudflare)

Public name: **openhub.run**. Origin VM: **pararouter-hk** (`asia-east2-b`, e2-medium). Cloudflare is DNS + TLS proxy only.

Run [`scripts/ci_local.sh`](../scripts/ci_local.sh) locally first (GitHub Actions no longer runs the test matrix). Push to `main` then triggers [`.github/workflows/deploy.yml`](../.github/workflows/deploy.yml), which builds the Linux binary and Admin UI, rsyncs them over SSH, and restarts `openhub.service`. Data under `/home/xinference/openhub-data` is not replaced.

## What runs on the VM

| Piece | Path |
| --- | --- |
| systemd | `openhub.service` → `/home/xinference/src/openhub/target/release/openhub` |
| nginx static | `/home/xinference/openhub-www` (`/api/`, `/git/`, `/health` proxied to `:8080`) |
| data | `/home/xinference/openhub-data/{repos,cells,cells-storage}` |

Do not `docker run` a second origin on this host: port 8080 is already the systemd process.

## GitHub secrets

Repo → Settings → Secrets and variables → Actions:

| Secret | Value |
| --- | --- |
| `DEPLOY_HOST` | `34.96.247.243` |
| `DEPLOY_USER` | `xinference` |
| `DEPLOY_SSH_KEY` | Deploy-only private key (public half in that user's `authorized_keys`) |

Manual run: Actions → **Deploy production** → Run workflow.

## GitHub OAuth (Settings → GitHub)

Create a GitHub OAuth App (not a GitHub App):

1. GitHub → **Settings** → **Developer settings** → **OAuth Apps** → **New OAuth App**
2. Application name: `OpenHub`
3. Homepage URL: `https://openhub.run`
4. Authorization callback URL: `https://openhub.run/api/v1/auth/github/callback`
5. Copy **Client ID** and generate a **Client secret**

On the origin VM, put them on `openhub.service` (then `sudo systemctl daemon-reload && sudo systemctl restart openhub`):

```
Environment=OPENHUB_GITHUB_CLIENT_ID=Iv1...
Environment=OPENHUB_GITHUB_CLIENT_SECRET=...
```

Local: export the same two variables before `cargo run`. Until they are set, Settings → GitHub shows that OAuth is not configured.

## Check after deploy

```bash
curl -sS https://openhub.run/health
# {"status":"ok","service":"openhub"}

curl -sS https://openhub.run/llms.txt | head
# # OpenHub
```

Local `cargo run` still uses `cd admin && npm run dev` unless `OPENHUB_STATIC_DIR` points at a built `admin/dist`.
