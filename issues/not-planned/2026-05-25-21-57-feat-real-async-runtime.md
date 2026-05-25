# 本物の async ランタイム同梱は対応しない

- Priority: -
- Created: 2026-05-25 21:06 JST
- Completed: 2026-05-25 21:06 JST (対応しない判断)
- Model: -
- Branch: -

## 目的

tokio などフル機能の async ランタイムを propcheck に同梱する案の
検討。

## 経緯

`BACKLOG.md` の ❌ スコープ外項目から `issues/` 配下へ移行する。

## 優先度根拠

スコープ外と判断したため優先度の議論は不要。

## 現状・問題

組み込み `block_on` (`std::pin::pin!` ベース、unsafe ゼロ) で
純粋な async ロジックには十分対応している。

## 完了条件

スコープ外として明示する。

## 解決方法

対応しない。ネットワーク / タイマー重量の async コードは tokio を
sync ラッパーの中で直接使う。

## 設計方針

- no-deps 方針と両立しない
- 既存の組み込み `block_on` で純粋ロジックには十分
- 重量 async は外部で sync ラッパー化する運用を推奨する

## 対応しない判断

組み込み `block_on` で純粋な async ロジックには十分。ネット
ワーク / タイマー重量の async コードは tokio を sync ラッパーの
中で直接使うべき。
