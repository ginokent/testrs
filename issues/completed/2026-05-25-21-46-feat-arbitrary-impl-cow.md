# Cow<'static, str> / Cow<'static, [T]> の Arbitrary 実装を追加する

- Priority: Low
- Created: 2026-05-25 20:55 JST
- Completed: 2026-05-29 JST (feature/add-arbitrary-impl-stdlib-types で実装)
- Model: -
- Branch: feature/add-arbitrary-impl-stdlib-types

## 目的

`std::borrow::Cow<'static, str>` と `Cow<'static, [T]>` を
`Arbitrary` 経由で生成できるようにする。

## 経緯

`BACKLOG.md` の 🟡 バックログ項目「追加 Arbitrary 実装」から
`issues/` 配下へ移行する。

## 優先度根拠

BACKLOG.md 上の影響度は「小」。Low とする。

## 現状・問題

`Cow<'static, _>` の `Arbitrary` 実装がないため、`Cow` を含む
型を扱うテストで毎回ラップが必要。

## 完了条件

- `Arbitrary for Cow<'static, str>` を追加する
- `Arbitrary for Cow<'static, [T]> where T: Arbitrary + Clone + 'static`
  を追加する
- いずれも `Owned` バリアントを返すことと、内部値のラウンドトリップ
  PBT を追加する

## 解決方法

BACKLOG.md 記載の概算は両者合わせて約 30 行。`String` / `Vec<T>` を
生成し `Cow::Owned` で包む。

## 設計方針

- no-deps 方針を維持する
- `'static` 制約があるため `Borrowed` を返さず `Owned` 固定とする
- shrink は内部 `String` / `Vec<T>` の shrink をそのまま `Cow::Owned`
  に包み直す

## 完了内容

- `crates/propcheck-core/src/collections.rs` に以下の 2 つの impl を追加:
  - `impl Arbitrary for Cow<'static, str>`
  - `impl<T: Arbitrary + Clone + 'static> Arbitrary for Cow<'static, [T]>`
- いずれも `'static` 制約のため `Borrowed` バリアントの構築は一般に困難な点を考慮し、常に `Owned` を返す方針 (`String` / `Vec<T>` を生成して `Cow::Owned` で包む)
- shrink は内部 `String` / `Vec<T>` の shrink を順次 `Cow::Owned` で再構築
- 常に `Owned` を返すこと、および空への shrink の単体テストを追加
