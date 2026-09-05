# Deploy OpenHub (GCP + Cloudflare)

Public name: **openhun.run**. Origin: one GCP VM running this binary (or the Docker image). Cloudflare is DNS + TLS proxy only — not Workers, not Pages.

## 1. GCP VM

1. Create a VM (Ubuntu LTS, e2-small is enough to start) with a **static external IP**.
2. VPC firewall: allow **tcp:80** and **tcp:443** from `0.0.0.0/0`. Do not expose 8080 publicly; Caddy/nginx on the VM listens 443 and reverse-proxies to `127.0.0.1:8080`.
3. Install Docker (or build with rustup). Git is only needed later if this host also runs `gitcell`.

## 2. Run the origin

From this repo on the VM:

```bash
docker build -t openhub:local -f deploy/Dockerfile .
docker run -d --name openhub --restart unless-stopped \
  -p 127.0.0.1:8080:8080 \
  -v /var/lib/openhub:/data \
  -e OPENHUB_PUBLIC_ORIGIN=https://openhun.run \
  openhub:local
```

Health: `curl -s http://127.0.0.1:8080/health`.

Put Caddy (or nginx) in front with a **Cloudflare Origin Certificate** (Cloudflare dashboard → SSL → Origin Server). Example Caddyfile:

```caddy
openhun.run, www.openhun.run {
    reverse_proxy 127.0.0.1:8080
    tls /etc/caddy/origin.pem /etc/caddy/origin.key
}
```

## 3. Cloudflare DNS

In the zone **openhun.run**:

| Type | Name | Content | Proxy |
| --- | --- | --- | --- |
| A | `@` | GCP static IP | Proxied (orange cloud) |
| A | `www` | GCP static IP | Proxied |

SSL/TLS mode: **Full (strict)**.

Optional: Cloudflare Access on `openhun.run` so the origin stays without app-level auth for now.

## 4. Check

```bash
curl -sS https://openhun.run/health
# {"status":"ok","service":"openhub"}
```
