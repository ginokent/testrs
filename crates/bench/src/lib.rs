//! 依存ゼロ (std のみ) のマイクロベンチマークハーネスです。
//!
//! クロージャの実行時間をウォームアップ込みで計測し、複数サンプルから
//! 記述統計 (中央値・平均・標準偏差・MAD・変動係数) を求め、外れ値を
//! 除去したうえで人間可読のレポートを出力します。`std::hint::black_box`
//! で最適化による消去を防ぎます。
//!
//! # 位置づけと限界
//!
//! 開発時の **単発・相対測定** (実装 A vs B、最適化前後の確認) を主目的と
//! します。OS スケジューリングや CPU 周波数変動に対するノイズ補正は最小限
//! であり、**criterion の代替ではありません**。基準値の永続化や CI 性能
//! 退行ゲートは現状の対象外ですが、[`BenchResult`] が生サンプルと全統計を
//! 公開しているため、それらは後付けできます。
//!
//! # 例
//!
//! ```no_run
//! use testrs_bench::{bench, bench_compare};
//!
//! // 単発の計測。
//! let result = bench("vec push 1k", || {
//!     let mut v = Vec::new();
//!     for i in 0..1000u32 {
//!         v.push(i);
//!     }
//!     v
//! });
//! println!("median: {} ns", result.stats.median);
//!
//! // 2 実装の相対比較。
//! bench_compare(
//!     "sort_unstable",
//!     || { let mut v = vec![3, 1, 2]; v.sort_unstable(); v },
//!     "sort",
//!     || { let mut v = vec![3, 1, 2]; v.sort(); v },
//! );
//! ```

use std::hint::black_box;
use std::time::{Duration, Instant};

pub mod stats;
pub use stats::{OutlierFilter, Statistics};

/// ベンチマーク実行の調整可能パラメータです。
#[derive(Debug, Clone)]
pub struct BenchConfig {
    /// 計測前にターゲットを空回しするウォームアップ時間です。
    /// CPU 周波数やキャッシュを定常状態に近づけます。
    pub warmup_time: Duration,
    /// サンプル収集に費やす目標時間です。この時間に達するか
    /// `max_samples` に達するまでサンプルを集めます。
    pub measurement_time: Duration,
    /// 収集するサンプルの最小数です。`measurement_time` を超過しても
    /// 最低この数までは収集します。
    pub min_samples: usize,
    /// 収集するサンプルの最大数です。
    pub max_samples: usize,
    /// 統計を取る前に適用する外れ値除去方式です。
    pub outlier_filter: OutlierFilter,
}

impl Default for BenchConfig {
    fn default() -> Self {
        Self {
            warmup_time: Duration::from_millis(500),
            measurement_time: Duration::from_secs(3),
            min_samples: 10,
            max_samples: 100,
            outlier_filter: OutlierFilter::Tukey,
        }
    }
}

/// 1 回のベンチマーク計測の結果です。
///
/// 生サンプル列と全統計を公開し、将来の永続化 / 有意差判定が
/// この構造体だけで完結できるようにしています。
#[derive(Debug, Clone)]
pub struct BenchResult {
    /// ベンチマーク名です。
    pub name: String,
    /// 1 反復あたりのナノ秒で表した生サンプル列です (外れ値除去前)。
    pub samples: Vec<f64>,
    /// 1 サンプルを計測するために連続実行した反復回数です。
    pub iters_per_sample: u64,
    /// 計測に費やした総反復回数です。
    pub total_iterations: u64,
    /// 外れ値除去で取り除かれたサンプル数です。
    pub outliers_removed: usize,
    /// 外れ値除去後のサンプルから計算した統計です。
    pub stats: Statistics,
    /// 1 反復あたりが処理するバイト数です (データスループット算出用のメタ)。
    ///
    /// 計測の挙動には一切影響しません。`None` のとき bytes/s は算出されません。
    /// 計測の制御 (`BenchConfig`) と「結果の解釈メタ」を分離するため、この値は
    /// 設定ではなく結果に持たせています。
    pub bytes_per_iter: Option<u64>,
}

