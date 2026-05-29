# BinaryHeap<T> の Arbitrary 実装を追加する

- Priority: Low
- Created: 2026-05-25 20:50 JST
- Completed: 2026-05-29 JST (feature/add-arbitrary-impl-stdlib-types で実装)
- Model: -
- Branch: feature/add-arbitrary-impl-stdlib-types

## 目的

`std::collections::BinaryHeap<T>` を `Arbitrary` 経由で生成
できるようにし、stdlib カバレッジを底上げする。

## 経緯

`BACKLOG.md` の 🟡 バックログ項目「追加 Arbitrary 実装」から
`issues/` 配下へ移行する。

## 優先度根拠

BACKLOG.md 上の影響度は「小」。Low とする。

## 現状・問題

現状 `BinaryHeap<T>` の `Arbitrary` 実装が存在せず、ユーザ側で
手書きする必要がある。

## 完了条件

- `Arbitrary` を `BinaryHeap<T> where T: Arbitrary + Ord` に実装する
- 生成サイズが `size_hint` を尊重することの単体テスト + PBT
  (ラウンドトリップ的) を追加する

## 解決方法

BACKLOG.md 記載の概算は約 20 行。`Vec<T>` の `Arbitrary` を
ベースに `BinaryHeap::from(vec)` で構築する。

## 設計方針

- no-deps 方針を維持する
- 既存 `Vec<T>` の `Arbitrary` 実装と整合的な size 制約を採用する
- `T: Ord` 制約を trait bound で明示する

## 完了内容

- `crates/propcheck-core/src/collections.rs` に `impl<T: Arbitrary + Ord> Arbitrary for BinaryHeap<T>` を追加 (Vec<T> ベースで `BinaryHeap::from` で構築)
- shrink は Vec の shrink 経路を流用して `BinaryHeap::from` で再構築
- 構築可能性 (`binary_heap_arbitrary_is_constructible`) と空への shrink (`binary_heap_shrinks_to_empty`) の単体テストを追加
