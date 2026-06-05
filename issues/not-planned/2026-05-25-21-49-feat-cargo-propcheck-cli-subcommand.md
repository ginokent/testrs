# cargo propcheck CLI サブコマンドを追加する

- Priority: Low
- Created: 2026-05-25 20:58 JST
- Completed:
- Model: -
- Branch: feature/add-cargo-propcheck-cli-subcommand

## 目的

長時間ファズランの progress bar、名前付き seed での再現等を
コマンドラインから扱えるようにする。

## 経緯

`BACKLOG.md` の 🟡 バックログ項目「CI / レポーティング」から
`issues/` 配下へ移行する。

## 優先度根拠

BACKLOG.md 上の影響度は「低 — 既に `cargo test` で動く」。Low と
する。

## 現状・問題

ファズランを長時間回す際の進捗確認や、名前付き seed の保存 /
再現を行う UI がなく、対話的なワークフローが弱い。

## 完了条件

- 新規 bin クレート (例: `propcheck-cli`) を追加する
- `cargo propcheck` として呼び出せるサブコマンド形式にする
- 進捗表示、名前付き seed 保存 / 再現、テスト指定実行などの
  サブコマンドを実装する
- 主要サブコマンドの統合テストを追加する

## 解決方法

BACKLOG.md 記載の概算は新規 bin クレート 500 行以上。Cargo
subcommand 規約 (`cargo-{name}` バイナリ + ワークスペース統合) に
従う。

## 設計方針

- no-deps 方針を維持する (clap などは使わず手書き parser)
- 進捗表示は ANSI escape を直書きする
- 既存ランナー / fuzzer の公開 API のみを使う

## クローズ (not-planned, 2026-06-05)

費用対効果が見合わないため not-planned とする。

- 主目的 (テスト駆動・seed 再現) は既に `cargo test` + `PROPCHECK_SEED`
  env + regression replay で満たされており、新規需要が提示されていない
- 概算 500 行超の新規 bin crate は表面積が大きく、progress bar 等の
  対話 UI は no-deps 方針 (手書き ANSI / 手書き parser) と緊張する。
  得られる利便に対し維持コストが見合わない
- CLI が本当に要るのは需要が顕在化してから。その際は SPEC の方針通り
  別兄弟 crate として起票し直す
