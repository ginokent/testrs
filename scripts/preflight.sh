#!/usr/bin/env bash
# scripts/preflight.sh — `mise run ci` / `mise run ci-publish-check-dangerously`
# の事前条件 (dirty check) を実行する。modified / staged の差分があれば fail。
#
# untracked は許容する (.gitignore に入っていない一時ファイルや新規 issue 文書
# 等を許容するため)。完全 clean (untracked 含む) を要求したいときは
# `git status --porcelain` の出力空判定に切り替えること。
set -euo pipefail

if ! git diff --quiet || ! git diff --cached --quiet; then
  cat >&2 <<'EOF'
error: uncommitted changes in tracked files.
       Commit or stash before running 'mise run ci' /
       'mise run ci-publish-check-dangerously'.
EOF
  git status --short >&2
  exit 1
fi
