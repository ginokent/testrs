# JUnit XML / JSON 出力モードを追加する

- Priority: Low
- Created: 2026-05-25 20:57 JST
- Completed:
- Model: -
- Branch: feature/add-junit-json-output-mode

## 目的

ダッシュボードや CI ツール連携向けに、テスト結果を JUnit XML /
JSON で書き出せるようにする。

## 経緯

`BACKLOG.md` の 🟡 バックログ項目「CI / レポーティング」から
`issues/` 配下へ移行する。

## 優先度根拠

BACKLOG.md 上の影響度は「低 — 通常用途では `cargo test` の出力で
十分」。Low とする。

## 現状・問題

通常用途では `cargo test` の出力で足りるが、ダッシュボード集計
やリッチなレポーティングを行うにはマシン可読な形式が必要。

## 完了条件

- `Outcome` または `Config` 経由で JUnit XML / JSON 出力を
  オプトインできるようにする
- 出力先 (ファイル / stdout) を指定できる
- 単体テストでスキーマ妥当性を検証する

## 解決方法

BACKLOG.md 記載の概算は約 150 行。シリアライズは手書きする
(`serde_json` 等は使わない)。

## 設計方針

- no-deps 方針を維持する (`serde` / `quick-xml` 等は使わない)
- 出力モードはデフォルト無効、`Config` フィールド or 環境変数
  でオプトイン
- 出力先のファイル I/O は std のみで実装する
