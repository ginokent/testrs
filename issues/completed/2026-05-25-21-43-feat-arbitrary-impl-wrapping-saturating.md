# Wrapping<T> / Saturating<T> の Arbitrary 実装を追加する

- Priority: Low
- Created: 2026-05-25 20:52 JST
- Completed: 2026-05-29 JST (feature/add-arbitrary-impl-stdlib-types で実装)
- Model: -
- Branch: feature/add-arbitrary-impl-stdlib-types

## 目的

`std::num::Wrapping<T>` と `std::num::Saturating<T>` を `Arbitrary`
経由で生成できるようにし、stdlib カバレッジを底上げする。

## 経緯

`BACKLOG.md` の 🟡 バックログ項目「追加 Arbitrary 実装」から
`issues/` 配下へ移行する。

## 優先度根拠

BACKLOG.md 上の影響度は「小」。Low とする。

## 現状・問題

現状これらのラッパー型の `Arbitrary` 実装が存在せず、
ユーザ側で手書きする必要がある。

## 完了条件

- `Arbitrary` を `Wrapping<T>` と `Saturating<T>` (`T: Arbitrary`)
  に実装する
- 各実装の単体テストと、内部値とのラウンドトリップ PBT を追加する

## 解決方法

BACKLOG.md 記載の概算は両者合わせて約 30 行。内部 `T` の
`Arbitrary` を呼んでラッパーで包む。

## 設計方針

- no-deps 方針を維持する
- 整数の `Arbitrary` 実装と整合する shrink (内部 `T` の shrink を
  ラッパーで再構築する形) を採用する

## 完了内容

- `crates/propcheck-core/src/collections.rs` に `impl<T: Arbitrary> Arbitrary for Wrapping<T>` と `Saturating<T>` を追加 (内部 `T` を生成して newtype 透過で wrap)
- shrink は内部 `T` の shrink を順次 wrap し直す形式 (整数の shrink_toward_zero と整合)
- round-trip テスト (内部値の再 wrap で同値) と shrink がゼロに到達する単体テストを追加
