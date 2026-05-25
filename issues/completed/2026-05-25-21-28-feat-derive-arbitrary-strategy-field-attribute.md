# #[derive(Arbitrary)] のフィールド属性 strategy = を追加する

- Priority: -
- Created: -
- Completed: 2026-05-25 20:47 JST (BACKLOG.md から移行)
- Model: -
- Branch: - (該当コミット: `45e9d00`)

## 完了内容

`#[derive(Arbitrary)]` のフィールド属性
`#[arbitrary(strategy = ...)]`。文字列リテラル形式
(`"expr"` 文字列、proptest スタイル) と裸の式形式の両方をサポート。
named-field 構造体・tuple 構造体・enum variant (両形式) に適用可。

## 関連マイルストーン

M6 — フィールド属性 + 依存 / 再帰生成 (コミット `45e9d00`)