impl BenchResult {
    /// 1 秒あたりの処理回数 (ops/s) です。平均が 0 のときは 0 です。
    pub fn throughput(&self) -> f64 {
        if self.stats.mean > 0.0 {
            1e9 / self.stats.mean
        } else {
            0.0
        }
    }

    /// この結果に「1 反復あたりの処理バイト数」を関連付けます。
    ///
    /// 計測値は変えず、データスループット (bytes/s) の算出と出力のみを
    /// 有効化します。`measure` / `measure_with` の結果に後付けする用途です。
    pub fn with_bytes_per_iter(mut self, bytes_per_iter: u64) -> Self {
        self.bytes_per_iter = Some(bytes_per_iter);
        self
    }

    /// 1 秒あたりの処理バイト数 (bytes/s) です。
    ///
    /// `bytes_per_iter` が未設定、または平均が 0 のときは `None` です。
    pub fn throughput_bytes(&self) -> Option<f64> {
        let bytes = self.bytes_per_iter?;
        if self.stats.mean > 0.0 {
            Some(bytes as f64 * 1e9 / self.stats.mean)
        } else {
            None
        }
    }

    /// 結果を 1 行の JSON オブジェクトに整形します (依存ゼロの手書き)。
    ///
    /// 生サンプル列 (`samples_ns`) を含む完全な結果を出力します。非有限値は
    /// JSON に表現できないため `null` になります。
    pub fn to_json(&self) -> String {
        let s = &self.stats;
        let bytes_per_iter = match self.bytes_per_iter {
            Some(b) => b.to_string(),
            None => "null".to_string(),
        };
        let throughput_bytes = match self.throughput_bytes() {
            Some(v) => fmt_json_num(v),
            None => "null".to_string(),
        };
        let samples: Vec<String> = self.samples.iter().map(|x| fmt_json_num(*x)).collect();
        format!(
            concat!(
                "{{\"name\":{name},\"n\":{n},\"mean_ns\":{mean},\"median_ns\":{median},",
                "\"min_ns\":{min},\"max_ns\":{max},\"std_dev_ns\":{std},\"mad_ns\":{mad},",
                "\"cv\":{cv},\"p25_ns\":{p25},\"p75_ns\":{p75},\"outliers_removed\":{out},",
                "\"iters_per_sample\":{ips},\"total_iterations\":{ti},",
                "\"bytes_per_iter\":{bpi},\"throughput_ops\":{tops},",
                "\"throughput_bytes\":{tbytes},\"samples_ns\":[{samples}]}}"
            ),
            name = json_string(&self.name),
            n = s.n,
            mean = fmt_json_num(s.mean),
            median = fmt_json_num(s.median),
            min = fmt_json_num(s.min),
            max = fmt_json_num(s.max),
            std = fmt_json_num(s.std_dev),
            mad = fmt_json_num(s.mad),
            cv = fmt_json_num(s.cv),
            p25 = fmt_json_num(s.p25),
            p75 = fmt_json_num(s.p75),
            out = self.outliers_removed,
            ips = self.iters_per_sample,
            ti = self.total_iterations,
            bpi = bytes_per_iter,
            tops = fmt_json_num(self.throughput()),
            tbytes = throughput_bytes,
            samples = samples.join(","),
        )
    }

    /// 結果を 1 行の CSV レコードに整形します。
    ///
    /// 列順は [`BenchResult::csv_header`] と一致します。集計値のみで生サンプル
    /// 列は含みません。数値は接頭辞なしの生値、`bytes_per_iter` /
    /// `throughput_bytes` が未設定のときは空フィールドになります。
    pub fn to_csv_record(&self) -> String {
        let s = &self.stats;
        let bpi = self
            .bytes_per_iter
            .map(|b| b.to_string())
            .unwrap_or_default();
        let tbytes = self
            .throughput_bytes()
            .map(|v| format!("{v}"))
            .unwrap_or_default();
        format!(
            "{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{}",
            csv_field(&self.name),
            s.n,
            s.mean,
            s.median,
            s.min,
            s.max,
            s.std_dev,
            s.mad,
            s.cv,
            s.p25,
            s.p75,
            self.outliers_removed,
            self.iters_per_sample,
            self.total_iterations,
            bpi,
            self.throughput(),
            tbytes,
        )
    }

