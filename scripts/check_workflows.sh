#!/usr/bin/env bash
# 工作流文件语法与结构门禁。
#
# 为什么需要它：`deploy-*.yml` 是 tag / workflow_dispatch 触发的，PR 里永远不会执行，
# 因此文件内部的错误（YAML 缩进写坏、新 job 忘记写 `runs-on`、step 既无 `uses` 也无
# `run`、漏写 `on:` 触发器）只有在**真正发版时才暴露**——那时代价是生产部署失败。
# 改工作流就跑满全矩阵只能证明“仓库没被弄坏”，并不能验证工作流文件本身；本门禁补的
# 正是后者。
#
# 引擎：Ruby + psych（两者都是标准库；GitHub ubuntu-latest runner 自带 Ruby 3.2.3，
# macOS 自带 Ruby 2.6 + psych 3.x），因此无需安装任何东西、无网络依赖。
# YAML 语法校验用 `Psych.parse_file`（只解析 AST，不构造对象，避免 `on:` 被当作 YAML
# 1.1 布尔值等语义坑）；结构校验用 `Psych.safe_load_file(aliases: true)`。
#
# 校验项：
#   1. 所有 `.github/workflows/*.yml|*.yaml` 能被 YAML 解析（报错含行号）；
#   2. 顶层存在非空 `on:` 触发器；
#   3. `jobs` 非空，每个 job 必须恰好具备 `runs-on`（普通 job）或 `uses`（复用工作流）；
#   4. 普通 job 的 `steps` 非空，每个 step 恰好具备 `uses` 或 `run`。
#
# 用法：
#   scripts/check_workflows.sh                  # 校验 .github/workflows/ 下全部文件
#   scripts/check_workflows.sh path/to/wf.yml   # 校验指定文件（自测即用此路径）
#   scripts/check_workflows.sh --self-test      # 用内置夹具自证门禁确实会拦截
set -euo pipefail

repo_root="$(cd "$(dirname "$0")/.." && pwd)"
# 顶层变量而非 local：EXIT trap 在函数返回后才触发，引用 local 会在 set -u 下报
# "unbound variable" 并让退出码变成 1。
self_test_tmp=""

ruby_engine() {
  # -W0 抑制宿主环境（如 macOS 世界可写目录）的启动告警；本脚本自己的信息走 $stderr.puts，
  # 不受 $VERBOSE 影响，因此错误信息不会被吞掉。
  ruby -W0 - "$@" <<'RUBY'
require 'psych'

def bail(msg)
  $stderr.puts "\u2717 #{msg}"
  exit 1
end

# 只走 AST，不做任何对象构造（Psych.load 可被恶意 YAML 触发任意对象实例化，
# 而工作流文件在 fork PR 中由外部可控，因此绝不能反序列化）。
def scalar(node)
  node.is_a?(Psych::Nodes::Scalar) ? node.value : nil
end

def entries(node)
  return [] unless node.is_a?(Psych::Nodes::Mapping)
  node.children.each_slice(2).map { |k, v| [scalar(k), v] }
end

def fetch(node, key)
  entries(node).find { |k, _| k == key }&.last
end

def key?(node, key)
  entries(node).any? { |k, _| k == key }
end

def anchored?(node)
  node.is_a?(Psych::Nodes::Alias) # 使用锚点时结构不透明，跳过严格类型断言
end

def problems_for(path)
  errors = []

  begin
    root = Psych.parse_file(path) # 语法：仅解析 AST
  rescue Psych::SyntaxError => e
    return ["#{path}: YAML 语法错误（line #{e.line}, column #{e.column}）：#{e.problem}"]
  end
  return ["#{path}: 文件为空"] if root.nil?

  doc = root.root
  return ["#{path}: 顶层必须是映射（mapping）"] unless doc.is_a?(Psych::Nodes::Mapping)

  # YAML 1.1 会把裸 `on:` 解析成布尔标量 true，两种都接受。
  triggers = fetch(doc, 'on') || fetch(doc, 'true')
  if triggers.nil? || (triggers.is_a?(Psych::Nodes::Scalar) && triggers.value.empty?)
    errors << "#{path}: 缺少顶层 `on:` 触发器（工作流永远不会运行）"
  end

  jobs = fetch(doc, 'jobs')
  unless jobs.is_a?(Psych::Nodes::Mapping) && !jobs.children.empty?
    errors << "#{path}: `jobs` 缺失或为空"
    return errors
  end

  entries(jobs).each do |name, job|
    errors << "#{path}: 存在未命名的 job" if name.to_s.strip.empty?
    unless job.is_a?(Psych::Nodes::Mapping)
      errors << "#{path}: job `#{name}` 不是映射" unless anchored?(job)
      next
    end

    has_runs_on = key?(job, 'runs-on')
    has_uses = key?(job, 'uses')

    if has_runs_on && has_uses
      errors << "#{path}: job `#{name}` 同时声明了 `runs-on` 与 `uses`（Runner job 与复用工作流互斥）"
      next
    end
    if !has_runs_on && !has_uses
      errors << "#{path}: job `#{name}` 缺少 `runs-on`（或 `uses`）——GitHub 会拒绝执行该工作流"
      next
    end
    next if has_uses # 复用工作流调用：无 steps 属正常

    steps = fetch(job, 'steps')
    if anchored?(steps)
      next
    elsif !steps.is_a?(Psych::Nodes::Sequence) || steps.children.empty?
      errors << "#{path}: job `#{name}` 缺少非空 `steps`"
      next
    end

    steps.children.each_with_index do |step, idx|
      label = "job `#{name}` 第 #{idx + 1} 个 step"
      unless step.is_a?(Psych::Nodes::Mapping)
        errors << "#{path}: #{label} 不是映射" unless anchored?(step)
        next
      end

      uses_node = fetch(step, 'uses')
      run_node = fetch(step, 'run')
      if uses_node && run_node
        errors << "#{path}: #{label} 同时声明了 `uses` 与 `run`"
      elsif uses_node.nil? && run_node.nil?
        errors << "#{path}: #{label} 既无 `uses` 也无 `run`（占位 step 会执行失败）"
      elsif uses_node && scalar(uses_node).to_s.strip.empty?
        errors << "#{path}: #{label} 的 `uses` 为空"
      elsif run_node && scalar(run_node).to_s.strip.empty?
        errors << "#{path}: #{label} 的 `run` 为空"
      end
    end
  end

  errors
end

files = ARGV
bail('未收到任何工作流文件') if files.empty?

all_errors = files.flat_map { |f| problems_for(f) }
if all_errors.empty?
  puts "\u2713 工作流语法与结构检查通过：#{files.length} 个文件"
  exit 0
end

all_errors.each { |e| $stderr.puts "\u2717 #{e}" }
$stderr.puts "\u2717 工作流检查未通过：#{all_errors.length} 个问题（文件数 #{files.length}）"
exit 1
RUBY
}

