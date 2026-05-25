# derive のコンパイルエラーメッセージを改善する

- Priority: Medium
- Created: 2026-05-25 21:03 JST
- Completed:
- Model: -
- Branch: feature/add-derive-improve-compile-error-messages

## 目的

`#[derive(Arbitrary)]` 適用時、フィールド型が `Arbitrary` を
実装していない場合のエラーメッセージを分かりやすくする。

## 経緯

`BACKLOG.md` の 🟡 バックログ項目「エッジケース」から
`issues/` 配下へ移行する。

## 優先度根拠

BACKLOG.md 上「新参ユーザには影響: 中」と記載。学習体験に直結
するため Medium とする。

## 現状・問題

現状、フィールド型が `Arbitrary` を実装していないと一般的な
trait 境界エラーが出るだけで、原因のフィールド名 / 位置が
分かりにくい。

## 完了条件

- proc-macro 出力にフィールド span を持たせ、未実装フィールドが
  指摘されるようにする
- 単体テスト (`trybuild` 相当の compile-fail テスト) を no-deps
  で実装可能なら追加する

## 解決方法

BACKLOG.md 記載の通り、proc-macro span の利用が必要。`TokenStream`
を組み立てる際に元フィールド名 / 型の span を保つ。

## 設計方針

- no-deps 方針を維持する (`syn` は使わず手書き parser で span を
  扱う)
- compile-fail テストは std 範囲で組めない場合、手動確認 + README
  例示で代替する
- 既存の正常系の挙動は変えない
