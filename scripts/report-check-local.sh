#!/usr/bin/env bash
# scripts/report-check-local.sh — ローカル task の結果を `gh api` で check run
# として PR HEAD (= 現在の git HEAD) に投影する。
# ci-publish-check-dangerously から呼ばれる。
#
# 引数:
#   $1 = check 表示名 (CI と同名 = Branch protection の Required 値と一致)
#   $2 = 結果 (success / failure / cancelled / skipped / neutral)
#
# 前提:
#   - gh CLI が install + 認証済み (`gh auth login`)
#   - 現在の HEAD commit が remote に push 済み (= GitHub 側で参照可能)
#   ci-publish-check-dangerously.sh の事前 check でこれらの前提は確認済み。
#
# ⚠️ 危険性 (CONTRIBUTING.md 参照):
# 本 script は **CI と同名の check 名** を投影するため、Branch protection
# の Required check を local 1 回で満たせる = CI を発火させずに merge 可能。
# GitHub Actions コスト削減目的のユーザー責任設計。ローカル環境差で本来 CI
# が落ちる check が local では緑になるリスクは実行者が負う。
set -euo pipefail

CHECK_NAME="$1"
CONCLUSION="$2"

HEAD_SHA=$(git rev-parse HEAD)

# `REPO` の解決順位:
# 1. `GH_REPO` env (明示指定、最優先)
# 2. `gh repo view` (gh が GitHub.com を remote として認識できる通常環境)
# 3. git remote URL の末尾 `owner/repo` 部分から抽出 (Claude Code on the web 等
#    のように remote URL が内部プロキシ (`http://127.0.0.1:NNNN/git/owner/repo`)
#    を指していて gh が GitHub host を認識できない場合の fallback)
REPO="${GH_REPO:-}"
if [[ -z "${REPO}" ]]; then
  REPO=$(gh repo view --json nameWithOwner --jq .nameWithOwner 2>/dev/null || true)
fi
if [[ -z "${REPO}" ]]; then
  REMOTE_URL=$(git remote get-url origin 2>/dev/null || true)
  # 末尾 `owner/repo[.git]` を抽出。`.git` は任意。
  if [[ "${REMOTE_URL}" =~ /([^/]+)/([^/]+)(\.git)?$ ]]; then
    REPO="${BASH_REMATCH[1]}/${BASH_REMATCH[2]%.git}"
  fi
fi
if [[ -z "${REPO}" ]]; then
  echo "error: could not determine GitHub repository (owner/name)." >&2
  echo "       Set the GH_REPO env (e.g. GH_REPO=owner/repo)." >&2
  exit 1
fi

# details_url: ローカル実行は GitHub Actions run URL を持たないので、commit
# ページ URL で代替する。github.com ドメインで固定 (REPO 解決経路に関わらず
# 実際の commit ページは github.com/<owner>/<repo>/commit/<sha> なため)。
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
