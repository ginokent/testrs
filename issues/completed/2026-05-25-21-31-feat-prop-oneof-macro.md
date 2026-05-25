# prop_oneof! マクロを追加する

- Priority: -
- Created: -
- Completed: 2026-05-25 20:47 JST (BACKLOG.md から移行)
- Model: -
- Branch: - (該当コミット: `45e9d00`)

## 完了内容

`prop_oneof![ … ]` マクロ。一様 (`prop_oneof![a, b, c]`) と
重み付き (`prop_oneof![1 => a, 4 => b]`) の両形式で、異なる具象型を
持つサブ strategy を自動 box 化してまとめる。

## 関連マイルストーン

M6 — フィールド属性 + 依存 / 再帰生成 (コミット `45e9d00`)
