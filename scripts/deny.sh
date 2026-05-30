#!/usr/bin/env bash
# scripts/deny.sh — cargo-deny で license / bans / sources / advisories を
# 一括検証する。設定は repo root の deny.toml を参照。
# 事前に cargo-deny の install が必要 (mise 経由なら `mise install` で入る)。
set -euo pipefail
if ! command -v cargo-deny >/dev/null 2>&1; then
  cat >&2 <<'EOF'
error: cargo-deny is not installed.
       Install it via one of the following:
         mise install                      # recommended (uses mise.toml)
         cargo install --locked cargo-deny
EOF
  exit 1
fi
echo "==> cargo deny check all"
cargo deny check all
