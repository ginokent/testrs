# prop_assert_panic! / prop_assert_no_panic! マクロを追加する

- Priority: Low
- Created: 2026-05-25 20:48 JST
- Completed: 2026-06-01 JST (feature/add-panic-assert-and-forall-macros で実装)
- Model: -
- Branch: feature/add-prop-assert-panic-no-panic-macros

## 目的

closure が panic する / しないを assert する手段を提供し、
特定入力で想定通り panic することのテストを書きやすくする。

## 経緯

`BACKLOG.md` の 🟡 バックログ項目「Polish / nice-to-have」から
`issues/` 配下へ移行する。

## 優先度根拠

BACKLOG.md 上の影響度は「小」。ユースケースは限定的だが、ある
場面では非常に便利。Low とする。

## 現状・問題

panic を期待 / 拒否するテストを書くには毎回 `std::panic::catch_unwind`
を直接呼ぶ必要がある。プロパティテスト本体に書くにはノイズが大きい。

## 完了条件

- `prop_assert_panic!(expr)` / `prop_assert_no_panic!(expr)` マクロを
  追加する
- panic ハンドリングが既存 `SilentPanicHook` と整合することを確認する
  単体テストを追加する

## 解決方法

BACKLOG.md 記載の概算は約 30 行。`std::panic::catch_unwind` の
ラッパーとして実装し、結果に応じて `prop_assert!` ベースの
失敗メッセージを返す。

## 設計方針

- no-deps 方針を維持する
- 既存 `prop_assert!` 系マクロと同様の失敗メッセージフォーマットを
  踏襲する
- `SilentPanicHook` のインストール / 解除との整合性を保つ
