#!/usr/bin/env bash
# redb-peek 构建脚本（macOS / Linux）。
#
# 用法:
#   ./build.sh
#
# 产物: target/release/redb-peek（Windows 上为 .exe，用 cargo 直接构建即可）。
set -euo pipefail

here="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
cd "$here"

if ! command -v cargo >/dev/null 2>&1; then
  echo "找不到 cargo。先安装 Rust 工具链：https://rustup.rs" >&2
  exit 1
fi

cargo build --release

bin="$here/target/release/redb-peek"
if [ ! -x "$bin" ]; then
  echo "构建结束但产物不存在: $bin" >&2
  exit 1
fi

echo "OK: $bin"
