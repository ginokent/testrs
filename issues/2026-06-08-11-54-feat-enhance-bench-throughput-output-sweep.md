# testrs-bench にデータスループット・機械可読出力・sweep を追加する

- Priority: Medium
- Created: 2026-06-08 11:54 JST
- Completed:
- Branch: feature/add-bench-throughput-csv-sweep

## 目的

testrs-bench を実際に使った別作業からのフィードバックで判明した 3 つの
機能ギャップを埋める。

1. **データスループット (bytes/s)** — `throughput()` は ops/s 専用で、
   codec / 圧縮 / シリアライズ / IO 系の主指標である bytes/s を毎回
   手計算する必要があった (実害が最も大きい)。
2. **機械可読出力 (CSV / JSON)** — サイズ × 設定 × op の表整形を pub
   フィールドから自前で組む必要があった。
3. **sweep (パラメタ化ベンチ)** — 入力サイズ等を振るベンチを手書きループで
   回す必要があった。

## 経緯

`testrs-bench` (PR #32) を別エージェントが codec 系ベンチで使用した際の
報告。なお同報告の「`Statistics` に p25/p75 が無い」は事実誤認 (現行コードに
存在) のため対象外。CSV/JSON と sweep は元 README で意図的スコープ外として
いたが、実需が確認できたため取り込む。

## 完了条件

- `bench_throughput` / `bench_throughput_with` を追加し、`BenchResult` に
  `bytes_per_iter: Option<u64>` メタと `with_bytes_per_iter` /
  `throughput_bytes` を追加する。`format_result` は bytes/s 行を自動出力する
- `BenchResult::to_json` / `to_csv_record` / `csv_header` を依存ゼロの
  手書きで追加する (JSON / CSV エスケープ込み)
- `sweep` / `sweep_with` (`Vec<BenchResult>` を返す) と `format_sweep`
  (表整形) を追加する
- テストとドキュメント (`crates/bench/README.md`、必要なら SPEC) を更新する

## 設計方針

- **throughput は BenchConfig に持たせない**。`BenchConfig` は計測の挙動を
  制御する設定であり、`bytes_per_iter` は計測を一切変えない「結果の解釈
  メタ」である。両者を混ぜると設定の意味論が濁るため、メタは `BenchResult`
  に持たせ、設定は専用 API (`bench_throughput*`) の引数で受ける (criterion の
  `group.throughput()` と同じ関心分離)
- **CSV / JSON は依存ゼロの手書き**。`serde` 等は使わない (no-deps 方針)
- **sweep は薄い層**。`measure_with` を各 param で呼び `Vec<BenchResult>` を
  返すだけ。bytes/s と組み合わせたい場合は結果に `with_bytes_per_iter` を
  map する (sweep 自体に throughput を混ぜない)
- 計測本体は非決定的なため、新機能のテストは決定的な部分 (エスケープ /
  整形 / 件数 / メタ計算) に寄せる
