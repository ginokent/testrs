# testrs-fuzz

任意入力に対するクラッシュ耐性 (panic 安全性) を叩くための、
in-process な mutation 駆動 fuzzer。バイト指向のターゲットが panic
するまで変異入力を実行する。**外部依存ゼロ** — 共有基盤
`testrs-core` のみに依存する。

> ワークスペース **testrs** を構成する一 crate。リポジトリ全体の俯瞰
> (PBT と fuzzing のカテゴリ分離、crate 構成) は
> [リポジトリトップの README](../../README.md) を参照。
>
> **fuzzing は PBT の一部ではなく別カテゴリ**である。`testrs-fuzz` は
> 共有基盤 `testrs-core` のみに依存し、PBT ランナー
> [`testrs-pbt`](../pbt/README.md) には依存しない。入力を生成して
> 不変条件を検証したい場合は `testrs-pbt` を使うこと。

```toml
[dev-dependencies]
testrs-fuzz = { git = "https://github.com/ginokent/testrs", package = "testrs-fuzz" }
```

`fuzz_typed` に独自型を渡す場合、`#[derive(Arbitrary)]` の生成コードが
PBT facade `testrs-pbt` を参照するため `testrs-pbt` も dev-dependency に
追加する。

## クイックスタート

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

mutator はバイト seed 上で変異を加えながらターゲットを繰り返し呼び出す。
ターゲットが panic したら、その入力を反例として `report` に記録する。

## 同梱機能

| 機能                                       | 場所                                                |
|--------------------------------------------|-----------------------------------------------------|
| mutation・バイト fuzzer           | `testrs_fuzz::fuzz`                                 |
| 型付き fuzzer (`Arbitrary` 駆動)          | `testrs_fuzz::fuzz_typed`                           |
| Fuzz dictionary                            | `FuzzConfig::dictionary`                            |
| Crash 後継続 + 重複排除                    | `FuzzConfig::{continue_after_crash, dedup_by_message}` |
| corpus / crash 永続化                    | `FuzzConfig::{corpus_dir, crash_dir}`               |

## パターン集

### 1. バイト指向ターゲットをファズ

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

### 2. 型付き API をファズ

```rust
use testrs_fuzz::{fuzz_typed, TypedFuzzConfig};

#[test]
fn json_query_never_panics() {
    let report = fuzz_typed::<String, _>(TypedFuzzConfig::default(), |s: &String| {
        let _ = json::query(s);
    });
    assert!(report.failure().is_none());
}
```

`Arbitrary` を実装した任意の型を `fuzz_typed` で駆動できる。
mutator はバイト seed 上で動くため、手書きの `&[u8] → T`
decoder 無しで多様な入力を探索する。

### 3. ファズランから全 distinct crash を回収

```rust
use testrs_fuzz::{fuzz, FuzzConfig};
use std::path::PathBuf;

let report = fuzz(
    FuzzConfig {
        iterations: 100_000,
        dictionary: vec![b"GET ".to_vec(), b"POST ".to_vec(), b"HTTP/1.1".to_vec()],
        continue_after_crash: true,
        dedup_by_message: true,
        corpus_dir: Some(PathBuf::from("target/fuzz-corpus")),
        crash_dir: Some(PathBuf::from("target/fuzz-crashes")),
        ..FuzzConfig::default()
    },
    |bytes: &[u8]| { let _ = http::parse(bytes); },
);
for f in &report.failures {
    eprintln!("crash: {} (input: {} bytes)", f.message, f.input.len());
}
```

`dictionary` は素朴な mutator では到達に永遠を要する複数バイトの
ゲートを越えさせる。`continue_after_crash` + `dedup_by_message` で
ユニークな panic をすべて収集。`crash_dir` には `.bin` 再現ファイルと
対応する `.txt` メタデータが保存される。

## 失敗の再現

fuzzer は PBT ランナーと同じ仕組みで再現用 seed を扱い、環境変数
`TESTRS_FUZZ_SEED` で固定できる:

```
TESTRS_FUZZ_SEED=12345 cargo test parser_does_not_panic
```

## 制約

- fuzzer にはカバレッジフィードバックがない。ランダム / 変異入力で
  panic を捕まえる「スモーク fuzzer」であり、libFuzzer や cargo-fuzz が
  使える場面ではそれらの代替にはならない。
- ターゲットが panic を捕捉するため、プロセスグローバルな panic hook を
  インストールする (安全のため参照カウント済み)。panic hook を別途
  取得 / 設定するコードと並行すると競合し得る。
- `panic = "abort"` プロファイルは非対応。
- corpus / crash の永続化先は `target/` 配下を起点とする。書き込み先の
  解決規則は PBT の regression replay と同じ (`CARGO_TARGET_DIR` →
  `CARGO_MANIFEST_DIR` の順、両方未設定なら黙ってスキップ)。

## テストスイートの実行

```
cargo test -p testrs-fuzz
cargo run --release --example find_crash -p testrs-fuzz   # 意図的に panic するデモ
```

> `find_crash` example は意図的に panic するデモであり、CI / test job には
> 混ぜない。`cargo test --workspace --all-targets` は example の binary build は
> するが run しないため問題ない。

## Contributing

開発手順・規約・設計判断のトレードオフは、リポジトリトップの
[`CONTRIBUTING.md`](../../CONTRIBUTING.md) を参照。
