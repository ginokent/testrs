# 既製ドメイン strategy パックを追加する

- Priority: Medium
- Created: 2026-05-25 20:56 JST
- Completed: 2026-05-29 JST (feature/add-domain-strategy-pack で実装)
- Model: -
- Branch: feature/add-domain-strategy-pack

## 目的

`email_like`, `url_like`, `uuid_like`, `ipv4_dotted`, `iso8601_date`
を既製で提供し、ドメイン固有なテストデータの生成コストを下げる。

## 経緯

`BACKLOG.md` の 🟡 バックログ項目「ドメイン Strategy パック」から
`issues/` 配下へ移行する。

## 優先度根拠

BACKLOG.md 上の影響度は「parser テストでは中」。parser や
シリアライザ系のテストで価値が大きいため Medium とする。

## 現状・問題

ドメイン文字列の strategy はユーザがその都度書く必要があり、
書き方が一貫しないという問題がある。

## 完了条件

- `propcheck::strategy::domain` (もしくは類似モジュール) に
  `email_like`, `url_like`, `uuid_like`, `ipv4_dotted`, `iso8601_date`
  を追加する
- 各 strategy について、生成値が概ね期待する形状を満たすことの
  単体テストと PBT を追加する
- 「擬似的な近似であって RFC / ISO 仕様準拠ではない」点を
  ドキュメントで明示する

## 解決方法

BACKLOG.md 記載の概算は合計約 200 行。既存の文字列 generator
(`ascii_digits`, `ascii_letters_*`, `hex_string` 等) を組み合わせて
合成する。

## 設計方針

- no-deps 方針を維持する (regex / dtparse 等は使わない)
- 仕様準拠でない近似であることを明示する。`*_like` という命名で
  「近似」を伝える
- 既存の文字列 generator (`propcheck::strategy::str`) を再利用する

## 完了内容

- `crates/propcheck-core/src/strategy_domain.rs` を新規追加し、`propcheck::strategy::domain` モジュールとして公開
- 5 strategy + 5 builder fn を追加:
  - `EmailLike` / `email_like()` — `<local>@<domain>.<tld>` (小英字 + 共通 TLD)
  - `UrlLike` / `url_like()` — builder pattern で `with_schemes(&[...])` と `with_port_range(0..u16::MAX)` をサポート。デフォルトは http/https / port なし / 50% で path 付与
  - `UuidLike` / `uuid_like()` — 8-4-4-4-12 hex (version/variant bit は強制せず)
  - `Ipv4Dotted` / `ipv4_dotted()` — 各オクテット 0..256
  - `Iso8601Date` / `iso8601_date()` — `YYYY-MM-DDTHH:MM:SSZ` (日は閏年回避のため 1..29 に近似、tz は Z 固定)
- 全 strategy で `shrink_value` は空 vec を返す設計 (構造化文字列の shrink は意味が崩れるため、`*_like` の精度内で意図的に省略)
- ユーザー追加要件に応じ `url_like` のみ builder pattern (`with_schemes` / `with_port_range`) で拡張性を確保
- 共通 TLD は `const COMMON_TLDS: &[&str] = &["com", "org", "net", "io"];` で集約
- 10 件の単体テストを追加 (各 strategy の形状検証、`url_like` のカスタマイズ・空 schemes / 空 port_range の panic 検証)
- `propcheck-core::strategy::domain` モジュールを `pub mod domain { pub use crate::strategy_domain::*; }` で公開、`propcheck` から `propcheck::strategy::domain::*` で利用可能
