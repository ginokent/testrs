# Reverse<T> の Arbitrary 実装を追加する

- Priority: Low
- Created: 2026-05-25 20:54 JST
- Completed:
- Model: -
- Branch: feature/add-arbitrary-impl-reverse

## 目的

`std::cmp::Reverse<T>` を `Arbitrary` 経由で生成できるようにする。

## 経緯

`BACKLOG.md` の 🟡 バックログ項目「追加 Arbitrary 実装」から
`issues/` 配下へ移行する。

## 優先度根拠

BACKLOG.md 上の影響度は「小」。Low とする。

## 現状・問題

`Reverse<T>` の `Arbitrary` 実装がないため、降順比較を絡めた
テストを書く時にユーザ側でラップする必要がある。

## 完了条件

- `Arbitrary for Reverse<T> where T: Arbitrary` を追加する
- 内部値とのラウンドトリップ PBT を追加する

## 解決方法

BACKLOG.md 記載の概算は約 15 行。内部 `T::arbitrary` を呼んで
`Reverse(t)` で包む。

## 設計方針

- no-deps 方針を維持する
- shrink は内部 `T` の shrink をそのまま `Reverse` で包み直す
