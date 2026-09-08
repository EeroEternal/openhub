# Deploy OpenHub (GCP + Cloudflare)

Public name: **openhub.run**. Origin VM: **pararouter-hk** (`asia-east2-b`, e2-medium). Cloudflare is DNS + TLS proxy only.

Push to `main` runs CI, then [`.github/workflows/deploy.yml`](../.github/workflows/deploy.yml) builds the Linux binary and Admin UI on GitHub Actions, rsyncs them over SSH, and restarts `openhub.service`. Data under `/home/xinference/openhub-data` is not replaced.

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

## Check after deploy

```bash
curl -sS https://openhub.run/health
# {"status":"ok","service":"openhub"}

curl -sS https://openhub.run/llms.txt | head
# # OpenHub
```

Local `cargo run` still uses `cd admin && npm run dev` unless `OPENHUB_STATIC_DIR` points at a built `admin/dist`.
