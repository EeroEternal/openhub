---
name: ci-local-runner
description: Run the full CI pipeline locally (scripts/ci_local.sh) instead of GitHub Actions. Covers change-scope sharding (rust/admin) and the harness gates. Use when configuring, debugging, or running local CI for this Rust/Admin stack.
---

# 本地 CI runner（原 GitHub Actions CI 已移除）

`ci.yml` 已删除，测试流水线全部迁到本机，由
[`scripts/ci_local.sh`](../../../scripts/ci_local.sh) 复刻原矩阵；部署工作流
[`deploy.yml`](../../../.github/workflows/deploy.yml) 仍在 GitHub（需要 secrets + SSH）。

OpenHub 用进程内 SQLite，无 Postgres / Redis / Playwright。不要从 xrouter 抄 `ci_db.sh`、e2e 分片或视觉基线。

## 入口

```bash
scripts/ci_local.sh            # 变更感知：与 origin/main 的 diff 决定跑 rust / admin
scripts/ci_local.sh --full     # 全量矩阵
scripts/ci_local.sh --jobs 4   # rust 与 admin 的并发上限（默认 min(nproc,4)）
```

## 变更感知（与原 CI 对齐后的本地版）

- 分类规则集中在 [`scripts/ci_change_scopes.sh`](../../../scripts/ci_change_scopes.sh)
  （单一事实源，`--self-test` 由 `ci_local.sh` 的 harness-gates 执行）：`rust` / `admin`。
- 本地基线默认 `origin/main`（`--base` 可换）；工作树干净（与基线无 diff）时只跑
  harness-gates，不跑重型矩阵。
- harness-gates（始终跑）：`check_ui_stack.sh`、`check_admin_nav.sh`、
  `check_workflows.sh`（含 `--self-test`）、`ci_change_scopes.sh --self-test`。

## 本地并发（无 GitHub 并发取消）

GitHub 的 `concurrency.cancel-in-progress`、`Swatinem/rust-cache` 与 10GB 缓存逐出问题
**不再存在**。本地替代：

- `--jobs N`：rust 与 admin 两路的并发上限。本地 `cargo` 并发过多只会争抢编译锁，故上限 4。
- 编译缓存靠本地 `target/`（增量），无需 `CARGO_INCREMENTAL` 调优。

## 检查清单

- [ ] 推送前跑本地门禁：见 skill [`pre-push-local-gates`](../pre-push-local-gates/SKILL.md)；
  一键全跑用 `scripts/ci_local.sh --full`。
- [ ] `scripts/ci_change_scopes.sh --self-test` 与 `scripts/check_workflows.sh --self-test` 绿
      （`ci_local.sh` 的 harness-gates 会自动跑）。
- [ ] 不要把测试矩阵搬回 `.github/workflows/`；deploy 工作流改动只靠 `check_workflows.sh`。
