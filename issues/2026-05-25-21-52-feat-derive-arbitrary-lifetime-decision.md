# #[derive(Arbitrary)] の lifetime パラメータ方針を決める

- Priority: Low
- Created: 2026-05-25 21:01 JST
- Completed:
- Model: -
- Branch: feature/add-derive-arbitrary-lifetime-decision

## 目的

`#[derive(Arbitrary)]` 対象型に lifetime パラメータが含まれる
ケースについて、明示的な方針を決め (非サポートとしてドキュメント化
するか、`borrowed` アダプタを設計するか) て対応する。

## 経緯

`BACKLOG.md` の 🟡 バックログ項目「エッジケース」から
`issues/` 配下へ移行する。

## 優先度根拠

BACKLOG.md 上の影響度は「小」。Low とする。

## 現状・問題

参照型はそのままでは `Arbitrary` にならない (生成すべき所有値が
無い)。現状は曖昧なコンパイルエラーになる。

## 完了条件

- 方針を決定する (非サポート明示 / アダプタ提供のいずれか)
- 非サポートの場合: derive で lifetime パラメータを検出した時に
  明確なエラーメッセージを出すこと、README / docs に記載すること
- アダプタの場合: `borrowed` アダプタの API と PBT を追加すること

## 解決方法

BACKLOG.md 記載の通り、まず方針決定が必要。非サポート明示が
最小コストで、`borrowed` アダプタは別 issue とする選択もあり得る。

## 設計方針

- no-deps 方針を維持する
- proc-macro での lifetime parameter 検出は既存 parser を拡張する
- 方針決定後、README の「制約」セクションに反映する