    /// [`BenchResult::to_csv_record`] に対応する CSV ヘッダ行です。
    pub fn csv_header() -> &'static str {
        "name,n,mean_ns,median_ns,min_ns,max_ns,std_dev_ns,mad_ns,cv,p25_ns,p75_ns,\
outliers_removed,iters_per_sample,total_iterations,bytes_per_iter,throughput_ops,throughput_bytes"
    }
}

/// 2 つのベンチマーク結果の比較です。
#[derive(Debug, Clone)]
pub struct BenchComparison {
    /// 1 つ目の結果です。
    pub a: BenchResult,
    /// 2 つ目の結果です。
    pub b: BenchResult,
}

impl BenchComparison {
    /// `b.median / a.median` です。1 より大きいほど `a` が速いことを意味します。
    pub fn ratio(&self) -> f64 {
        let (ma, mb) = (self.a.stats.median, self.b.stats.median);
        if ma > 0.0 {
            mb / ma
        } else {
            f64::INFINITY
        }
    }
}

/// デフォルト設定で `f` を計測し、結果を標準エラー出力へ表示します。
pub fn bench<F, O>(name: &str, f: F) -> BenchResult
where
    F: FnMut() -> O,
{
    bench_with(name, &BenchConfig::default(), f)
}

/// `cfg` に基づいて `f` を計測し、結果を標準エラー出力へ表示します。
pub fn bench_with<F, O>(name: &str, cfg: &BenchConfig, f: F) -> BenchResult
where
    F: FnMut() -> O,
{
    let result = measure_with(name, cfg, f);
    eprintln!("{}", format_result(&result));
    result
}

/// 1 反復あたり `bytes_per_iter` バイトを処理するクロージャをデフォルト設定で
/// 計測し、データスループット込みの結果を表示します。
pub fn bench_throughput<F, O>(name: &str, bytes_per_iter: u64, f: F) -> BenchResult
where
    F: FnMut() -> O,
{
    bench_throughput_with(name, &BenchConfig::default(), bytes_per_iter, f)
}

/// `cfg` に基づき、1 反復あたり `bytes_per_iter` バイトを処理するクロージャを
/// 計測し、データスループット込みの結果を表示します。
pub fn bench_throughput_with<F, O>(
    name: &str,
    cfg: &BenchConfig,
    bytes_per_iter: u64,
    f: F,
) -> BenchResult
where
    F: FnMut() -> O,
{
    let result = measure_with(name, cfg, f).with_bytes_per_iter(bytes_per_iter);
    eprintln!("{}", format_result(&result));
    result
}

/// デフォルト設定で `f` を計測しますが、何も出力しません。
///
/// 出力を自前で整形したい場合や、結果を永続化したい場合に使います。
pub fn measure<F, O>(name: &str, f: F) -> BenchResult
where
    F: FnMut() -> O,
{
    measure_with(name, &BenchConfig::default(), f)
}

