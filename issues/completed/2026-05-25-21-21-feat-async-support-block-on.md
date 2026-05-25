# Async サポートと組み込み block_on を追加する

- Priority: -
- Created: -
- Completed: 2026-05-25 20:47 JST (BACKLOG.md から移行)
- Model: -
- Branch: - (該当コミット: `48eba2e`)

## 完了内容

Async サポート: 組み込み `block_on` (`std::pin::pin!` ベース、
unsafe ゼロ) + `#[propcheck] async fn` のラッパー。

## 関連マイルストーン

M5 — Tier S/A/B (コミット `48eba2e`)
