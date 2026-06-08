# testrs-bench

依存ゼロ (std のみ) のマイクロベンチマークハーネス。クロージャの実行時間を
ウォームアップ込みで計測し、複数サンプルから記述統計を求め、外れ値を除去
したうえで人間可読のレポートを出力する。

> ワークスペース **testrs** を構成する一 crate。リポジトリ全体の俯瞰
> (カテゴリ分離、crate 構成) は [リポジトリトップの README](../../README.md)
> を参照。プロパティベーステストは [`testrs-pbt`](../pbt/README.md)、
> fuzzing は [`testrs-fuzz`](../fuzz/README.md) を使う。
>
> **benchmarking は PBT / fuzzing とは別カテゴリ**であり、`testrs-bench` は
> 計測に Rng/Arbitrary を必要としないため **`testrs-core` にも依存せず
> std のみで完結する独立 crate** である。

## 位置づけと限界

開発時の **単発・相対測定**（実装 A vs B、最適化前後の確認）を主目的とする。
OS スケジューリングや CPU 周波数変動に対するノイズ補正は最小限であり、
**criterion の代替ではない**。プロット生成・基準値の永続化・CI 性能退行
ゲート・統計的有意差判定は現状の対象外。ただし [`BenchResult`] が生サンプル
列と全統計を公開しているため、それらは後付けできる（[将来の拡張余地](#将来の拡張余地)参照）。

```toml
[dev-dependencies]
testrs-bench = { git = "https://github.com/ginokent/testrs", package = "testrs-bench" }
```

## クイックスタート

```rust
use testrs_bench::{bench, bench_compare};

// 単発の計測。結果を標準エラー出力へ表示しつつ BenchResult を返す。
let result = bench("vec push 0..1000", || {
    let mut v = Vec::new();
    for i in 0..1000u32 {
        v.push(i);
    }
    v
});
println!("median = {} ns/iter", result.stats.median);

// 2 実装の相対比較。両者の計測値と「A is N.Nx faster than B」を表示する。
bench_compare(
    "sort_unstable",
    || { let mut v = vec![3, 1, 2]; v.sort_unstable(); v },
    "sort",
    || { let mut v = vec![3, 1, 2]; v.sort(); v },
);
```

出力例:

```
bench: vec push 0..1000
  time:    [612.00 ns 640.00 ns 1.21 µs]  (min median max)
  mean:    668.40 ns ± 92.10 ns (cv 13.8%)
  thrpt:   1.50 Mops/s
  samples: 100 (3 outliers removed, 4096 iters/sample)
```

## 同梱機能

| 機能                                   | 場所                                        |
|----------------------------------------|---------------------------------------------|
| 単発計測 (出力あり)                    | `testrs_bench::{bench, bench_with}`         |
| 単発計測 (出力なし)                    | `testrs_bench::{measure, measure_with}`     |
| データスループット計測 (bytes/s 込み)  | `testrs_bench::{bench_throughput, bench_throughput_with}` |
| 相対比較                               | `testrs_bench::{bench_compare, bench_compare_with}` |
| sweep (パラメタ化ベンチ)               | `testrs_bench::{sweep, sweep_with}`         |
| 実行設定                               | `testrs_bench::BenchConfig`                 |
| 外れ値除去方式                         | `testrs_bench::OutlierFilter` (`None` / `Tukey` / `Mad`) |
| 結果 (生サンプル + 全統計)             | `testrs_bench::BenchResult`                 |
| 記述統計                               | `testrs_bench::Statistics`                  |
| スループット (ops/s)                   | `BenchResult::throughput()`                 |
| データスループット (bytes/s)           | `BenchResult::{with_bytes_per_iter, throughput_bytes}` |
| 比較比率                               | `BenchComparison::ratio()`                  |
| 出力整形 (結果構造から分離)            | `testrs_bench::{format_result, format_comparison, format_sweep}` |
| 機械可読出力 (依存ゼロ手書き)          | `BenchResult::{to_json, to_csv_record}` / `BenchResult::csv_header()` |
| 統計関数 (mean/median/std_dev/MAD/percentile) | `testrs_bench::stats::*`             |

## パターン集

### 1. 単発の計測

```rust
use testrs_bench::bench;

bench("parse 1KB json", || {
    let input = include_str!("../fixtures/sample.json");
    my_json::parse(std::hint::black_box(input))
});
```

クロージャの **返り値は自動で `black_box` を通る**ため、結果が使われずに
最適化で消えることはない。一方、**入力側は呼び出し側で `black_box`** に
通すこと（上の `black_box(input)`）。さもないと定数畳み込みで計測対象が
消える場合がある。

### 2. 2 実装の相対比較

```rust
use std::hint::black_box;
use testrs_bench::bench_compare;

let data: Vec<i32> = (0..10_000).rev().collect();
let comparison = bench_compare(
    "sort_unstable",
    || { let mut v = black_box(data.clone()); v.sort_unstable(); v },
    "sort",
    || { let mut v = black_box(data.clone()); v.sort(); v },
);
// median 比をプログラムから参照する。
println!("ratio (b/a) = {:.2}", comparison.ratio());
```

### 3. 実行設定の調整

```rust
use std::time::Duration;
use testrs_bench::{bench_with, BenchConfig, OutlierFilter};

let cfg = BenchConfig {
    warmup_time: Duration::from_millis(200),
    measurement_time: Duration::from_secs(5),
    min_samples: 30,
    max_samples: 200,
    outlier_filter: OutlierFilter::Mad,
};
bench_with("hot path", &cfg, || my_lib::hot_path());
```

ハーネスはまず `warmup_time` だけ空回しし、続いて「1 サンプルの計測が
目標バッチ時間以上になる」反復回数を倍々で自動決定する（タイマー分解能の
ノイズを抑えるため、目標バッチ時間には下限 1µs を設けている）。その後、
`measurement_time` に達するか `max_samples` に達するまでサンプルを集める
（`min_samples` までは時間超過しても収集する）。

### 4. 外れ値除去方式の選択

| 方式               | 判定                                                   | 向き                        |
|--------------------|--------------------------------------------------------|-----------------------------|
| `OutlierFilter::None`  | 除去しない                                          | 生の分布を見たいとき        |
| `OutlierFilter::Tukey` | Q1−1.5·IQR 〜 Q3+1.5·IQR の外を除去 (デフォルト)     | 一般用途                    |
| `OutlierFilter::Mad`   | 中央値からの偏差が 3·(1.4826·MAD) 超を除去           | 裾の重い分布に頑健          |

散らばりが 0（または極小）で全件が外れ値になる病的ケースでは、統計を
計算できるよう入力をそのまま残す。

### 5. 出力せずに結果だけ取得する

```rust
use testrs_bench::measure;

// measure / measure_with は何も出力せず BenchResult を返す。
// 自前の整形・集計・永続化を行いたいときに使う。
let r = measure("encode", || my_codec::encode(&payload));
let row = format!("{},{},{}", r.name, r.stats.median, r.outliers_removed);
std::fs::write("bench.csv", row).unwrap();
```

### 6. 出力を自前で整形する

```rust
use testrs_bench::{measure, format_result};

let r = measure("decode", || my_codec::decode(&bytes));
// デフォルト整形を使う。
eprintln!("{}", format_result(&r));
// もしくは BenchResult の pub フィールドから独自フォーマットを組む。
eprintln!("{}: p25={} p75={}", r.name, r.stats.p25, r.stats.p75);
```

### 7. データスループット (bytes/s)

codec / 圧縮 / シリアライズ / IO 系では bytes/s が主指標になる。1 反復あたりの
処理バイト数を渡すと、出力に `data` 行 (bytes/s) が自動で加わる。

```rust
use std::hint::black_box;
use testrs_bench::bench_throughput;

let input = vec![0u8; 64 * 1024];
let r = bench_throughput("decode 64KiB", input.len() as u64, || {
    my_codec::decode(black_box(&input))
});
// プログラムからも取得できる (未設定なら None)。
println!("{:?} bytes/s", r.throughput_bytes());
```

**設計上の注意**: 処理バイト数は計測の挙動を変えない「結果の解釈メタ」なので、
`BenchConfig` ではなく **結果側** に持たせている (`BenchResult::bytes_per_iter`)。
`measure` 系の結果には `r.with_bytes_per_iter(n)` で後付けできる。

### 8. sweep (入力サイズなどを振る)

同じベンチをパラメータ列で回し、`Vec<BenchResult>` を得る。ラベルは
`"{name}/{param}"`。`format_sweep` で桁揃えの表に整形できる。

```rust
use std::hint::black_box;
use testrs_bench::{sweep, format_sweep, BenchResult};

let sizes = [1024usize, 4096, 16384];
let results: Vec<BenchResult> = sweep("decode", &sizes, |&n| {
    let input = vec![0u8; n];
    my_codec::decode(black_box(&input))
})
.into_iter()
.zip(sizes)
.map(|(r, n)| r.with_bytes_per_iter(n as u64)) // 各サイズを bytes/s メタに
.collect();
eprintln!("{}", format_sweep(&results));
```

`sweep` 自体は throughput を扱わない（関心を混ぜない）。bytes/s を併記したい
場合は上のように結果へ `with_bytes_per_iter` を map する。

> 入力を **型情報から生成** するパラメタ化 (`Strategy` 連携) は現状未対応。
> 固定パラメータ列の sweep のみをサポートする。

### 9. 機械可読出力 (CSV / JSON)

依存ゼロの手書き出力。表集計や外部ツール連携に使う。

```rust
use testrs_bench::{measure, BenchResult};

let results = [measure("a", || work_a()), measure("b", || work_b())];

// CSV: ヘッダ + 1 結果 1 行。
let mut csv = String::from(BenchResult::csv_header());
for r in &results {
    csv.push('\n');
    csv.push_str(&r.to_csv_record());
}
std::fs::write("bench.csv", csv).unwrap();

// JSON: 1 結果 1 オブジェクト (生サンプル列 samples_ns 込み)。
for r in &results {
    println!("{}", r.to_json());
}
```

`to_csv_record` は集計値のみ（生サンプルは含まない）。`to_json` は生サンプル
列まで含む完全な結果。名前は CSV (RFC 4180) / JSON それぞれの規則で
エスケープされる。

## 出力の読み方

- **time `[min median max]`** — サンプル分布の代表値。中央値は外れ値に
  頑健なため、相対比較の主軸として `ratio()` も中央値ベース。
- **mean ± std_dev (cv)** — 平均と標本標準偏差、変動係数 (cv = std/mean)。
  **cv が大きい（目安: 10% 超）ほど計測が不安定**で、結果を鵜呑みに
  できない。値を上げて再計測するか、環境ノイズ（他プロセス、周波数
  スケーリング）を疑うこと。
- **thrpt** — 1 秒あたりの処理回数 (ops/s)。
- **data** — 1 秒あたりの処理バイト数 (bytes/s)。`bench_throughput` などで
  処理バイト数を渡したときのみ表示される。
- **samples** — 母数・除去された外れ値数・1 サンプルあたりの反復回数。

## 計測を安定させるには

- `--release` で計測する（デバッグビルドの数値は無意味）。
- ノートPCなら電源接続・性能モードにし、計測中は他の重い処理を避ける。
- cv が高いときは `measurement_time` / `max_samples` を増やす。
- マイクロベンチは「相対比較」に使う。**絶対値は環境依存**であり、
  別マシンの数値とは比較しないこと。

## 制約

- **カバレッジや統計的検定は持たない**。criterion / divan が使える場面で
  それらの代替にはならない。「依存を増やさず手元で相対比較したい」用途に
  特化している。
- 計測は本質的に非決定的で、ノイズ補正は最小限。同一コードでも実行ごとに
  数値はぶれる。
- `testrs-core` を含む他の testrs crate には依存しない。固定パラメータの
  sweep は持つが、入力を **型情報から生成** する (`Strategy` 連携) ベンチは
  現状サポートしない（将来 core 依存を足して対応しうる）。
- `panic = "abort"` プロファイルでは、ベンチ対象が panic するとプロセスごと
  落ちる（ハーネスは panic を捕捉しない）。

## 将来の拡張余地

現 MVP のスコープ外だが、後付けできるよう設計上ふさいでいない:

- **基準値の永続化 / CI 性能退行ゲート** — `BenchResult` が全統計を pub で
  保持し、`to_json` / `to_csv_record` で機械可読化もできるため、結果を保存
  して閾値比較する層を上に重ねられる。
- **統計的有意差判定** — `BenchResult::samples` に生サンプル列を残してあり、
  Mann–Whitney U などの検定を後付けできる。
- **`Strategy` 連携の sweep** — 現状の sweep は固定パラメータ列のみ。型情報
  から入力を生成する版は、`testrs-core` への依存を足して対応しうる。

## テストスイートの実行

```
cargo test -p testrs-bench
cargo run --release --example compare_sorts -p testrs-bench
cargo run --release --example throughput_sweep -p testrs-bench
```

統計関数の数学的性質は `tests/stats_pbt.rs` で `testrs-pbt` を用いた PBT で
検証している（時間依存の計測本体は非決定的なため、構造的な不変条件のみを
単体テストで確認する）。

## Contributing

開発手順・規約・設計判断のトレードオフは、リポジトリトップの
[`CONTRIBUTING.md`](../../CONTRIBUTING.md) を参照。
