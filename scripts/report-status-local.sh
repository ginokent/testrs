#!/usr/bin/env bash
# scripts/report-status-local.sh — ローカル task の結果を `gh api` で commit
# status として PR HEAD (= 現在の git HEAD) に投影する。
# ci-publish-status-dangerously から呼ばれる。
#
# 引数:
#   $1 = context (Branch protection の Required 値と一致させる文字列)
#   $2 = 結果 (success / failure / cancelled / skipped)
#
# 前提:
#   - gh CLI が install + 認証済み (`gh auth login`)
#   - 現在の HEAD commit が remote に push 済み (= GitHub 側で参照可能)
#   ci-publish-status-dangerously.sh の事前 check でこれらの前提は確認済み。
#
# なぜ check-runs ではなく commit statuses を使うか:
# - check-runs API (`POST /repos/.../check-runs`) は **GitHub Apps 認証専用**
#   で、PAT (Classic / Fine-grained 問わず) では作成不可。GitHub Actions の
#   `GITHUB_TOKEN` は内部的に App の installation token なので CI 側からは
#   投影できるが、ローカルで `gh auth login` した PAT では「Resource not
#   accessible by personal access token (HTTP 403)」で拒否される。
# - 一方 commit statuses API (`POST /repos/.../statuses/{sha}`) は PAT の
#   `repo` scope (Fine-grained PAT では "Commit statuses: Read and write")
#   で書き込み可能。同じ Branch protection の Required check 設定に
#   commit status の context を指定できるため、check-runs と機能上等価。
#
# ⚠️ 危険性 (CONTRIBUTING.md 参照):
# 本 script は **CI と同名の context 名** で commit status を投影するため、
# Branch protection の Required check を local 1 回で満たせる = CI を発火
# させずに merge 可能。GitHub Actions コスト削減目的のユーザー責任設計。
# ローカル環境差で本来 CI が落ちる check が local では緑になるリスクは実行
# 者が負う。
set -euo pipefail

CONTEXT="$1"
JOB_STATUS="$2"

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
  if [[ "${REMOTE_URL}" =~ /([^/]+)/([^/]+)(\.git)?$ ]]; then
    REPO="${BASH_REMATCH[1]}/${BASH_REMATCH[2]%.git}"
  fi
fi
if [[ -z "${REPO}" ]]; then
  echo "error: could not determine GitHub repository (owner/name)." >&2
  echo "       Set the GH_REPO env (e.g. GH_REPO=owner/repo)." >&2
  exit 1
fi

# job.status → commit status state mapping。
# commit statuses API の state は {success, failure, error, pending} の 4 値。
# - success / failure はそのまま対応
# - cancelled / skipped / 未知の値は **安全側に倒して error** にする
#   (success にすると「実行されなかったのに緑」になり Branch protection を
#    誤って満たしうる)
case "${JOB_STATUS}" in
  success)   STATE=success ;;
  failure)   STATE=failure ;;
  cancelled) STATE=error ;;
  skipped)   STATE=error ;;
  *)         STATE=error ;;
esac

# description は GitHub PR の Status 表示に出る短文 (140 文字以内)。
DESCRIPTION="${STATE} (local)"

# target_url: ローカル実行は GitHub Actions run URL を持たないので、commit
# ページ URL で代替する。github.com ドメインで固定 (REPO 解決経路に関わらず
# 実際の commit ページは github.com/<owner>/<repo>/commit/<sha> なため)。
TARGET_URL="https://github.com/${REPO}/commit/${HEAD_SHA}"

echo "Reporting status '${CONTEXT}' = ${STATE} to ${REPO} HEAD ${HEAD_SHA}"

# 同じ commit / 同じ context の最新 status のみが GitHub UI に表示される
# (= 過去の status は上書きされる)。Branch protection は context 名で
# Required を判定するので、CI workflow の check 名と完全に揃える必要がある。
gh api "repos/${REPO}/statuses/${HEAD_SHA}" \
  --method POST \
  -f state="${STATE}" \
  -f context="${CONTEXT}" \
  -f description="${DESCRIPTION}" \
  -f target_url="${TARGET_URL}" \
  >/dev/null
