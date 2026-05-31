# CLAUDE.md

[Claude Code](https://claude.ai/code) およびその他のコーディングエージェントへの
本リポジトリ固有の指示。global な指示 (`~/.claude/CLAUDE.md`) を上書きするもの
ではなく補足する。

## ローカル開発フロー (mise + scripts)

本リポジトリは [mise](https://mise.jdx.dev/) のタスクランナーで CI 相当の
check をローカル並列実行する運用。CI 側 (GitHub Actions) はコスト最適化のため
PR コメント (`!run ci` / `!run ci matrix`) で初めて発火する設計なので、PR を
投げる前のセルフチェックは **ローカルで完結** させること。

### Setup (初回のみ)

```bash
# mise 本体: https://mise.jdx.dev/getting-started.html に従って install
mise install        # mise.toml の [tools] (cargo-deny 等) を一括 install

# ci-publish を使うなら gh CLI も別途入れて認証する:
#   https://cli.github.com/
#   gh auth login
```

### 日常使用

```bash
mise run ci         # 開発 loop 用: dirty check + fmt / clippy / doc / test / deny を並列実行 (check run は投げない)

# 個別:
mise run fmt
mise run clippy
mise run doc
mise run test
mise run deny

# commit / push 後の正式署名:
mise run ci-publish
```

各 task は `scripts/*.sh` を呼ぶ単純な bash wrapper。mise を入れていない
環境では `./scripts/<name>.sh` を直接実行しても同じ結果を得られる。

`mise run ci` は実行前に dirty check (working tree に modified / staged の差分が
ある場合は fail) を行う。untracked file は許容するため、`.gitignore` に含まれ
ない一時ファイルが残っていても通る。

### `mise run ci-publish` — ⚠️ コスト削減目的のユーザー責任設計

`mise run ci` 相当を実行した上で、**各 task の結果を CI と同名の check run と
して PR HEAD に投影する**。`gh api repos/.../check-runs` を使うため `gh` CLI の
install + 認証 (`gh auth login`) が前提。

事前条件 (満たさなければ exit 1):

- working tree が clean (modified / staged の差分なし、untracked は許容)
- HEAD commit が remote に push 済み (check 投影先 SHA が GitHub 側で参照可能)
- `gh` CLI が install + 認証済み

#### ⚠️ 危険性 (理解した上で使用すること)

本 task は **CI と完全同一の check 名** (`cargo fmt --check` / `cargo clippy
-D warnings` / `cargo doc` / `cargo test (ubuntu-latest)` / `cargo deny
(license/bans/sources/advisories)`) で check run を投影する。これにより:

- Branch protection の Required status checks を **ローカル 1 回で満たせる**
  → CI を発火させずに merge 可能になる
- GitHub Actions の CI コストを 0 にできる (コスト削減目的)

**裏返しのリスク**:

- ローカル環境の差異 (OS / toolchain / cache 状態 / 環境変数) で **CI なら
  落ちる check が local では緑になりうる**
- ローカルで意図的に修正を加えずに緑判定だけ投げることも技術的に可能 (gh
  api を直叩きすれば任意の conclusion を post できる)
- 「PR HEAD に紐づいた緑」 = 「実際に PR コードが緑」を保証しない

→ 信頼性の責任は実行者が負う前提。第三者から検証可能な CI を捨ててもなお
コスト削減を優先する判断として、本リポジトリでは ci-publish 経路を許容して
いる。**他の co-maintainer / 外部 contributor が居る環境では、CI と区別可能
な check 名 (`local-ci: *` 等の prefix) に切り替えるか、ci-publish を撤去
することを検討すること**。

### local / CI 対応

| local             | 実コマンド                                                              | CI job          |
|-------------------|-------------------------------------------------------------------------|-----------------|
| `mise run fmt`    | `cargo fmt --all -- --check`                                            | `fmt`           |
| `mise run clippy` | `cargo clippy --workspace --all-targets -- -D warnings`                 | `clippy`        |
| `mise run doc`    | `RUSTDOCFLAGS=-D warnings cargo doc --workspace --no-deps`              | `doc`           |
| `mise run test`   | `cargo test --workspace --all-targets` + `cargo test --workspace --doc` | `test-linux` 等 |
| `mise run deny`   | `cargo deny check all`                                                  | `deny`          |
| `mise run ci`     | preflight + 上記 5 つを並列実行 (check run なし)                        | (なし)          |
| `mise run ci-publish` | ci 相当 + 結果を **CI と同名** check run として PR HEAD に投影      | (なし)          |

CI で multi-OS (ubuntu / macos / windows) を確認するときは、PR コメントで
`!run ci matrix` を投稿する (`.github/workflows/ci.yml` の trigger 仕様参照)。
MSRV (1.82) 検証もローカル task からは外しており、CI 側の `msrv` job に
任せる方針 (= `ci-publish` も MSRV check は投影しない)。

`mise run ci-publish` で `cargo test` の check は **ローカル OS によらず**
`cargo test (ubuntu-latest)` 名で投影する。Branch protection の Required
として ubuntu-latest だけを必須化している想定に合わせるため。macOS /
Windows での挙動確認は CI 側の `!run ci matrix` で別途行う。

## PR 投稿前のチェックリスト

- [ ] `mise run ci` がローカルで緑になっていること (5 task すべて pass)
- [ ] 変更点に対応するドキュメント (`README.md` / `SPEC.md` / 本ファイル) と
      の整合性を確認すること
- [ ] 関連 issue を `issues/completed/` に移動し、同じ commit に含めること
- [ ] commit message が global `CLAUDE.md` の規約 (動詞終止形 / 体言止め本文)
      に従っていること
- [ ] (任意、CI コスト削減を狙う場合) push 後に `mise run ci-publish` で
      Branch protection の Required check を local 経由で満たす。**上の
      「危険性」セクションを読んで理解した上で実行すること**
