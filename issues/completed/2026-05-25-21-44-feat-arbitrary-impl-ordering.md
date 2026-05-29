# std::cmp::Ordering の Arbitrary 実装を追加する

- Priority: Low
- Created: 2026-05-25 20:53 JST
- Completed: 2026-05-29 JST (feature/add-arbitrary-impl-stdlib-types で実装)
- Model: -
- Branch: feature/add-arbitrary-impl-stdlib-types

## 目的

`std::cmp::Ordering` を `Arbitrary` 経由で生成できるようにする。

## 経緯

`BACKLOG.md` の 🟡 バックログ項目「追加 Arbitrary 実装」から
`issues/` 配下へ移行する。

## 優先度根拠

BACKLOG.md 上の影響度は「小」。Low とする。

## 現状・問題

`Ordering` は 3 値しかとらない単純型だが `Arbitrary` 実装が
ないため、テスト側で `one_of` などで手書きする必要がある。

## 完了条件

- `Arbitrary for Ordering` を追加する
- 3 値が概ね一様にサンプリングされることの単体テスト + PBT を
  追加する

## 解決方法

BACKLOG.md 記載の概算は約 15 行。`Less` / `Equal` / `Greater`
を一様分布で返す実装。

## 設計方針

- no-deps 方針を維持する
- shrink は `Equal` を最小値とする (最も「中立」な値)

## 完了内容

- `crates/propcheck-core/src/collections.rs` に `impl Arbitrary for Ordering` を追加 (`gen_range_usize(0, 3)` による `Less` / `Equal` / `Greater` の一様サンプリング)
- shrink は `Equal` を終端最小値とし、`Less` / `Greater` から 1 段で `Equal` に到達する形式
- 3 値網羅 (300 回試行で全 variant 出現) と shrink 形状の単体テストを追加
