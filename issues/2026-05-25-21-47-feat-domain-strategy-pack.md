# 既製ドメイン strategy パックを追加する

- Priority: Medium
- Created: 2026-05-25 20:56 JST
- Completed:
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
