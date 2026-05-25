# snapshot テスト (insta スタイル) は対応しない

- Priority: -
- Created: 2026-05-25 21:07 JST
- Completed: 2026-05-25 21:07 JST (対応しない判断)
- Model: -
- Branch: -

## 目的

`insta` 相当の snapshot テスト機能の検討。

## 経緯

`BACKLOG.md` の ❌ スコープ外項目から `issues/` 配下へ移行する。

## 優先度根拠

スコープ外と判断したため優先度の議論は不要。

## 現状・問題

snapshot テストは別領域のテスト関心事であり、propcheck の
プロパティテスト / fuzzing 責務と直交している。

## 完了条件

スコープ外として明示する。

## 解決方法

対応しない。`insta` に任せる。

## 設計方針

- propcheck の責務範囲外
- snapshot テストを必要とするユースケースでは `insta` 等を併用する

## 対応しない判断

別領域のテスト関心事。`insta` に任せる。
