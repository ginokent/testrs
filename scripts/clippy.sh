#!/usr/bin/env bash
# scripts/clippy.sh — workspace 全 target に対し clippy を warning = error で実行。
# CI の clippy job (.github/workflows/ci.yml) と同じ引数。
set -euo pipefail
echo "==> cargo clippy --workspace --all-targets -- -D warnings"
cargo clippy --workspace --all-targets -- -D warnings
