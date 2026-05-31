#!/usr/bin/env bash
# PreToolUse(Bash) hook: 規約外ブランチ上での git commit / git push をブロックする安全網。
#
# stdin には PreToolUse の JSON ペイロードが渡され、tool_input.command に実行
# 予定のコマンドが入る。settings.json 側で `if: "Bash(git *)"` により git
# コマンド実行時のみ起動される前提。commit / push 以外は素通しする。
#
# 規約外ブランチ上で commit / push しようとした場合は exit 2 でブロックし、
# stderr のメッセージがエージェントへフィードバックされる。
set -u

payload="$(cat)"

# git commit / git push 以外は対象外 (ペイロード JSON 内のコマンド文字列を素朴に検査)。
case "$payload" in
*"git commit"* | *"git push"*) ;;
*) exit 0 ;;
esac

repo_dir="${CLAUDE_PROJECT_DIR:-.}"
branch="$(git -C "$repo_dir" symbolic-ref --quiet --short HEAD 2>/dev/null || true)"

# detached HEAD などブランチ名が取れない場合は判定不能のため許可する。
[ -z "$branch" ] && exit 0

# 許可ブランチ: develop / main / Git Flow の feature プレフィックス。
case "$branch" in
develop | main | feature/add-* | feature/fix-* | feature/update-* | feature/breaking-change-*)
	exit 0
	;;
esac

# 規約外ブランチ -> commit / push をブロックし、リネームを促す。
cat >&2 <<EOF
Branch "$branch" violates the naming convention; refusing to commit or push.
Rename it first, for example:
  git branch -m "$branch" feature/add-<short-description>
Allowed: develop, main, feature/{add,fix,update,breaking-change}-*. See CLAUDE.md.
EOF
exit 2
