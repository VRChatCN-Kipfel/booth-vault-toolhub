#!/usr/bin/env bash
# Tauri 版本一致性门控：npm 侧 @tauri-apps/* 与 Cargo.lock 的对应 crate 必须同 major.minor。
#
# 判据与 `tauri build` 一致（只比 major.minor），但两侧取包方式不同，差异已尽量收窄：
#   - tauri 用 known_plugins() 的静态列表逐个比对；
#   - 本脚本从 package-lock 里**实际安装**的 @tauri-apps/* 反推 crate 名，
#     故新增插件时无需改脚本（硬编码只会漏检，动态推导不会）。
# 映射：@tauri-apps/api -> tauri；@tauri-apps/plugin-X -> tauri-plugin-X。
# 跳过：@tauri-apps/cli 及其平台子包 cli-*（devDependency，tauri 不校验）。
#
# 用法: check-tauri-versions.sh <Cargo.lock> <gui/package-lock.json>
set -uo pipefail

lock="${1:?需要 Cargo.lock 路径}"
plock="${2:?需要 gui/package-lock.json 路径}"

# 打印某 crate 在 Cargo.lock 里的版本（可能多行）。
# Cargo.lock 允许同一 crate 存在多个版本；此时无法判断 tauri 会取哪一个
# （它改读 Cargo.toml 的依赖要求，解析失败就跳过该包），故由调用方显式报错，不静默取第一个。
crate_versions() {
  awk -v want="$1" '
    /^\[\[package\]\]$/ { n=""; v="" }
    /^name = "/       { n=$0; sub(/^name = "/,"",n); sub(/"$/,"",n) }
    /^version = "/    { v=$0; sub(/^version = "/,"",v); sub(/"$/,"",v)
                        if (n==want) { print v; n="" } }
  ' "$lock"
}

# 打印某 npm 包在 package-lock 里的实际解析版本。
npm_version() {
  awk -v p="node_modules/@tauri-apps/$1" '
    { line=$0; sub(/^[[:space:]]+/,"",line) }
    line == "\""p"\": {" { f=1; next }
    f && /"version": "/ { v=$0; sub(/.*"version": "/,"",v); sub(/".*/,"",v); print v; exit }
  ' "$plock"
}

# 列出 package-lock 里实际安装的 @tauri-apps/* 子包名。
# 只取一级：排除 `node_modules/@tauri-apps/x/node_modules/@tauri-apps/y` 这类嵌套项。
npm_packages() {
  awk '
    { line=$0; sub(/^[[:space:]]+/,"",line) }
    line ~ /^"node_modules\/@tauri-apps\/[^"\/]+": \{$/ {
      sub(/^"node_modules\/@tauri-apps\//,"",line)
      sub(/": \{$/,"",line)
      print line
    }
  ' "$plock"
}

# semver 的 major.minor。先剥构建元数据（+…），再剥预发布段（-…），最后取前两段。
# 不能直接 ${v%.*}：2.12.6+build.1 会被截成 2.12.6+build，2.12.0-beta.1 会被截成 2.12.0-beta。
major_minor() {
  local v="${1%%+*}"
  v="${v%%-*}"
  case "$v" in
    *.*.*) printf '%s' "${v%.*}" ;;
    *)     printf '%s' "$v" ;;
  esac
}

fail=0
checked=0
skipped=0

for pkg in $(npm_packages | sort -u); do
  case "$pkg" in
    cli|cli-*)
      # devDependency 及其平台子包：tauri 不参与校验
      skipped=$((skipped + 1))
      continue
      ;;
    api)
      crate="tauri"
      ;;
    plugin-*)
      crate="tauri-plugin-${pkg#plugin-}"
      ;;
    *)
      skipped=$((skipped + 1))
      continue
      ;;
  esac

  nv=$(npm_version "$pkg")
  if [ -z "$nv" ]; then
    echo "::error::package-lock 里找不到 @tauri-apps/$pkg 的解析版本"
    fail=1
    continue
  fi

  cvs=$(crate_versions "$crate")
  if [ -z "$cvs" ]; then
    # npm 侧有插件、Rust 侧没有对应 crate：tauri 不会比对这一对，这里同样跳过，仅提示。
    echo "skip @tauri-apps/$pkg（Cargo.lock 无对应 crate $crate）"
    skipped=$((skipped + 1))
    continue
  fi

  ccount=$(printf '%s\n' "$cvs" | wc -l | tr -d ' ')
  if [ "$ccount" -gt 1 ]; then
    echo "::error::Cargo.lock 里 $crate 存在多个版本（$(printf '%s' "$cvs" | tr '\n' ' ')），无法判断 tauri 取哪一个；请先收敛到单版本"
    fail=1
    continue
  fi

  nm=$(major_minor "$nv")
  cm=$(major_minor "$cvs")
  if [ "$nm" != "$cm" ]; then
    echo "::error::版本不一致 @tauri-apps/$pkg ($nv) : $crate ($cvs) —— major.minor 必须相同（$nm != $cm）"
    fail=1
  else
    echo "ok  @tauri-apps/$pkg ($nv) : $crate ($cvs)"
  fi
  checked=$((checked + 1))
done

echo "---"
echo "已核对 $checked 对，跳过 $skipped 项"

if [ "$checked" -eq 0 ]; then
  echo "::error::没有核对到任何一对，说明解析链路失效（而非版本一致）"
  exit 1
fi

exit $fail
