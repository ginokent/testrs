# Strategy::no_shrink() ラッパーを追加する

- Priority: Low
- Created: 2026-05-25 20:49 JST
- Completed: 2026-05-29 JST (feature/add-strategy-sample-and-no-shrink で実装)
- Model: -
- Branch: feature/add-strategy-sample-and-no-shrink

## 目的

1 つの strategy だけ shrink を無効化できるようにし、shrink が
高コストだったり、失敗ケースが歪んだりする場合の対処手段を提供する。

## 経緯

`BACKLOG.md` の 🟡 バックログ項目「Polish / nice-to-have」から
`issues/` 配下へ移行する。

## 優先度根拠

BACKLOG.md 上の影響度は「小」。回避策 (個別 strategy の手書き
書き換え) は存在するため、Low とする。

## 現状・問題

特定の strategy だけ shrink を抑制したい場合、`Strategy` を
手書きで実装し直す必要があり、繰り返し書くのが面倒。

## 完了条件

- `StrategyExt::no_shrink()` を追加し、`NoShrink<S>` ラッパー型を
  公開する
- ラップ後は `simplify` が常に `None` を返すことを単体テストで
  確認する

## 解決方法

BACKLOG.md 記載の概算は約 20 行。`Strategy` を再ラップして、
内部 strategy が返した `ValueTree` の `simplify` を常に `None`
として上書きする実装。

## 設計方針

- no-deps 方針を維持する
- 既存 `Strategy` / `ValueTree` 抽象を変えない
- shrink を無効化することによる挙動はドキュメントで明示する

## 完了内容

- `crates/propcheck-core/src/strategy.rs` に `pub struct NoShrink<S>` を追加し、`Strategy` trait を impl (`new_value` は内部に委譲、`shrink_value` は常に空 vec)
- `StrategyExt` trait に `no_shrink(self) -> NoShrink<Self>` を default method として追加
- 内部 strategy で shrink 候補が出る入力 (`int_range(0..100).shrink_value(&50)`) を `no_shrink()` で空にできること、および同じ seed で値生成が一致することを確認する単体テストを追加 (`no_shrink_returns_empty_shrink_candidates` / `no_shrink_preserves_new_value`)
- `propcheck::strategy::NoShrink` として外部から見える (`pub use propcheck_core::strategy` 経由)