/// `cfg` に基づいて `f` を計測しますが、何も出力しません。
pub fn measure_with<F, O>(name: &str, cfg: &BenchConfig, mut f: F) -> BenchResult
where
    F: FnMut() -> O,
{
    // ウォームアップ: 指定時間だけ空回しする。
    if cfg.warmup_time > Duration::ZERO {
        let warm_start = Instant::now();
        while warm_start.elapsed() < cfg.warmup_time {
            black_box(f());
        }
    }

    // キャリブレーション: 1 サンプルの計測が目標バッチ時間以上になるよう
    // 反復回数を倍々で探索する。タイマー分解能ノイズを抑えるため、
    // 目標バッチ時間には下限 (1µs) を設ける。
    let target_batch =
        (cfg.measurement_time / cfg.max_samples.max(1) as u32).max(Duration::from_micros(1));
    let iters_per_sample = calibrate(&mut f, target_batch);

    // サンプル収集: measurement_time に達するか max_samples に達するまで。
    // ただし min_samples までは時間超過しても収集する。
    let mut samples: Vec<f64> = Vec::with_capacity(cfg.max_samples);
    let m_start = Instant::now();
    while samples.len() < cfg.max_samples
        && (samples.len() < cfg.min_samples || m_start.elapsed() < cfg.measurement_time)
    {
        let elapsed = time_batch(&mut f, iters_per_sample);
        let ns_per_iter = elapsed.as_nanos() as f64 / iters_per_sample as f64;
        samples.push(ns_per_iter);
    }

    let total_iterations = samples.len() as u64 * iters_per_sample;
    let (kept, outliers_removed) = stats::filter_outliers(&samples, cfg.outlier_filter);
    let statistics = Statistics::from_samples(&kept).expect("at least one sample collected");

    BenchResult {
        name: name.to_string(),
        samples,
        iters_per_sample,
        total_iterations,
        outliers_removed,
        stats: statistics,
        bytes_per_iter: None,
    }
}

/// 2 つのクロージャをデフォルト設定で計測し、比較結果を表示します。
pub fn bench_compare<FA, OA, FB, OB>(
    a_name: &str,
    a_fn: FA,
    b_name: &str,
    b_fn: FB,
) -> BenchComparison
where
    FA: FnMut() -> OA,
    FB: FnMut() -> OB,
{
    bench_compare_with(&BenchConfig::default(), a_name, a_fn, b_name, b_fn)
}

/// 2 つのクロージャを `cfg` で計測し、比較結果を表示します。
pub fn bench_compare_with<FA, OA, FB, OB>(
    cfg: &BenchConfig,
    a_name: &str,
    a_fn: FA,
    b_name: &str,
    b_fn: FB,
) -> BenchComparison
where
    FA: FnMut() -> OA,
    FB: FnMut() -> OB,
{
    let a = measure_with(a_name, cfg, a_fn);
    let b = measure_with(b_name, cfg, b_fn);
    let comparison = BenchComparison { a, b };
    eprintln!("{}", format_result(&comparison.a));
    eprintln!("{}", format_result(&comparison.b));
    eprintln!("{}", format_comparison(&comparison));
    comparison
}

/// パラメータ列の各要素についてデフォルト設定でベンチを実行し、結果を集めます。
///
/// 各結果のラベルは `"{name}/{param}"` になります。返り値は [`format_sweep`]
/// で表形式に整形できます。データスループットを併記したい場合は、各結果に
/// [`BenchResult::with_bytes_per_iter`] を map してください
/// (sweep 自体には throughput を混ぜません)。
pub fn sweep<P, F, O>(name: &str, params: &[P], run: F) -> Vec<BenchResult>
where
    P: std::fmt::Display,
    F: FnMut(&P) -> O,
{
    sweep_with(name, &BenchConfig::default(), params, run)
}

/// `cfg` に基づいて [`sweep`] を行います。
pub fn sweep_with<P, F, O>(
    name: &str,
    cfg: &BenchConfig,
    params: &[P],
    mut run: F,
) -> Vec<BenchResult>
where
    P: std::fmt::Display,
    F: FnMut(&P) -> O,
{
    params
        .iter()
        .map(|p| {
            let label = format!("{name}/{p}");
            measure_with(&label, cfg, || run(p))
        })
        .collect()
}

