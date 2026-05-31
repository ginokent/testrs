#!/usr/bin/env bash
# scripts/report-check-local.sh — ローカル task の結果を `gh api` で check run
# として PR HEAD (= 現在の git HEAD) に投影する。ci-publish から呼ばれる。
#
# 引数:
#   $1 = check 表示名 (CI と同名 = Branch protection の Required 値と一致)
#   $2 = 結果 (success / failure / cancelled / skipped / neutral)
#
# 前提:
#   - gh CLI が install + 認証済み (`gh auth login`)
#   - 現在の HEAD commit が remote に push 済み (= GitHub 側で参照可能)
#   ci-publish.sh の事前 check でこれらの前提は確認済み。
#
# ⚠️ 危険性 (CLAUDE.md / mise.toml の対応セクション参照):
# 本 script は **CI と同名の check 名** を投影するため、Branch protection
# の Required check を local 1 回で満たせる = CI を発火させずに merge 可能。
# GitHub Actions コスト削減目的のユーザー責任設計。ローカル環境差で本来 CI
# が落ちる check が local では緑になるリスクは実行者が負う。
set -euo pipefail

CHECK_NAME="$1"
CONCLUSION="$2"

HEAD_SHA=$(git rev-parse HEAD)
REPO=$(gh repo view --json nameWithOwner --jq .nameWithOwner)

# details_url: ローカル実行は GitHub Actions run URL を持たないので、commit
# ページ URL で代替する。将来コンソール出力を gist 等にアップロードする運用
# にしたければここに上書き可。
DETAILS_URL="https://github.com/${REPO}/commit/${HEAD_SHA}"

echo "Reporting check '${CHECK_NAME}' = ${CONCLUSION} to ${REPO} HEAD ${HEAD_SHA}"

# `gh api` の `-f` は文字列 field の POST。check-runs API は status=completed
# のとき conclusion 必須 (固定列挙値: success / failure / cancelled / skipped
# / neutral / timed_out / action_required)。
gh api "repos/${REPO}/check-runs" \
  --method POST \
  -f name="${CHECK_NAME}" \
  -f head_sha="${HEAD_SHA}" \
  -f status=completed \
  -f conclusion="${CONCLUSION}" \
  -f details_url="${DETAILS_URL}" \
  >/dev/null