collect_default_targets() {
  shopt -s nullglob
  local found=(".github/workflows/"*.yml ".github/workflows/"*.yaml)
  shopt -u nullglob
  printf '%s\n' "${found[@]}"
}

run_check() {
  local targets=()
  if [[ "$#" -gt 0 ]]; then
    targets=("$@")
  else
    cd "$repo_root"
    mapfile -t targets < <(collect_default_targets)
  fi
  if [[ "${#targets[@]}" -eq 0 ]]; then
    echo "✗ 未找到任何工作流文件" >&2
    return 1
  fi
  if ! command -v ruby >/dev/null 2>&1; then
    echo "✗ 未找到 ruby（本门禁依赖 Ruby 标准库 psych 解析 YAML；CI runner 与 macOS 均自带）" >&2
    return 1
  fi
  ruby_engine "${targets[@]}"
}

self_test() {
  local failures=0
  self_test_tmp="$(mktemp -d)"
  trap 'rm -rf "${self_test_tmp:-}"' EXIT
  local tmp="$self_test_tmp"

  expect() { # expect <期望 pass|fail> <名称> <文件>
    local want="$1" name="$2" file="$3" got
    if run_check "$file" >/dev/null 2>&1; then got=pass; else got=fail; fi
    if [[ "$got" == "$want" ]]; then
      echo "  ✓ ${name}（${got}）"
    else
      echo "  ✗ ${name}：期望 ${want}，实际 ${got}"
      failures=$((failures + 1))
    fi
  }

  cat > "$tmp/valid.yml" <<'YML'
name: valid
on: [push]
jobs:
  build:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v5
      - name: run
        run: echo ok
YML

  cat > "$tmp/bad-yaml.yml" <<'YML'
name: broken
on: [push
jobs:
  build:
    runs-on: ubuntu-latest
YML

  cat > "$tmp/no-runs-on.yml" <<'YML'
name: no runs-on
on: [push]
jobs:
  build:
    steps:
      - run: echo ok
YML

  cat > "$tmp/empty-step.yml" <<'YML'
name: empty step
on: [push]
jobs:
  build:
    runs-on: ubuntu-latest
    steps:
      - name: does nothing
YML

  cat > "$tmp/no-trigger.yml" <<'YML'
name: no trigger
jobs:
  build:
    runs-on: ubuntu-latest
    steps:
      - run: echo ok
YML

  cat > "$tmp/reusable.yml" <<'YML'
name: reusable call
on: [push]
jobs:
  call:
    uses: ./.github/workflows/valid.yml
YML

  echo "check_workflows 自测："
  expect pass "合法工作流" "$tmp/valid.yml"
  expect pass "复用工作流调用（无 steps 合法）" "$tmp/reusable.yml"
  expect fail "YAML 语法错误" "$tmp/bad-yaml.yml"
  expect fail "job 缺 runs-on" "$tmp/no-runs-on.yml"
  expect fail "step 缺 uses/run" "$tmp/empty-step.yml"
  expect fail "缺 on: 触发器" "$tmp/no-trigger.yml"

  # 真实仓库也必须通过（否则夹具之外的现实写法有误判）
  if run_check >/dev/null 2>&1; then
    echo "  ✓ 真实 .github/workflows/ 全部通过"
  else
    echo "  ✗ 真实 .github/workflows/ 未通过："
    run_check 2>&1 | sed 's/^/      /' || true
    failures=$((failures + 1))
  fi

  if [[ "$failures" -eq 0 ]]; then
    echo "✓ check_workflows：全部用例通过"
    return 0
  fi
  echo "✗ check_workflows：${failures} 个用例失败"
  return 1
}

case "${1:-}" in
  --self-test) self_test ;;
  *) run_check "$@" ;;
esac
