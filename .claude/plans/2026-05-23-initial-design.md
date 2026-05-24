# 初期設計 — propcheck ワークスペース

**Status: コミット `793f067` (M1) で着地。`27c663d` (M2) と
`ef9f8b1` (M3) で段階的に拡張。**

## ゴール

Rust 向けの dev-dependency プロパティベーステスト + ファジング
ライブラリ。外部依存ゼロで、80% のケースについて `proptest` +
`cargo-fuzz` の代替として使える。

## 制約

- **外部クレートなし。** std とコンパイラ提供の `proc_macro`
  クレートのみ。ワークスペース全体で `unsafe_code = forbid`。
- **ワークスペース、マルチクレート構成。** モノリシッククレートは
  進化させづらい。

## クレート構成

```
crates/
├── propcheck-core/   # Rng + Arbitrary trait + Strategy コンビネータ
├── propcheck-derive/ # proc-macro: #[derive(Arbitrary)] + #[propcheck]
├── propcheck/        # ランナー、アサーションマクロ、regression replay
└── propcheck-fuzz/   # in-process ミューテーション・バイトファザー + 型付きファズ
```

アンブレラクレート `propcheck` が `-core` と `-derive` の内容を
すべて再エクスポート。ユーザは通常 `propcheck` のみに依存
(ファジングを使う場合は `propcheck-fuzz` も)。

## 中核抽象

- `Rng`: 最小 PRNG trait。必須メソッドは `next_u64` のみ。
  `gen_range_u64`, `gen_range_i64`, `gen_range_usize`, `gen_bool`,
  `fill_bytes`, `choose` のデフォルト実装付き。
- `XorShift64`: デフォルト PRNG。Marsaglia/Vigna の xorshift64* —
  高速、seed あたり決定論的、周期 2^64−1。
- `Arbitrary`: `arbitrary(&mut R, size: usize) -> Self` と、
  デフォルト実装付きの
  `shrink(&self) -> Box<dyn Iterator<Item=Self> + '_>`。

## シュリンク戦略

- 型ごとのシュリンク: 整数は 0 方向 (halve-then-walk-back)、
  コレクションは長さ → 要素ごと、文字列は char vec 経由。
- ランナーのシュリンクループは「まだ失敗する最初の候補を採用」
  (Greedy)。M5 で Exhaustive モードを追加。

## 初期 Arbitrary 実装

`bool`、全整数幅、`f32`、`f64`、`char`、`String`、`Vec<T>`、
`Option<T>`、`Result<T, E>`、タプル 0/1/2/3/4。

## ランナー

- `Arbitrary` 駆動プロパティ向けの `forall` / `run`。
- デフォルト 100 ケース、`Config` で設定可能。
- 再現用環境変数 `PROPCHECK_SEED`。
- `std::panic::catch_unwind` による panic 捕捉。

## ファザー

- `fuzz(cfg, |&[u8]| ...)` — ミューテートされたバイト列を
  ターゲットに流す。Crash 信号として panic を使用。初期コーパスは
  任意。
- ミューテーション: bit-flip, byte-replace, interesting-byte,
  insert, delete, splice (コーパスから), swap。
- 最小化: 1 バイト削除 + ゼロ置換のパスを greedy に適用。
- 再現用環境変数 `PROPCHECK_FUZZ_SEED`。

## サンプル

- `sort_props` — ソートの冪等性 / 長さ保存 / 非減少。
- `fail_demo` — 意図的失敗時のシュリンク出力デモ。
- `find_crash` — バグを仕込んだパーサのファジング。

## 未対応 (この時点)

初期リリースでは意図的に以下を含めず:

- カスタム strategy / コンビネータ (M2 で追加)。
- `#[derive(Arbitrary)]` (M3 で追加)。
- アサーションマクロ、`prop_assume!`、classify、async、状態機械 —
  以降のマイルストーンで全部追加。
