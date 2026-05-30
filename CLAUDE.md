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
```

### 日常使用

```bash
mise run ci         # 全 check を並列実行 (fmt / clippy / doc / test / deny)

# 個別:
mise run fmt
mise run clippy
mise run doc
mise run test
mise run deny
```

各 task は `scripts/*.sh` を呼ぶ単純な bash wrapper。mise を入れていない
環境では `./scripts/<name>.sh` を直接実行しても同じ結果を得られる。

### local / CI 対応

| local             | 実コマンド                                                              | CI job          |
|-------------------|-------------------------------------------------------------------------|-----------------|
| `mise run fmt`    | `cargo fmt --all -- --check`                                            | `fmt`           |
| `mise run clippy` | `cargo clippy --workspace --all-targets -- -D warnings`                 | `clippy`        |
| `mise run doc`    | `RUSTDOCFLAGS=-D warnings cargo doc --workspace --no-deps`              | `doc`           |
| `mise run test`   | `cargo test --workspace --all-targets` + `cargo test --workspace --doc` | `test-linux` 等 |
| `mise run deny`   | `cargo deny check all`                                                  | `deny`          |
| `mise run ci`     | 上記 5 つを並列実行                                                     | (PR コメントで) |

CI で multi-OS (ubuntu / macos / windows) を確認するときは、PR コメントで
`!run ci matrix` を投稿する (`.github/workflows/ci.yml` の trigger 仕様参照)。
MSRV (1.82) 検証もローカル task からは外しており、CI 側の `msrv` job に
任せる方針。

## PR 投稿前のチェックリスト

- [ ] `mise run ci` がローカルで緑になっていること (5 task すべて pass)
- [ ] 変更点に対応するドキュメント (`README.md` / `SPEC.md` / 本ファイル) と
      の整合性を確認すること
- [ ] 関連 issue を `issues/completed/` に移動し、同じ commit に含めること
- [ ] commit message が global `CLAUDE.md` の規約 (動詞終止形 / 体言止め本文)
      に従っていること
