# フル正規表現ベース文字列生成は対応しない

- Priority: -
- Created: 2026-05-25 21:04 JST
- Completed: 2026-05-25 21:04 JST (対応しない判断)
- Model: -
- Branch: -

## 目的

`proptest` の `regex` 機能に相当する、正規表現を strategy として
直接使える機能の検討。

## 経緯

`BACKLOG.md` の ❌ スコープ外項目から `issues/` 配下へ移行する。

## 優先度根拠

スコープ外と判断したため優先度の議論は不要。

## 現状・問題

正規表現から strategy を構築する機能は存在しないが、
Tier-S の文字列 strategy で 80% のケースをカバーしている。

## 完了条件

スコープ外として明示する。

## 解決方法

対応しない。

## 設計方針

- 数千行規模の `regex` 再実装か、`regex` クレート依存を必要と
  するため、no-deps 方針と両立しない
- 既存の文字列 generator (`propcheck::strategy::str`) と
  `from_char_set` で 80% のユースケースをカバーしており、
  追加コストに見合わない

## 対応しない判断

数千行規模の `regex` 再実装か、クレート依存を必要とする。
Tier-S の文字列 strategy で 80% のケースをカバー。
