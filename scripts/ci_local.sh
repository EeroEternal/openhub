#!/usr/bin/env bash
# Local CI runner — replays the removed GitHub Actions pipeline
# (former .github/workflows/ci.yml) on the developer machine.
#
# Stage map (former CI jobs):
#   harness-gates   mechanical gates + --self-test (always, seconds)
#   rust            cargo fmt / clippy / test --workspace
#   admin           tsc / lint / production build
#
# Usage:
#   scripts/ci_local.sh            # change-aware vs origin/main
#   scripts/ci_local.sh --full     # full matrix, no change filter
#   scripts/ci_local.sh --base main
#   scripts/ci_local.sh --jobs 4   # parallel rust+admin (default min(nproc,4))
#
# Deps: Rust toolchain (rustfmt + clippy), Node 22+ (admin). SQLite in-process;
# no Postgres / Redis / Playwright.
set -euo pipefail

repo_root="$(cd "$(dirname "$0")/.." && pwd)"
cd "$repo_root"

BASE_REF="${BASE_REF:-origin/main}"
FULL=0
JOBS="${JOBS:-}"

usage() { sed -n '2,16p' "$0" | sed 's/^# \{0,1\}//'; }

while [[ $# -gt 0 ]]; do
  case "$1" in
    --base) BASE_REF="$2"; shift 2 ;;
    --base=*) BASE_REF="${1#--base=}"; shift ;;
    --full) FULL=1; shift ;;
    --jobs|-j) JOBS="$2"; shift 2 ;;
    --help|-h) usage; exit 0 ;;
    *) echo "unknown arg: $1" >&2; usage >&2; exit 2 ;;
  esac
done

if [[ -z "$JOBS" ]]; then
  JOBS="$(nproc)"
  (( JOBS > 4 )) && JOBS=4
fi

logdir="$repo_root/artifacts/ci-local"
mkdir -p "$logdir"
rm -f "$logdir"/*.log

say()  { printf '\n\033[1;36m== %s ==\033[0m\n' "$*"; }
ok()   { printf '\033[32m✓ %s\033[0m\n' "$*"; }
fail() { printf '\033[31m✗ %s\033[0m\n' "$*"; }
die()  { fail "$*"; exit 1; }

preflight() { # <rust> <admin>
  local rust="$1" admin="$2" missing=0
  if [[ "$rust" == true ]] && ! command -v cargo >/dev/null 2>&1; then
    fail "cargo/rustc not found — install Rust (rustup) first"
    missing=1
  fi
  if [[ "$admin" == true ]] && ! command -v node >/dev/null 2>&1; then
    fail "node not found — need Node 22+ for admin/"
    missing=1
  fi
  return "$missing"
}

# ---------------------------------------------------------------- change scopes
detect_scopes() {
  if [[ "$FULL" == 1 ]]; then
    printf 'rust=true\nadmin=true\n'
    return
  fi
  if ! git rev-parse --verify -q "${BASE_REF}^{commit}" >/dev/null 2>&1; then
    echo "⚠ cannot resolve baseline '$BASE_REF' (fetch first?); falling back to full matrix" >&2
    printf 'rust=true\nadmin=true\n'
    return
  fi
  local committed uncommitted files
  committed="$(git diff --name-only "$BASE_REF" HEAD 2>/dev/null || true)"
  uncommitted="$(git status --porcelain --untracked-files=normal | awk '{ $1=""; sub(/^ /, ""); print }' || true)"
  files="$(printf '%s\n%s\n' "$committed" "$uncommitted" | sed '/^$/d' | LC_ALL=C sort -u)"
  if [[ -z "$files" ]]; then
    echo "ℹ no diff vs $BASE_REF (clean tree); harness-gates only" >&2
    printf 'rust=false\nadmin=false\n'
    return
  fi
  printf '%s\n' "$files" | scripts/ci_change_scopes.sh
}

# ---------------------------------------------------------------- gates
job_harness_gates() {
  bash scripts/check_ui_stack.sh
  bash scripts/check_admin_nav.sh
  bash scripts/check_workflows.sh
  bash scripts/check_workflows.sh --self-test
  bash scripts/ci_change_scopes.sh --self-test
}

job_rust() {
  cargo fmt --check
  cargo clippy --all-targets -- -D warnings
  cargo test --workspace
}

npm_ci_if_needed() {
  if [[ -d node_modules ]] && [[ ! package-lock.json -nt node_modules ]]; then
    echo "    node_modules is current, skip npm ci"
  else
    npm ci
  fi
}

job_admin() {
  (
    cd admin
    npm_ci_if_needed
    npx tsc -b --noEmit
    npm run lint
    npm run build
  )
}

# ---------------------------------------------------------------- pool
declare -a POOL_PIDS=() POOL_NAMES=() POOL_ACTIVE=()

spawn() { # <name>  (calls job_<name>)
  local name="$1"
  while (( ${#POOL_ACTIVE[@]} >= JOBS )); do
    local -a alive=()
    local p
    for p in "${POOL_ACTIVE[@]}"; do
      kill -0 "$p" 2>/dev/null && alive+=("$p")
    done
    POOL_ACTIVE=("${alive[@]}")
    (( ${#POOL_ACTIVE[@]} >= JOBS )) && sleep 0.3
  done
  "job_$name" >"$logdir/$name.log" 2>&1 &
  POOL_PIDS+=("$!")
  POOL_NAMES+=("$name")
  POOL_ACTIVE+=("$!")
  echo "▶ scheduled $name"
}

reap() {
  local i rc=0
  for i in "${!POOL_PIDS[@]}"; do
    if wait "${POOL_PIDS[$i]}"; then
      ok "${POOL_NAMES[$i]}"
    else
      fail "${POOL_NAMES[$i]} — log: $logdir/${POOL_NAMES[$i]}.log"
      tail -40 "$logdir/${POOL_NAMES[$i]}.log" >&2 || true
      rc=1
    fi
  done
  POOL_PIDS=(); POOL_NAMES=(); POOL_ACTIVE=()
  return "$rc"
}

# ---------------------------------------------------------------- main
main() {
  local out rust admin
  say "harness-gates (always)"
  if ! job_harness_gates; then
    die "harness-gates failed"
  fi

  say "change scopes (baseline $BASE_REF)"
  out="$(detect_scopes)"
  rust="$(sed -n 's/^rust=//p' <<<"$out")"
  admin="$(sed -n 's/^admin=//p' <<<"$out")"
  echo "  rust=$rust admin=$admin"

  preflight "$rust" "$admin" || die "missing toolchain"

  say "Rust / Admin matrix (concurrency $JOBS)"
  if [[ "$rust" == true ]]; then
    spawn rust
  fi
  if [[ "$admin" == true ]]; then
    spawn admin
  fi
  if (( ${#POOL_PIDS[@]} > 0 )); then
    reap || die "Rust/Admin matrix failed"
  else
    echo "ℹ no rust/admin changes, skip matrix"
  fi

  say "all green ✅"
}

main
