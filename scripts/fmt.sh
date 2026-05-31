#!/usr/bin/env bash
# scripts/fmt.sh — workspace 全体に対する rustfmt の check (整形は適用しない)。
# CI の fmt job (.github/workflows/ci.yml) と同じ引数を用いる。
set -euo pipefail
echo "==> cargo fmt --all -- --check"
cargo fmt --all -- --check
