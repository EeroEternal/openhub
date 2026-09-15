---
name: pre-push-local-gates
description: Push 前必须在本地跑满与 CI 等效的门禁(Rust fmt/clippy/tests、admin tsc/lint/build),禁止把 GitHub 当本地沙盒。Use before every git push touching src/, tests/, or admin/.
---

# Pre-push local gates（推送前本地门禁）

## 核心痛点与禁止项 (Symptom / Misjudgment)
严禁把远端当本地沙盒：推送后才发现 lint 报错、rustfmt 未对齐、编译报警、测试失败，
形成「推 → 挂 → 本地修 → 再推」的低效循环——不仅污染提交历史，还可能把未验证
的 `main` 直接部署出去（`deploy.yml` 在 push 到 main 时触发）。

GitHub Actions 测试矩阵已删除；权威入口是 `scripts/ci_local.sh`（见 skill
[`ci-local-runner`](../ci-local-runner/SKILL.md)）。

## 本地门禁执行标准 (Local Gate Checklist)
在执行 `git push` 或提 PR 之前，以下必须**全部在本地通过**：

```bash
# 一键（变更感知；与 origin/main 无 diff 时只跑 harness-gates）
scripts/ci_local.sh

# 或全量
scripts/ci_local.sh --full
```

等价拆开：

```bash
cargo fmt --check
cargo clippy --all-targets -- -D warnings
cargo test --workspace
bash scripts/check_ui_stack.sh
bash scripts/check_admin_nav.sh
bash scripts/check_workflows.sh
(cd admin && npx tsc -b --noEmit && npm run lint && npm run build)
```

UI 规范改动需确保符合 `docs/design.md`；发版与打 Tag 前，转入 skill
[`release`](../release/SKILL.md) 执行完整发版流程（三查 + 人工批准硬停）。

## 适用范围与纪律 (Scope & Discipline)
- 开发过程中的中间 commit 允许临时不跑全量，但 **push 前最后一次提交必须全绿**。
- 门禁挂了：禁止盲猜盲改，必须在本地先复现该失败的等价命令，确认修复通过后再推送。
