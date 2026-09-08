# Deploy OpenHub (GCP + Cloudflare)

Public name: **openhub.run**. Origin: one GCP VM. Cloudflare is DNS + TLS proxy only — not Workers, not Pages.

Push to `main` runs CI, then [`.github/workflows/deploy.yml`](../.github/workflows/deploy.yml) builds a Docker image (API + Admin UI), pushes it to GHCR, SSH into the VM, and restarts the `openhub` container. `/var/lib/openhub` is a volume and is not replaced.

## 1. GCP VM (once)

1. Ubuntu LTS VM with a **static external IP**. Docker installed.
2. VPC firewall: **tcp:80** and **tcp:443** from `0.0.0.0/0`. Do not expose 8080.
3. Caddy (or nginx) on 443 with a Cloudflare Origin Certificate. Use [`Caddyfile`](Caddyfile): **reverse_proxy everything** to `127.0.0.1:8080`. Do not `file_server` an old `admin/dist` — that is why the UI went stale.
4. Create `/var/lib/openhub` for SQLite, git repos, and cellz.
5. Install the GitHub deploy public key as `authorized_keys` for `DEPLOY_USER` (docker must run without a TTY sudo prompt; put the user in the `docker` group).

## 2. GitHub secrets (once)

Repo → Settings → Secrets and variables → Actions:

| Secret | Value |
| --- | --- |
| `DEPLOY_HOST` | VM public IP (or DNS that is not orange-clouded for SSH) |
| `DEPLOY_USER` | SSH user that can run `docker` |
| `DEPLOY_SSH_KEY` | Private key matching the VM `authorized_keys` entry |

The workflow logs into GHCR with `GITHUB_TOKEN` (no extra registry secret). First image: `ghcr.io/<owner>/openhub`.

Manual run: Actions → **Deploy production** → Run workflow.

## 3. Cloudflare DNS

| Type | Name | Content | Proxy |
| --- | --- | --- | --- |
| A | `@` | GCP static IP | Proxied |
| A | `www` | GCP static IP | Proxied |

SSH to the VM must use the **grey-cloud** IP (or a separate `ssh.openhub.run` record that is DNS-only). SSL/TLS mode: **Full (strict)**.

## 4. Check after deploy

```bash
curl -sS https://openhub.run/health
# {"status":"ok","service":"openhub"}

curl -sS https://openhub.run/llms.txt | head
# # OpenHub
```

The Admin UI (`/login`, `/help`) is the same origin as `/api`. Local `cargo run` still needs `cd admin && npm run dev` unless `OPENHUB_STATIC_DIR` points at a built `admin/dist`.
