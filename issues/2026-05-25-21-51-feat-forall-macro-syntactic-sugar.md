# forall! シンタックスシュガーマクロを追加する

- Priority: Low
- Created: 2026-05-25 21:00 JST
- Completed:
- Model: -
- Branch: feature/add-forall-macro-syntactic-sugar

## 目的

`forall` 呼び出しの宣言性を高めるためのマクロ。

## 経緯

`BACKLOG.md` の 🟡 バックログ項目「テスト足回り」から
`issues/` 配下へ移行する。

## 優先度根拠

BACKLOG.md 上の影響度は「軽微」。Low とする。

## 現状・問題

`forall("name", |t: &T| body)` の書式はやや冗長で、変数バインド
形式の DSL があると読みやすい。

## 完了条件

- `forall!` マクロを追加し、変数 + 型バインドの構文を提供する
- 既存 `forall` への展開を確認する単体テストを追加する

## 解決方法

BACKLOG.md 記載の概算は約 30 行。declarative macro で
`forall! { |x: T, y: U| body }` を `forall(name, |(x, y): &(T, U)| body)`
に展開する。

## 設計方針

- no-deps 方針を維持する (proc-macro ではなく declarative macro)
- 既存 `forall` の意味論を変えない
- マクロ展開後のエラーメッセージが読みやすいよう、span を可能な
  限り保つ
