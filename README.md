# OpenHub

Hosted origin for **[openhun.run](https://openhun.run)**.

Compute runs on a **GCP VM**. **Cloudflare** holds the domain and terminates public TLS, then proxies to that VM. The origin embeds [`gitcell`](https://github.com/EeroEternal/gitcell) for per-repo git ops, agent prompt history (cellz), and local workflows.

```text
browser / agent
    │
    ▼
openhun.run          Cloudflare DNS + proxy + TLS
    │
    ▼
GCP VM :8080         openhub  (embeds gitcell)
    │
    ├── /health
    └── /api/v1/repos/...
```

## Local

```bash
cargo run
curl -s http://127.0.0.1:8080/health
# {"status":"ok","service":"openhub"}

curl -X POST http://127.0.0.1:8080/api/v1/repos/demo/init
```

Working trees: `$OPENHUB_DATA_DIR/<repo>` (default `./data/repos/<repo>`). Same gitcell HTTP API as the gitcell README.

| Variable | Default | Purpose |
| --- | --- | --- |
| `OPENHUB_HOST` | `0.0.0.0` | Bind address |
| `OPENHUB_PORT` | `8080` | Bind port |
| `OPENHUB_PUBLIC_ORIGIN` | `https://openhun.run` | Public URL |
| `OPENHUB_DATA_DIR` | `./data/repos` | Git working trees |
| `OPENHUB_CELLS_DIR` | `./data/cells` | cellz event logs |
| `OPENHUB_CELLS_STORAGE_DIR` | `./data/cells-storage` | cellz snapshots |
| `OPENHUB_CELLS_LEASE_TTL_SECS` | `60` | cellz lease TTL |

## Deploy

[`deploy/README.md`](deploy/README.md) — GCP VM, Docker, Cloudflare DNS for `openhun.run`.

## Development

```bash
cargo fmt --check
cargo clippy --all-targets -- -D warnings
cargo test
```