/// sweep の結果 (複数の [`BenchResult`]) を桁揃えした表に整形します。
///
/// 列は name / median / thrpt (ops/s) で、いずれかの結果が
/// [`BenchResult::throughput_bytes`] を持つ場合は data (bytes/s) 列が加わります。
pub fn format_sweep(results: &[BenchResult]) -> String {
    if results.is_empty() {
        return String::new();
    }
    let has_bytes = results.iter().any(|r| r.throughput_bytes().is_some());
    // 桁揃えのため、各セルの文字列を先に作る。
    let rows: Vec<(String, String, String, String)> = results
        .iter()
        .map(|r| {
            (
                r.name.clone(),
                fmt_ns(r.stats.median),
                fmt_thrpt(r.throughput()),
                r.throughput_bytes()
                    .map(fmt_bytes_per_sec)
                    .unwrap_or_default(),
            )
        })
        .collect();
    // 幅は char 数で測る (µ などのマルチバイト文字を 1 幅として扱う)。
    let wn = rows
        .iter()
        .map(|x| x.0.chars().count())
        .max()
        .unwrap_or(0)
        .max(4);
    let wm = rows
        .iter()
        .map(|x| x.1.chars().count())
        .max()
        .unwrap_or(0)
        .max(6);
    let wo = rows
        .iter()
        .map(|x| x.2.chars().count())
        .max()
        .unwrap_or(0)
        .max(5);
    let mut out = String::new();
    if has_bytes {
        let wd = rows
            .iter()
            .map(|x| x.3.chars().count())
            .max()
            .unwrap_or(0)
            .max(4);
        out.push_str(&format!(
            "{:<wn$}  {:>wm$}  {:>wo$}  {:>wd$}\n",
            "name", "median", "thrpt", "data"
        ));
        for (n, m, o, d) in &rows {
            out.push_str(&format!("{n:<wn$}  {m:>wm$}  {o:>wo$}  {d:>wd$}\n"));
        }
    } else {
        out.push_str(&format!(
            "{:<wn$}  {:>wm$}  {:>wo$}\n",
            "name", "median", "thrpt"
        ));
        for (n, m, o, _) in &rows {
            out.push_str(&format!("{n:<wn$}  {m:>wm$}  {o:>wo$}\n"));
        }
    }
    out.pop(); // 末尾の改行を除く
    out
}

// --- 計測のコア ---------------------------------------------------

/// `iters` 回 `f` を実行するのにかかった時間を返します。
/// 返り値を `black_box` に通して死コード除去を防ぎます。
fn time_batch<F, O>(f: &mut F, iters: u64) -> Duration
where
    F: FnMut() -> O,
{
    let start = Instant::now();
    for _ in 0..iters {
        black_box(f());
    }
    start.elapsed()
}

/// 1 バッチが `target` 以上の時間になるような反復回数を倍々で求めます。
fn calibrate<F, O>(f: &mut F, target: Duration) -> u64
where
    F: FnMut() -> O,
{
    const MAX_ITERS: u64 = 1 << 30;
    let mut iters: u64 = 1;
    loop {
        let elapsed = time_batch(f, iters);
        if elapsed >= target || iters >= MAX_ITERS {
            return iters.max(1);
        }
        // 実測から目標到達に必要な倍率を見積もる (最低 2 倍)。
        let factor = if elapsed.as_nanos() == 0 {
            2
        } else {
            ((target.as_nanos() / elapsed.as_nanos()) as u64).max(2)
        };
        iters = iters.saturating_mul(factor);
    }
}

// --- 出力整形 (結果構造から分離) -----------------------------------

/// 単一の結果を人間可読の複数行文字列に整形します。
pub fn format_result(r: &BenchResult) -> String {
    let s = &r.stats;
    let mut out = String::new();
    out.push_str(&format!("bench: {}\n", r.name));
    out.push_str(&format!(
        "  time:    [{} {} {}]  (min median max)\n",
        fmt_ns(s.min),
        fmt_ns(s.median),
        fmt_ns(s.max),
    ));
    out.push_str(&format!(
        "  mean:    {} ± {} (cv {:.1}%)\n",
        fmt_ns(s.mean),
        fmt_ns(s.std_dev),
        s.cv * 100.0,
    ));
    out.push_str(&format!("  thrpt:   {}\n", fmt_thrpt(r.throughput())));
    if let Some(bps) = r.throughput_bytes() {
        out.push_str(&format!("  data:    {}\n", fmt_bytes_per_sec(bps)));
    }
    out.push_str(&format!(
        "  samples: {} ({} outliers removed, {} iters/sample)",
        s.n, r.outliers_removed, r.iters_per_sample,
    ));
    out
}

