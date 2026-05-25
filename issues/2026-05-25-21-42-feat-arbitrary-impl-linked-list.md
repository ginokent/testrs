# LinkedList<T> の Arbitrary 実装を追加する

- Priority: Low
- Created: 2026-05-25 20:51 JST
- Completed:
- Model: -
- Branch: feature/add-arbitrary-impl-linked-list

## 目的

`std::collections::LinkedList<T>` を `Arbitrary` 経由で生成
できるようにし、stdlib カバレッジを底上げする。

## 経緯

`BACKLOG.md` の 🟡 バックログ項目「追加 Arbitrary 実装」から
`issues/` 配下へ移行する。

## 優先度根拠

BACKLOG.md 上の影響度は「小」。Low とする。

## 現状・問題

現状 `LinkedList<T>` の `Arbitrary` 実装が存在せず、ユーザ側で
手書きする必要がある。

## 完了条件

- `Arbitrary` を `LinkedList<T> where T: Arbitrary` に実装する
- 生成サイズと型に関する単体テスト + PBT を追加する

## 解決方法

BACKLOG.md 記載の概算は約 20 行。`Vec<T>` ベースで生成し
`LinkedList::from_iter` で構築する。

## 設計方針

- no-deps 方針を維持する
- 既存 `Vec<T>` / `VecDeque<T>` の `Arbitrary` 実装と整合的な
  size 制約を採用する
