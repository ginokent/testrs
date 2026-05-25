# crates.io 公開向けメタデータ整備は対応しない

- Priority: -
- Created: 2026-05-25 21:10 JST
- Completed: 2026-05-25 21:10 JST (対応しない判断)
- Model: -
- Branch: -

## 目的

各サブクレートに `readme` / `keywords` / `categories` を追加し、
`LICENSE-*` ファイル同梱、公開順序ドキュメントなどを整備する案の
検討。

## 経緯

`BACKLOG.md` の ❌ スコープ外項目から `issues/` 配下へ移行する。

## 優先度根拠

スコープ外と判断したため優先度の議論は不要。

## 現状・問題

crates.io 公開向けメタデータが未整備だが、`SPEC.md` の配布方針に
より crates.io 公開は行わない。

## 完了条件

スコープ外として明示する。

## 解決方法

対応しない。配布方針に従い crates.io 公開は行わない。

## 設計方針

- 配布方針: git dependency 前提とし crates.io には公開しない
- 公開順序ドキュメント、`readme` / `keywords` / `categories` 等の
  整備は行わない

## 対応しない判断

配布方針に従い crates.io 公開は行わない。