/// 比較結果を人間可読の 1 行文字列に整形します。
pub fn format_comparison(c: &BenchComparison) -> String {
    let (ma, mb) = (c.a.stats.median, c.b.stats.median);
    let (faster, slower, ratio) = if ma <= mb {
        let r = if ma > 0.0 { mb / ma } else { f64::INFINITY };
        (&c.a.name, &c.b.name, r)
    } else {
        let r = if mb > 0.0 { ma / mb } else { f64::INFINITY };
        (&c.b.name, &c.a.name, r)
    };
    format!(
        "comparison: {} is {:.2}x faster than {} (median {} vs {})",
        faster,
        ratio,
        slower,
        fmt_ns(ma.min(mb)),
        fmt_ns(ma.max(mb)),
    )
}

/// ナノ秒を ns / µs / ms / s に丸めて表示します。
fn fmt_ns(ns: f64) -> String {
    if ns < 1e3 {
        format!("{ns:.2} ns")
    } else if ns < 1e6 {
        format!("{:.2} µs", ns / 1e3)
    } else if ns < 1e9 {
        format!("{:.2} ms", ns / 1e6)
    } else {
        format!("{:.2} s", ns / 1e9)
    }
}

/// ops/s を K / M / G の接頭辞付きで表示します。
fn fmt_thrpt(ops: f64) -> String {
    if ops < 1e3 {
        format!("{ops:.2} ops/s")
    } else if ops < 1e6 {
        format!("{:.2} Kops/s", ops / 1e3)
    } else if ops < 1e9 {
        format!("{:.2} Mops/s", ops / 1e6)
    } else {
        format!("{:.2} Gops/s", ops / 1e9)
    }
}

/// bytes/s を 2 進接頭辞 (B/s, KiB/s, MiB/s, GiB/s) で表示します。
fn fmt_bytes_per_sec(bps: f64) -> String {
    const KIB: f64 = 1024.0;
    const MIB: f64 = KIB * 1024.0;
    const GIB: f64 = MIB * 1024.0;
    if bps < KIB {
        format!("{bps:.2} B/s")
    } else if bps < MIB {
        format!("{:.2} KiB/s", bps / KIB)
    } else if bps < GIB {
        format!("{:.2} MiB/s", bps / MIB)
    } else {
        format!("{:.2} GiB/s", bps / GIB)
    }
}

/// f64 を JSON 数値に整形します。非有限値は JSON で表現できないため `null` です。
fn fmt_json_num(x: f64) -> String {
    if x.is_finite() {
        format!("{x}")
    } else {
        "null".to_string()
    }
}

/// 文字列を JSON 文字列リテラル (両端のダブルクォート込み) に整形します。
fn json_string(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 2);
    out.push('"');
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if (c as u32) < 0x20 => out.push_str(&format!("\\u{:04x}", c as u32)),
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

