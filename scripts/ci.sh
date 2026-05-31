#!/usr/bin/env bash
# scripts/ci.sh — 開発 loop 用の集約 task。dirty check の上で fmt / clippy /
# doc / test / deny を mise の並列実行で走らせる。
#
# check run は投げない (= ci-publish-check-dangerously の責務)。ローカルで「commit 前に手早く
# check したい」という用途向け。
set -euo pipefail

./scripts/preflight.sh

# mise の --jobs N で並列実行。各 task は scripts/<name>.sh を呼ぶ wrapper
# (mise.toml 参照)。失敗があれば mise が非ゼロ exit で返す。
mise run --jobs 5 fmt clippy doc test deny
