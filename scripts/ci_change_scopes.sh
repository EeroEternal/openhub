#!/usr/bin/env bash
# CI change-scope classifier.
#
# Input: stdin, one repo-relative path per line (git diff filenames).
# Output (stdout, one key=value per line; parsed by scripts/ci_local.sh):
#   rust=<true|false>   affects Rust compile / unit / integration tests
#   admin=<true|false>  affects Admin frontend build and lint
#
# Why this script exists: scope rules scattered as YAML greps cannot be tested.
# Rules live here with `--self-test` (harness-gates in ci_local.sh runs it).
#
# Scope map (each row has a self-test):
#   rust  : src/ tests/ migrations/, root manifests
#           (Cargo.toml / Cargo.lock / rust-toolchain.toml / rustfmt.toml /
#            clippy.toml / build.rs)
#   admin : admin/
#   everything else (docs/ .agents/ *.md scripts/ deploy workflows): both false;
#          only harness-gates run (check_ui_stack / check_admin_nav /
#          check_workflows).
#
# GitHub Actions CI (ci.yml) has been removed. The heavy matrix runs locally
# via scripts/ci_local.sh. deploy.yml stays on GitHub (secrets + SSH) and is
# syntax-checked by check_workflows.sh, not by the rust/admin matrix.
set -euo pipefail

rust_pattern='^(src/|tests/|migrations/)|^(Cargo\.toml|Cargo\.lock|rust-toolchain\.toml|rustfmt\.toml|clippy\.toml|build\.rs)$'
admin_pattern='^admin/'

classify() {
  local files rust=false admin=false
  files="$(cat)"
  if grep -qE "$rust_pattern" <<<"$files"; then rust=true; fi
  if grep -qE "$admin_pattern" <<<"$files"; then admin=true; fi
  printf 'rust=%s\nadmin=%s\n' "$rust" "$admin"
}

self_test() {
  local failures=0
  check() {
    local name="$1" files="$2" expected="$3" actual
    actual="$(printf '%s\n' "$files" | classify | tr '\n' ' ' | sed 's/ $//')"
    if [[ "$actual" == "$expected" ]]; then
      echo "  ✓ ${name}"
    else
      echo "  ✗ ${name}: expected [${expected}], got [${actual}]"
      failures=$((failures + 1))
    fi
  }

  echo "ci_change_scopes self-test:"
  check "scripts only" "scripts/check_ui_stack.sh" "rust=false admin=false"
  check "docs only" "docs/design.md
README.md" "rust=false admin=false"
  check "skill / metacode" ".agents/skills/release/SKILL.md" "rust=false admin=false"
  check "deploy workflow" ".github/workflows/deploy.yml" "rust=false admin=false"
  check "backend" "src/store.rs" "rust=true admin=false"
  check "cli bin" "src/bin/oh.rs" "rust=true admin=false"
  check "integration tests" "tests/server_test.rs" "rust=true admin=false"
  check "migration" "migrations/001_init.sql" "rust=true admin=false"
  check "Cargo.toml" "Cargo.toml" "rust=true admin=false"
  check "Admin page" "admin/src/App.tsx" "rust=false admin=true"
  check "Admin lockfile" "admin/package-lock.json" "rust=false admin=true"
  check "mixed backend + docs" "src/server.rs
docs/product.md" "rust=true admin=false"
  check "mixed backend + admin" "src/auth.rs
admin/src/pages/login.tsx" "rust=true admin=true"
  check "empty input" "" "rust=false admin=false"

  if [[ "$failures" -eq 0 ]]; then
    echo "✓ ci_change_scopes: all cases passed"
    return 0
  fi
  echo "✗ ci_change_scopes: ${failures} case(s) failed"
  return 1
}

case "${1:-}" in
  --self-test) self_test ;;
  "" ) classify ;;
  * ) echo "usage: git diff file list | $0   (or $0 --self-test)" >&2; exit 2 ;;
esac
