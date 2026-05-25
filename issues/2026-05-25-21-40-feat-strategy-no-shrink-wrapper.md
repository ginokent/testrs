# Strategy::no_shrink() ラッパーを追加する

- Priority: Low
- Created: 2026-05-25 20:49 JST
- Completed:
- Model: -
- Branch: feature/add-strategy-no-shrink-wrapper

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
