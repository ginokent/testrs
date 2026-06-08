# testrs

Rust 向けテストツール群を収めるワークスペース。**外部依存ゼロ** —
std とコンパイラ提供の `proc_macro` クレートのみで動作する。

現在は **PBT** (プロパティベーステスト)・**fuzzing**・**benchmarking** の
3 カテゴリを提供する。共有基盤 `testrs-core` の上に PBT 系 (`testrs-pbt` /
`testrs-pbt-derive`) と fuzzing 系 (`testrs-fuzz`) が乗り、さらに `testrs-core`
にも依存しない独立カテゴリとして benchmarking 系 (`testrs-bench`) が並ぶ。
**各カテゴリは別物**であり、必要な共有基盤だけに依存する兄弟関係にある
(`testrs-fuzz` は `testrs-core` のみに依存し PBT ランナー `testrs-pbt` には
依存しない。`testrs-bench` は計測に Rng/Arbitrary を要しないため
`testrs-core` にすら依存せず std のみで完結する)。いずれも testrs が持つ
複数カテゴリの一つにすぎない。

大方針は [`SPEC.md`](SPEC.md) を参照。計画中の項目・明示的な非ゴール・
完了履歴は [`issues/`](issues/) 配下で、主要な設計判断と gap 分析は
[`.claude/plans/`](.claude/plans/) 配下で管理する。

## ワークスペース構成

| クレート             | カテゴリ | 目的                                                              | README |
|----------------------|----------|-------------------------------------------------------------------|--------|
| `testrs-core`        | 共有     | `Rng`, `XorShift64`, `Arbitrary` trait, `strategy::*` combinator   | —      |
| `testrs-pbt-derive`  | PBT      | `#[derive(Arbitrary)]` と `#[pbt]` proc-macro                      | —      |
| `testrs-pbt`         | PBT          | テストランナー、assertion マクロ、regression shrinking        | [crates/pbt/README.md](crates/pbt/README.md) |
| `testrs-fuzz`        | fuzzing      | in-process mutation 駆動の fuzzer (`fuzz` + `fuzz_typed`)      | [crates/fuzz/README.md](crates/fuzz/README.md) |
| `testrs-bench`       | benchmarking | std のみ依存のマイクロベンチ (`bench` + `bench_compare`)       | [crates/bench/README.md](crates/bench/README.md) |

`testrs-core` と `testrs-pbt-derive` は単体利用を想定しない内部基盤で、
`testrs-pbt` がその内容をすべて再エクスポートする。利用者が直接依存する
のは `testrs-pbt` (PBT 利用)・`testrs-fuzz` (fuzzing 利用)・`testrs-bench`
(benchmarking 利用) の 3 つ。

今後これらとは異質なテストカテゴリを加える場合は、既存 crate の feature
として押し込まず、**新しい兄弟 crate として並べて追加する**。テスト
カテゴリを crate 単位で分離し、各 crate が依存する共有基盤は最小限に
留めるのが本リポジトリの拡張方針である。

## どちらを使うか

- **プロパティベーステスト** — 入力を生成し不変条件 (ラウンドトリップ、
  仕様一致など) を検証したい → **`testrs-pbt`**。
  利用ガイド: [crates/pbt/README.md](crates/pbt/README.md)
- **fuzzing** — 任意入力に対するクラッシュ耐性 (panic 安全性) を叩きたい
  → **`testrs-fuzz`**。利用ガイド: [crates/fuzz/README.md](crates/fuzz/README.md)
- **benchmarking** — コードの実行時間を測りたい / 2 実装を相対比較したい
  → **`testrs-bench`**。利用ガイド: [crates/bench/README.md](crates/bench/README.md)

なお `testrs-fuzz` の `fuzz_typed` に独自型を渡す場合、`#[derive(Arbitrary)]`
の生成コードが PBT facade `testrs-pbt` を参照するため `testrs-pbt` も必要となる。

## インストール

crates.io には公開しない。git dependency として参照される運用を前提とする
(配布方針は [`SPEC.md`](SPEC.md) を参照)。

```toml
[dev-dependencies]
# PBT 利用
testrs-pbt = { git = "https://github.com/ginokent/testrs", package = "testrs-pbt" }
# fuzzing 利用 (オプション)
testrs-fuzz = { git = "https://github.com/ginokent/testrs", package = "testrs-fuzz" }
# benchmarking 利用 (オプション)
testrs-bench = { git = "https://github.com/ginokent/testrs", package = "testrs-bench" }
```

## 最小例

プロパティベーステスト ([詳細](crates/pbt/README.md)):

```rust
use testrs_pbt::{pbt, prop_assert_eq};

#[pbt]
fn addition_is_commutative(a: i32, b: i32) {
    prop_assert_eq!(a.wrapping_add(b), b.wrapping_add(a));
}
```

fuzzing ([詳細](crates/fuzz/README.md)):

```rust
use testrs_fuzz::{fuzz, FuzzConfig};

#[test]
fn parser_does_not_panic() {
    let report = fuzz(FuzzConfig::default(), |bytes: &[u8]| {
        let _ = my_parser::parse(bytes);
    });
    assert!(report.failure().is_none(), "found crash: {:?}", report.failure());
}
```

benchmarking ([詳細](crates/bench/README.md)):

```rust
use testrs_bench::bench_compare;

bench_compare(
    "sort_unstable",
    || { let mut v = vec![3, 1, 2]; v.sort_unstable(); v },
    "sort",
    || { let mut v = vec![3, 1, 2]; v.sort(); v },
);
```

## 設計原則

- **外部依存ゼロ** — 直接依存は std とコンパイラ組み込みの `proc_macro`
  クレートのみ。`syn` / `quote` / `proc-macro2` を含む proc-macro 補助
  クレートも使わない (`testrs-pbt-derive` は手書き parser で実装)。
- **`unsafe_code = "forbid"`** をワークスペース全体で強制。組み込み
  `block_on` executor も `std::pin::pin!` で unsafe ゼロで実装する。
- **Toolchain** — `rust-toolchain.toml` で stable 1.95 に pin。MSRV は 1.82。

## テストスイートの実行

```
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
```

## Contributing

開発手順 ([mise](https://mise.jdx.dev/) install / `mise run ci` の使い方 /
local-CI 対応表)、リポ局所の規約、および設計判断のトレードオフ (特に
`ci-publish-status-dangerously` の危険性) は [`CONTRIBUTING.md`](CONTRIBUTING.md)
を参照。
