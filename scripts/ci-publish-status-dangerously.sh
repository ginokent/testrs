#!/usr/bin/env bash
# scripts/ci-publish-status-dangerously.sh — push 済みの clean HEAD に対し、
# ci 相当の check を実行して結果を **CI と同名の context** で commit status
# として PR HEAD に投影する。
#
# ⚠️ 名前に `-dangerously` を含めているのは、本 task が GitHub Actions の CI
# signal を local が override できる構造であることを CLI レベルで明示する
# 意図 (npm `--force` 系の命名意図と同じ)。利用前に CONTRIBUTING.md の
# 「ci-publish-status-dangerously — コスト削減目的のユーザー責任設計」
# セクションを必ず読むこと。
#
# 設計判断 (詳細は CONTRIBUTING.md 参照):
# - context 名は **CI workflow の check 名と完全同名** で投げる。Branch
#   protection の Required check 設定値を local 1 回で満たせる = CI を発火
#   させずに merge が可能になる。GitHub Actions コスト削減が一義的目的。
# - check-runs API ではなく commit statuses API を使う理由: check-runs は
#   GitHub Apps 認証専用で PAT (`gh auth login` で得るもの) では作成不可。
#   commit statuses は PAT の `repo` scope で書き込み可能で、Branch
#   protection の Required としても check-runs と等価に扱える。詳細は
#   scripts/report-status-local.sh の冒頭コメント参照。
# - ⚠️ 裏返しのリスク: ローカル環境差 (OS / toolchain / cache / 環境変数)
#   で CI が落ちる check が local では緑になりうる。また gh api 直叩きで
#   任意 state を post 可能なので 「PR HEAD に緑」 = 「PR コードが緑」を
#   保証しない。第三者 contributor 環境では `local-ci: *` 等の prefix 戦略
#   への切替を検討すること。
# - 各 task は順次実行 (並列 status 投影で gh api の order 保証が崩れる
#   のを避ける目的)。`mise run ci` 側は並列実行で高速化する。
# - 各 task 単独の失敗で全体を止めず、5 task すべて実行した上で集約 status
#   を最後に決める (= 1 つ落ちた時に他の落ち位置も同時に分かる)。
set -euo pipefail

./scripts/preflight.sh

# gh CLI 存在チェック。
if ! command -v gh >/dev/null 2>&1; then
  cat >&2 <<'EOF'
error: gh CLI is not installed.
       ci-publish-status-dangerously requires `gh` to post commit statuses
       to GitHub.
       Install:        https://cli.github.com/
       Authenticate:   gh auth login
EOF
  exit 1
fi

# push 済み check: HEAD が remote 上に存在するか。
# git branch --remotes --contains <sha> が空でないことで判定する。
HEAD_SHA=$(git rev-parse HEAD)
if [[ -z "$(git branch --remotes --contains "${HEAD_SHA}" 2>/dev/null)" ]]; then
  cat >&2 <<EOF
error: HEAD (${HEAD_SHA}) is not present on any remote.
       Push the branch before running 'mise run ci-publish-status-dangerously'.
       (GitHub commit statuses are attached to commits known to the server;
       unpushed commits will trigger a 404 from the statuses API.)
EOF
  exit 1
fi

overall_status=0

run_and_report() {
  local context="$1"
  local script_path="$2"
  if "${script_path}"; then
    ./scripts/report-status-local.sh "${context}" success
  else
    overall_status=1
    ./scripts/report-status-local.sh "${context}" failure
  fi
}

# context は CI workflow (.github/workflows/ci.yml) の各 job `name:` と完全
# 一致させる。Branch protection の Required check 設定と同期。
# MSRV は CI 専用 job のためここからは除外する (rust-toolchain.toml の pin
# とは別 toolchain が必要で、ローカルで毎回回すコストに見合わないため)。
run_and_report "cargo fmt --check"                          "./scripts/fmt.sh"
run_and_report "cargo clippy -D warnings"                   "./scripts/clippy.sh"
run_and_report "cargo doc"                                  "./scripts/doc.sh"
# ローカル OS を問わず、Branch protection の Required check (Linux) を
# 満たすことを意図して `(ubuntu-latest)` 固定で投影する。macOS / Windows
# 上で実行しても context 名は ubuntu-latest になることに留意。
run_and_report "cargo test (ubuntu-latest)"                 "./scripts/test.sh"
run_and_report "cargo deny (license/bans/sources/advisories)" "./scripts/deny.sh"

if [[ "${overall_status}" -ne 0 ]]; then
  echo "error: one or more checks failed (see commit statuses on the PR HEAD)" >&2
fi

exit "${overall_status}"
