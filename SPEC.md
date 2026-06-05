## リポジトリの位置づけ (最上位)

本リポジトリ **testrs** は Rust 向けテストツール群を収めるワークスペース
である。現在は **PBT** と **fuzzing** の 2 カテゴリを提供し、共有基盤
`testrs-core` の上に PBT 系 (`testrs-pbt` / `testrs-pbt-derive`) と fuzzing 系
(`testrs-fuzz`) が乗る構成を採る。

- **testrs** = テストツール群の傘 (ワークスペース / リポジトリ)。
- **PBT と fuzzing は別カテゴリ**である。fuzzing は PBT の一部ではなく、
  両者は共有基盤 `testrs-core` を並行して利用する兄弟関係にある
  (`testrs-fuzz` は `testrs-core` のみに依存し、PBT ランナー `testrs-pbt`
  には依存しない)。PBT も fuzzing も testrs が持つ複数カテゴリの一つにすぎない。
- 今後 PBT / fuzzing とは異質なテストカテゴリ (例: コンパイル時テスト) を
  加える場合は、既存 crate の feature として押し込まず、**新しい兄弟 crate
  として並べて追加する**。テストカテゴリを crate 単位で分離するのが本
  リポジトリの拡張方針である。

workspace 全体に効く制約 (後述の依存方針 / 安全性方針 / toolchain pin) は、
将来追加される crate にも等しく適用する。

## 大方針 (testrs crate 群)

testrs の各 crate は std とコンパイラ提供の `proc_macro` クレートのみで
動作する、プロパティベーステスト + fuzzing のためのライブラリ群。本
ドキュメントは大方針を記述する。詳細設計はコード内コメントで管理する。

## 配布方針

- crates.io には **公開しない**。git dependency として参照される運用を前提とする。
- インストールは `git = "..."` 形式の dependency 指定で行う。
- 内部依存も crates.io 公開要件に縛られない。必要であれば依存先が
  git のみで配布されているクレートでも構わない (本クレート自体の
  no-deps 方針は維持)。
- `readme` / `keywords` / `categories` 等、crates.io ページ向けの
  メタデータ整備、および公開順序の運用ドキュメント等は対応しない。

## 依存方針

- 直接依存は **std とコンパイラ組み込みの `proc_macro` クレートのみ**。
- `syn` / `quote` / `proc-macro2` を含む外部 proc-macro 補助クレートも
  使わない (`testrs-pbt-derive` は手書き parser で実装する)。

## 安全性方針

- ワークスペース全体で `unsafe_code = "forbid"` を強制する。
- 組み込み `block_on` executor も `std::pin::pin!` を使い unsafe ゼロで
  実装する。

## Toolchain pin

- `rust-toolchain.toml` で **stable 1.95** に pin する (rustfmt / clippy 同梱)。
- ローカルと CI の fmt / clippy / test / doc ジョブはすべてこの版で動かす。
- **MSRV は 1.82** (`PanicHookInfo` のため)。MSRV ジョブは `rustup default 1.82`
  で 1.82 に切り替え、`cargo test --workspace --all-targets` まで回す (直接依存・
  dev-dep ともにゼロのため build/check ではなく test で実挙動まで検証する、最も
  厳格な MSRV 保証)。

## ワークスペース構成

現在のワークスペースは 4 crate で構成される。新しいテストカテゴリは、この表に
兄弟 crate として追加していく。

| クレート            | カテゴリ | 目的                                                            |
|---------------------|----------|-----------------------------------------------------------------|
| `testrs-core`       | 共有     | `Rng`, `XorShift64`, `Arbitrary` trait, `strategy::*` combinator |
| `testrs-pbt-derive` | PBT      | `#[derive(Arbitrary)]` と `#[pbt]` proc-macro                    |
| `testrs-pbt`        | PBT      | テストランナー、assertion マクロ、regression shrinking          |
| `testrs-fuzz`       | fuzzing  | in-process mutation 駆動の fuzzer (`fuzz` + `fuzz_typed`)        |

PBT を使う通常利用では `testrs-pbt` のみで足りる (`testrs-core` と
`testrs-pbt-derive` の内容をすべて再エクスポートしている)。fuzzing 利用では
`testrs-fuzz` を使う。なお `fuzz_typed` に独自型を渡す場合、`#[derive(Arbitrary)]`
の生成コードが PBT facade `testrs-pbt` を参照するため `testrs-pbt` も必要となる。

## Lint / Doc 方針

- `cargo clippy --workspace --all-targets -- -D warnings` が clean であること。
- `cargo doc --workspace --no-deps` が `RUSTDOCFLAGS=-D warnings` で
  clean であること。
- CI に `doc` ジョブと MSRV (1.82) の `cargo build` ジョブを常設する。
