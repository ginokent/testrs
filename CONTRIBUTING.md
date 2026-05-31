# Contributing

`propcheck` (testrs) への貢献ガイド。**開発手順** (mise install / mise run ci の
使い方)、**設計判断とトレードオフ** (特に `ci-publish-check-dangerously` の
危険性)、リポ局所の規約を集約する。

エージェント (Claude Code 等) 向けの行動指針は [`CLAUDE.md`](CLAUDE.md)、
利用者向けの API と機能一覧は [`README.md`](README.md) を参照。

## 開発手順 (mise + scripts)

ローカル開発では [mise](https://mise.jdx.dev/) のタスクランナーで CI 相当
の check を並列実行する運用。

### Setup (初回のみ)

```bash
# mise 本体: https://mise.jdx.dev/getting-started.html に従って install
mise install        # mise.toml の [tools] (cargo-deny 等) を一括 install

# (任意) ci-publish-check-dangerously を使うなら gh CLI も:
#   https://cli.github.com/
#   gh auth login
```

### 日常使用

```bash
mise run ci         # 開発 loop 用: dirty check + fmt / clippy / doc / test / deny を並列実行

# 個別:
mise run fmt
mise run clippy
mise run doc
mise run test
mise run deny
```

`mise run ci` は **modified / staged の差分があれば fail** (untracked は許容)。
commit してから回す前提。各 task は `scripts/*.sh` を呼ぶ単純な bash wrapper
で、mise を入れていない環境では `./scripts/<name>.sh` を直接実行しても同じ
結果を得られる。

### local / CI 対応表

| local             | 実コマンド                                                              | CI job          |
|-------------------|-------------------------------------------------------------------------|-----------------|
| `mise run fmt`    | `cargo fmt --all -- --check`                                            | `fmt`           |
| `mise run clippy` | `cargo clippy --workspace --all-targets -- -D warnings`                 | `clippy`        |
| `mise run doc`    | `RUSTDOCFLAGS=-D warnings cargo doc --workspace --no-deps`              | `doc`           |
| `mise run test`   | `cargo test --workspace --all-targets` + `cargo test --workspace --doc` | `test` (matrix の各 OS) |
| `mise run deny`   | `cargo deny check all`                                                  | `deny`          |
| `mise run ci`     | preflight + 上記 5 つを並列実行 (check run なし)                        | (なし)          |
| `mise run ci-publish-check-dangerously` | (⚠️ 危険) ci 相当 + 結果を **CI と同名** check run として PR HEAD に投影 ([下の専用セクション参照](#%EF%B8%8F-ci-publish-check-dangerously--コスト削減目的のユーザー責任設計)) | (なし) |

CI は GitHub Actions のコスト最適化のため PR コメント (`!run ci` /
`!run ci matrix`) で trigger する設計。multi-OS (ubuntu / macos / windows)
を確認したいときは PR 上で `!run ci matrix` を投稿する。詳細は
[`.github/workflows/ci.yml`](.github/workflows/ci.yml) 冒頭参照。MSRV (1.82)
検証はローカル task からは外しており、CI 側の `msrv` job に任せる方針。

## ブランチ命名 / コミットメッセージ / issue 管理

- ブランチ命名規則 (`feature/add-` / `feature/fix-` / `feature/update-` /
  `feature/breaking-change-`)、コミットメッセージ規約 (動詞終止形 /
  体言止め本文)、issue 管理 (`issues/` 配下、`issues/completed/` への移動)
  は global な開発ルール (`~/.claude/CLAUDE.md`) に従う
- ブランチ名に issue 番号を含めない
- issue は JST 時刻 `YYYY-mm-dd-HH-MM` を id として `{id}-{conv-type}-{desc}.md`
  形式で作成する

## ⚠️ `ci-publish-check-dangerously` — コスト削減目的のユーザー責任設計

本リポジトリは **個人開発前提** で、GitHub Actions のコストを実質ゼロ化する
ため、ローカル check 結果を CI と同名の check run として PR HEAD に投影する
経路 (`mise run ci-publish-check-dangerously`) を許容している。本セクションは
その設計判断・危険性・第三者環境での切替案を一元化して説明する。

### 動作

`scripts/ci-publish-check-dangerously.sh` は以下を順に行う:

1. **preflight** (dirty check): `git diff --quiet` + `git diff --cached
   --quiet` で modified / staged の差分があれば exit 1 (untracked は許容)
2. **`gh` CLI の存在チェック**: 未 install なら exit 1
3. **push 済み確認**: HEAD commit が remote 上に存在するか確認、未 push なら
   exit 1
4. **5 task の順次実行**: fmt / clippy / doc / test / deny を呼び、各 task の
   終了直後に `scripts/report-check-local.sh` で結果を **CI と同名** の
   check run として `gh api repos/.../check-runs` で PR HEAD に POST
5. **集約 status**: 1 つでも fail があれば全体 exit 1

投影する check 名 (CI workflow の各 job `name:` と完全一致):

- `cargo fmt --check`
- `cargo clippy -D warnings`
- `cargo doc`
- `cargo test (ubuntu-latest)` (ローカル OS を問わず固定)
- `cargo deny (license/bans/sources/advisories)`

MSRV (1.82) は CI 専用 job のため投影対象外。

### なぜ CI と同名にするのか

**Branch protection の Required status checks を local 1 回で満たせる** ため。
GitHub Actions を発火させずに merge を成立させることができ、CI コストを
実質ゼロにできる。これが本構成を導入した一義的目的。

### ⚠️ 裏返しのリスク (実行者が負う責任)

| リスク | 詳細 |
|---|---|
| ローカル環境差 | OS / toolchain / cache 状態 / 環境変数の違いで、本来 CI で落ちる check が local では緑になりうる。再現性が CI < local になる |
| API 直叩きで偽装可能 | `scripts/report-check-local.sh` は `gh api` で任意の `conclusion` を POST できる。ローカルで check を回さずに「success」だけ post することも技術的に可能 |
| 「緑」 ≠ 「実コード緑」 | PR HEAD に紐付いた check run はあくまで「ローカル実行者がそう報告した」だけ。実 PR コードが緑である保証にはならない |
| 第三者監査が困難 | CI のように workflow log が GitHub 上に残らないため、check 緑の根拠を第三者が事後検証できない |
| typo / 未投影で偽陽性 | 投影する check 名と Branch protection の Required 設定値が一致していないと、緑表示されているのに実は merge ブロックを満たしていない、というケースが発生しうる |

→ **個人開発でユーザー自身が緑判定の真偽を担保できる状況でのみ運用可能**。
複数 contributor 環境では機能しない。

### 第三者 contributor 環境での切替案

本構成を他リポにばらまく際、co-maintainer / 外部 contributor が居る場合は
**「CI と同名」戦略を捨てるべき**。具体的な切替手順:

1. `scripts/ci-publish-check-dangerously.sh` の `run_and_report` 呼び出しを
   `local-ci: <task>` のような prefix 付き名前に変更:
   ```bash
   run_and_report "local-ci: cargo fmt --check"   "./scripts/fmt.sh"
   ```
2. Branch protection の Required check は CI workflow が投影する元の名前を
   そのまま使用 (= ローカル check では満たせない)
3. (任意) `ci-publish-check-dangerously` を `ci-publish-info` 等にリネーム
   し、「CI を override する」性質を持たないことを明示
4. (任意) 完全に撤去 (= ローカルは `mise run ci` のみ、Required check は
   CI 経路でしか満たせない、最も健全な構成)

### エージェント (Claude Code 等) への指示

`mise run ci-publish-check-dangerously` は **ユーザーからの明示的な指示が
ある場合に限り実行する**。エージェントが自発的に判断して呼ばないこと。
ユーザーが指示した場合も、本セクションの「⚠️ 裏返しのリスク」を理解した
上での実行であることを応答内で確認すること。

`mise run ci` (check run を投げない安全版) は通常運用として呼んで差し支え
ない (PR 作成前のセルフチェック手段)。

## ドキュメント間の責務分担

| ファイル | 役割 |
|---|---|
| [`README.md`](README.md) | プロジェクト概要、利用方法、API パターン集、機能一覧 |
| [`SPEC.md`](SPEC.md) | プロジェクトの大方針設計 (依存方針 / Toolchain pin / Workspace 構成等) |
| [`CLAUDE.md`](CLAUDE.md) | エージェント (Claude Code 等) への局所行動指針、リポ固有の落とし穴 |
| `CONTRIBUTING.md` (本ファイル) | 開発手順 (mise + scripts) と設計判断のトレードオフ (特に ci-publish-check-dangerously の危険性) |
| `~/.claude/CLAUDE.md` (global) | 全リポ共通の規約 (ブランチ命名 / コミットメッセージ / テスト方針 / 言語) |

ドキュメントを変更する際は、上の表の責務に従って適切なファイルに書くこと。
本リポを他リポにばらまく際は `CLAUDE.md` / `CONTRIBUTING.md` を共通テンプレ
として転用する想定 (README は各リポ固有の API / 利用方法に特化させる)。