/// 文字列を RFC 4180 の CSV フィールドに整形します。
///
/// カンマ / ダブルクォート / 改行を含む場合のみダブルクォートで囲み、
/// 内部の `"` を `""` に二重化します。
fn csv_field(s: &str) -> String {
    if s.contains([',', '"', '\n', '\r']) {
        format!("\"{}\"", s.replace('"', "\"\""))
    } else {
        s.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // 時間依存のため計測値そのものは非決定的。ここでは構造的な不変条件
    // (サンプルが集まる / 統計が有限 / 反復数が整合する / panic しない)
    // のみを検証する。統計の数学的性質は tests/stats_pbt.rs で PBT 検証する。

    fn fast_cfg() -> BenchConfig {
        BenchConfig {
            warmup_time: Duration::from_millis(1),
            measurement_time: Duration::from_millis(20),
            min_samples: 5,
            max_samples: 10,
            outlier_filter: OutlierFilter::Tukey,
        }
    }

    #[test]
    fn collects_samples_and_computes_finite_stats() {
        let r = measure_with("noop", &fast_cfg(), || 1u64 + 1);
        assert!(r.stats.n >= 1);
        assert!(!r.samples.is_empty());
        assert!(r.stats.mean.is_finite());
        assert!(r.stats.median.is_finite());
        assert!(r.stats.std_dev.is_finite());
        assert!(r.stats.min <= r.stats.median);
        assert!(r.stats.median <= r.stats.max);
    }

    #[test]
    fn total_iterations_matches_samples_times_iters() {
        let r = measure_with("mul", &fast_cfg(), || 7u64 * 3);
        assert_eq!(
            r.total_iterations,
            r.samples.len() as u64 * r.iters_per_sample
        );
        assert!(r.iters_per_sample >= 1);
    }

    #[test]
    fn respects_max_samples() {
        let cfg = BenchConfig {
            max_samples: 7,
            ..fast_cfg()
        };
        let r = measure_with("bounded", &cfg, || 1u64);
        assert!(r.samples.len() <= 7);
    }

    #[test]
    fn comparison_ratio_is_finite_and_positive() {
        let c = bench_compare_with(
            &fast_cfg(),
            "a",
            || (0..50u64).sum::<u64>(),
            "b",
            || (0..100u64).sum::<u64>(),
        );
        assert!(c.ratio().is_finite());
        assert!(c.ratio() > 0.0);
    }

    #[test]
    fn throughput_is_consistent_with_mean() {
        let r = measure_with("thrpt", &fast_cfg(), || 1u64);
        if r.stats.mean > 0.0 {
            let expected = 1e9 / r.stats.mean;
            assert!((r.throughput() - expected).abs() < 1e-3);
        }
    }

    #[test]
    fn format_result_contains_name_and_sections() {
        let r = measure_with("my_bench", &fast_cfg(), || 1u64);
        let s = format_result(&r);
        assert!(s.contains("my_bench"));
        assert!(s.contains("time:"));
        assert!(s.contains("mean:"));
        assert!(s.contains("thrpt:"));
        assert!(s.contains("samples:"));
    }

    #[test]
    fn throughput_bytes_is_consistent_with_mean() {
        let r = measure_with("io", &fast_cfg(), || 1u64).with_bytes_per_iter(1024);
        let tb = r.throughput_bytes().expect("bytes_per_iter is set");
        if r.stats.mean > 0.0 {
            assert!((tb - 1024.0 * 1e9 / r.stats.mean).abs() < 1e-3);
        }
    }

    #[test]
    fn throughput_bytes_is_none_without_meta() {
        let r = measure_with("io", &fast_cfg(), || 1u64);
        assert!(r.bytes_per_iter.is_none());
        assert!(r.throughput_bytes().is_none());
    }

    #[test]
    fn bench_throughput_sets_meta_and_emits_data_line() {
        let r = bench_throughput_with("io", &fast_cfg(), 1024, || 1u64);
        assert_eq!(r.bytes_per_iter, Some(1024));
        assert!(format_result(&r).contains("data:"));
    }

    #[test]
    fn sweep_yields_one_labelled_result_per_param() {
        let params = [1u32, 2, 4];
        let results = sweep_with("size", &fast_cfg(), &params, |p| {
            (0..*p as u64).sum::<u64>()
        });
        assert_eq!(results.len(), 3);
        assert_eq!(results[0].name, "size/1");
        assert_eq!(results[2].name, "size/4");
    }

    #[test]
    fn format_sweep_has_header_and_one_line_per_result() {
        let params = [1u32, 2];
        let results = sweep_with("s", &fast_cfg(), &params, |p| *p as u64 + 1);
        let table = format_sweep(&results);
        let lines: Vec<&str> = table.lines().collect();
        assert_eq!(lines.len(), 3); // ヘッダ + 2 行
        assert!(lines[0].contains("name"));
        assert!(lines[0].contains("thrpt"));
    }
}
