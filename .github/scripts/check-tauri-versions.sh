#!/usr/bin/env bash
# Tauri 版本一致性门控：npm 侧 @tauri-apps/* 与 Cargo.lock 的 tauri crate 必须同 major.minor。
# 用法: check-tauri-versions.sh <Cargo.lock> <gui/package-lock.json>
set -uo pipefail

lock="${1:?需要 Cargo.lock 路径}"
plock="${2:?需要 gui/package-lock.json 路径}"

crate_version() {
  awk -v want="$1" '
    /^\[\[package\]\]$/ { n=""; v="" }
    /^name = "/       { n=$0; sub(/^name = "/,"",n); sub(/"$/,"",n) }
    /^version = "/    { v=$0; sub(/^version = "/,"",v); sub(/"$/,"",v) }
    n==want && v!=""  { print v; exit }
  ' "$lock"
}

npm_version() {
  awk -v p="node_modules/@tauri-apps/$1" '
    { line=$0; sub(/^[[:space:]]+/,"",line) }
    line == "\""p"\": {" { f=1; next }
    f && /"version": "/ { v=$0; sub(/.*"version": "/,"",v); sub(/".*/,"",v); print v; exit }
  ' "$plock"
}

minor() { printf '%s' "${1%.*}"; }

fail=0
check() {
  local npm_pkg="$1" crate="$2" nv cv
  nv=$(npm_version "$npm_pkg")
  cv=$(crate_version "$crate")
  if [ -z "$nv" ] || [ -z "$cv" ]; then
    echo "::error::无法解析版本: @tauri-apps/$npm_pkg='$nv' / crate '$crate'='$cv'"
    fail=1
    return
  fi
  if [ "$(minor "$nv")" != "$(minor "$cv")" ]; then
    echo "::error::版本不一致 @tauri-apps/$npm_pkg ($nv) : $crate ($cv) —— major.minor 必须相同"
    fail=1
  else
    echo "ok  @tauri-apps/$npm_pkg ($nv) : $crate ($cv)"
  fi
}

check api                tauri
check plugin-dialog      tauri-plugin-dialog
check plugin-opener      tauri-plugin-opener
check plugin-store       tauri-plugin-store
check plugin-window-state tauri-plugin-window-state

exit $fail
