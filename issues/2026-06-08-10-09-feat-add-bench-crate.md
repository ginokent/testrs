# ベンチマーク用 crate testrs-bench を追加する

- Priority: Medium
- Created: 2026-06-08 10:09 JST
- Completed:
- Model: -
- Branch: feature/add-bench-crate

## 目的

依存ゼロ (std のみ) のマイクロベンチマークハーネスを新しい兄弟
crate `testrs-bench` として追加する。複数プロジェクトで毎回ベンチの
仕組みを自作している現状を、testrs の「外部依存ゼロ」哲学に沿った形で
共通化する。

## 経緯

他プロジェクトで度々ベンチが必要になり、そのたびに各プロジェクトで
仕組みを自作している。criterion はデファクトだが依存が重く
(plotters 等)、「依存を増やしたくない」場面では使いにくい。
PBT (`testrs-pbt`) / fuzzing (`testrs-fuzz`) と同じく依存ゼロの
ベンチを `testrs-*` 一式で揃えられる価値がある。

## 優先度根拠

利用者の実需 (毎回自作している) があり、依存ゼロという testrs の
価値提案にも合致するため Medium とする。

## 現状・問題

- 主用途は「開発時の単発・相対測定」(A vs B の比較、最適化前後の確認)。
  基準値の永続化や CI 退行ゲートは現 MVP の対象外。
- 測定精度は「統計処理を厚く」: 外れ値除去・標準偏差/MAD・変動係数まで
  含める。簡易すぎる median だけの実装は「測れているつもりで測れて
  いない」罠に陥るため避ける。

## 完了条件

- 新 crate `crates/bench` (`testrs-bench`) を workspace に追加する
- `bench` / `bench_with` でクロージャを計測し、結果を人間可読で出力する
- ウォームアップ → 時間ベースの自動反復回数決定 → 複数サンプル収集
- `std::hint::black_box` で最適化除去、`std::time::Instant` で計測
- 統計: mean / median / min / max / std_dev / MAD / 変動係数 (CV) /
  外れ値除去 (Tukey IQR / MAD)
- 相対比較 `bench_compare` (A vs B の median 比を表示)
- 統計関数は `testrs-pbt` を dev-dependency にして PBT で検証する
  (時間依存の本体は非決定的なため最小限の単体テストで確認)
- README (`crates/bench/README.md`) と、トップ README / SPEC /
  CLAUDE / CONTRIBUTING の整合を取る

## 設計方針

- **依存ゼロ** — 直接依存は std のみ。`testrs-core` にも依存しない
  独立カテゴリとする (ベンチに Rng/Arbitrary は不要)。将来 Strategy
  ベースのパラメタライズドベンチが欲しくなれば、その時点で core 依存を
  足す (後方互換不要方針)
- **`unsafe_code = "forbid"`** を遵守する
- **将来余地を切らない** — 以下は現 MVP のスコープ外だが、将来欲しく
  なった際に後付けできる拡張点を設計上ふさがない:
  - 基準値の永続化 / CI 性能退行ゲート → `BenchResult` に全統計を pub で
    保持し、シリアライズしやすい構造にする
  - 自前の機械可読出力 (JSON 等、依存ゼロ) / レポート → 出力ロジックを
    結果構造から分離する
  - 統計的有意差判定 → `BenchResult` に raw サンプル列を pub で残す
- **正確性の限界を明記** — fuzzer 同様、README に「ノイズ補正は最小限。
  criterion の代替ではなく開発時の相対測定用」と正直に書く
