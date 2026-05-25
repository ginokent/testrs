# カバレッジ駆動 fuzzing は対応しない

- Priority: -
- Created: 2026-05-25 21:05 JST
- Completed: 2026-05-25 21:05 JST (対応しない判断)
- Model: -
- Branch: -

## 目的

libFuzzer / SanitizerCoverage を活用したカバレッジ駆動 fuzzing の
検討。

## 経緯

`BACKLOG.md` の ❌ スコープ外項目から `issues/` 配下へ移行する。

## 優先度根拠

スコープ外と判断したため優先度の議論は不要。

## 現状・問題

現状の fuzzer はカバレッジフィードバックを持たない「スモーク
fuzzer」であり、カバレッジ駆動はサポートしない。

## 完了条件

スコープ外として明示する。

## 解決方法

対応しない。`cargo-fuzz` の利用を推奨する。

## 設計方針

- LLVM 計装が必要となり、no-deps では到達不可
- カバレッジ駆動が必要なユースケースは `cargo-fuzz` を推奨する

## 対応しない判断

no-deps では到達できない LLVM 計装が必要。このワークフローには
`cargo-fuzz` を推奨。
