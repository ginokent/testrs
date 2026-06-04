# derive proc-macro パーサのエラーパス検証を追加する

- Priority: Low
- Created: 2026-06-03 05:52 JST
- Completed:
- Model: -
- Branch: feature/add-derive-parser-error-path-tests

## 目的

手書き proc-macro パーサ (`propcheck-derive`) の `compile_error!`
分岐を検証し、エラーメッセージのリグレッションを検出可能にする。

## 経緯

テスト網羅性の批判的レビューで、手書きパーサのエラーパスが 1 つも
検証されていないことが判明したため起票する。

## 優先度根拠

手書きパーサはバグ混入リスクが最も高い箇所であり、エラー分岐が壊れても
誰も気付けない。一方で no-deps 方針により素直な検証手段が取れず、対応
方針自体にオーバーエンジニアリング回避の観点での判断が要るため Low と
する。

## 現状・問題

- `crates/propcheck-derive/src/lib.rs` (約 1315 行) には
  `#[cfg(test)]` モジュールが無い。
- `parse_input` (185)、`parse_enum_variants` (293)、`parse_generics`
  (363)、`parse_named_fields` (449)、`parse_tuple_fields` (488)、
  `parse_fn` (1104)、`parse_attr_args` (1040) などに多数の
  `return Err(...)` → `compile_error!` 分岐があるが、そのエラーパスを
  1 つも検証していない。
- `crates/propcheck/tests/derive_*.rs` はすべて正常系 (コンパイルが
  通るケース) のみ。
- 「at least one variant on an enum」(256)、「only supports struct and
  enum」(201) のような分岐がリグレッションで壊れても検出できない。

## 完了条件

- 主要な `compile_error!` 分岐 (enum バリアント空、struct/enum 以外、
  属性引数の不正など) について、コンパイル失敗とエラーメッセージを
  検証するテストが存在する

## 解決方法

以下のいずれかを採る (要ユーザー確認)。

- (a) `tests/` に `compile_fail` doctest を並べ、コンパイルエラーと
  メッセージを検証する
- (b) パーサのコア部分を `proc_macro::TokenStream` 非依存の内部トークン
  表現にリファクタし、通常の `#[cfg(test)]` 単体テストで `Err` を直接
  検証可能にする

## 設計方針

- no-deps 方針を維持する (`trybuild` 等の dev-dep 追加は SPEC.md の依存
  方針と衝突するため不可)
- `unsafe_code = "forbid"` を維持する
- 検証手段の選択にあたりオーバーエンジニアリングを避ける
