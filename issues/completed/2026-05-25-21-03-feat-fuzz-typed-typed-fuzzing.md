# fuzz_typed による型付き fuzzing を実装する

- Priority: -
- Created: -
- Completed: 2026-05-25 20:47 JST (BACKLOG.md から移行)
- Model: -
- Branch: - (該当コミット: `ef9f8b1`)

## 完了内容

`fuzz_typed<T: Arbitrary>`: バイト seed を `T::arbitrary` に渡す
型付き fuzzing。

## 関連マイルストーン

M3 — Derive マクロ、assertion、型付きファズ (コミット `ef9f8b1`)
