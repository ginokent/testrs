## 大方針

`propcheck` は std とコンパイラ提供の `proc_macro` クレートのみで動作する、
プロパティベーステスト + fuzzing ライブラリ。本ドキュメントは大方針を
記述する。詳細設計はコード内コメントで管理する。

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
  使わない (`propcheck-derive` は手書き parser で実装する)。

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

| クレート           | 目的                                                                  |
|--------------------|-----------------------------------------------------------------------|
| `propcheck-core`   | `Rng`, `XorShift64`, `Arbitrary` trait, `strategy::*` combinator     |
| `propcheck-derive` | `#[derive(Arbitrary)]` と `#[propcheck]` proc-macro                   |
| `propcheck`        | テストランナー、assertion マクロ、regression shrinking                |
| `propcheck-fuzz`   | in-process mutation 駆動の fuzzer (`fuzz` + `fuzz_typed`)             |

通常利用では `propcheck` のみで足りる。`propcheck-core` と
`propcheck-derive` の内容をすべて再エクスポートしている。

## Lint / Doc 方針

- `cargo clippy --workspace --all-targets -- -D warnings` が clean であること。
- `cargo doc --workspace --no-deps` が `RUSTDOCFLAGS=-D warnings` で
  clean であること。
- CI に `doc` ジョブと MSRV (1.82) の `cargo build` ジョブを常設する。
