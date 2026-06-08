//! ベンチマークサンプル列に対する記述統計と外れ値除去です。
//!
//! 入力はすべて「1 反復あたりの所要ナノ秒」を表す `f64` のスライスです。
//! 外部依存はありません。NaN / 無限大は呼び出し側が混入させない前提で、
//! ここでは有限値のみを扱います。

/// 外れ値の除去方式です。
///
/// ベンチマークの測定値は OS スケジューリングや CPU 周波数変動により
/// 上振れした外れ値を含みやすいため、統計を取る前に除去できます。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OutlierFilter {
    /// 除去しません。
    None,
    /// Tukey の fence (Q1 - 1.5*IQR 未満、Q3 + 1.5*IQR 超過) を外れ値とします。
    Tukey,
    /// 中央値からの偏差が 3 * (1.4826 * MAD) を超えるものを外れ値とします。
    Mad,
}

/// サンプル列から計算した記述統計です。
///
/// すべてのフィールドを公開し、将来の永続化 / 有意差判定が
/// この構造体だけで完結できるようにしています。
#[derive(Debug, Clone, PartialEq)]
pub struct Statistics {
    /// 統計の母数となったサンプル数 (外れ値除去後) です。
    pub n: usize,
    /// 算術平均です。
    pub mean: f64,
    /// 中央値です。
    pub median: f64,
    /// 最小値です。
    pub min: f64,
    /// 最大値です。
    pub max: f64,
    /// 標本標準偏差 (n-1 で割る不偏推定) です。n < 2 のときは 0 です。
    pub std_dev: f64,
    /// 中央絶対偏差 (median absolute deviation) です。
    pub mad: f64,
    /// 変動係数 (std_dev / mean)。mean が 0 のときは 0 です。
    pub cv: f64,
    /// 第 1 四分位数 (25 パーセンタイル) です。
    pub p25: f64,
    /// 第 3 四分位数 (75 パーセンタイル) です。
    pub p75: f64,
}

impl Statistics {
    /// サンプル列から統計を計算します。空スライスのときは `None` です。
    pub fn from_samples(samples: &[f64]) -> Option<Statistics> {
        if samples.is_empty() {
            return None;
        }
        let mut sorted = samples.to_vec();
        sorted.sort_by(|a, b| a.partial_cmp(b).expect("samples must be finite (no NaN)"));

        let n = sorted.len();
        let mean = mean(&sorted).expect("non-empty");
        let median = percentile_sorted(&sorted, 50.0).expect("non-empty");
        let min = sorted[0];
        let max = sorted[n - 1];
        let std_dev = std_dev(&sorted).unwrap_or(0.0);
        let mad = mad_sorted(&sorted);
        let cv = if mean != 0.0 { std_dev / mean } else { 0.0 };
        let p25 = percentile_sorted(&sorted, 25.0).expect("non-empty");
        let p75 = percentile_sorted(&sorted, 75.0).expect("non-empty");

        Some(Statistics {
            n,
            mean,
            median,
            min,
            max,
            std_dev,
            mad,
            cv,
            p25,
            p75,
        })
    }
}

/// 算術平均です。空スライスのときは `None` です。
pub fn mean(xs: &[f64]) -> Option<f64> {
    if xs.is_empty() {
        return None;
    }
    Some(xs.iter().sum::<f64>() / xs.len() as f64)
}

/// 標本標準偏差 (n-1 で割る不偏推定) です。要素数が 2 未満のときは `None` です。
pub fn std_dev(xs: &[f64]) -> Option<f64> {
    if xs.len() < 2 {
        return None;
    }
    let m = mean(xs)?;
    let sum_sq = xs
        .iter()
        .map(|x| {
            let d = x - m;
            d * d
        })
        .sum::<f64>();
    Some((sum_sq / (xs.len() - 1) as f64).sqrt())
}

/// 昇順ソート済みスライスに対する線形補間パーセンタイルです。
///
/// `p` は `[0.0, 100.0]` の範囲を想定します。空スライスのときは `None` です。
pub fn percentile_sorted(sorted: &[f64], p: f64) -> Option<f64> {
    if sorted.is_empty() {
        return None;
    }
    if sorted.len() == 1 {
        return Some(sorted[0]);
    }
    let rank = (p / 100.0) * (sorted.len() - 1) as f64;
    let lo = rank.floor() as usize;
    let hi = rank.ceil() as usize;
    let frac = rank - lo as f64;
    Some(sorted[lo] + (sorted[hi] - sorted[lo]) * frac)
}

/// 中央絶対偏差 (median absolute deviation) です。
///
/// `median(|x_i - median(x)|)` を返します。空スライスのときは 0 です。
pub fn mad_sorted(sorted: &[f64]) -> f64 {
    let Some(med) = percentile_sorted(sorted, 50.0) else {
        return 0.0;
    };
    let mut devs: Vec<f64> = sorted.iter().map(|x| (x - med).abs()).collect();
    devs.sort_by(|a, b| a.partial_cmp(b).expect("deviations must be finite"));
    percentile_sorted(&devs, 50.0).unwrap_or(0.0)
}

/// 指定方式で外れ値を除去し、`(残ったサンプル, 除去数)` を返します。
///
/// 残るサンプルは常に元の部分集合であり、`OutlierFilter::None` のときは
/// 入力をそのまま返します。すべてが外れ値と判定される構成
/// (例: 散らばりが 0) では入力をそのまま残します。
pub fn filter_outliers(samples: &[f64], filter: OutlierFilter) -> (Vec<f64>, usize) {
    if samples.is_empty() || filter == OutlierFilter::None {
        return (samples.to_vec(), 0);
    }

    let mut sorted = samples.to_vec();
    sorted.sort_by(|a, b| a.partial_cmp(b).expect("samples must be finite (no NaN)"));

    let keep: Vec<f64> = match filter {
        OutlierFilter::None => samples.to_vec(),
        OutlierFilter::Tukey => {
            let q1 = percentile_sorted(&sorted, 25.0).expect("non-empty");
            let q3 = percentile_sorted(&sorted, 75.0).expect("non-empty");
            let iqr = q3 - q1;
            let lo = q1 - 1.5 * iqr;
            let hi = q3 + 1.5 * iqr;
            samples
                .iter()
                .copied()
                .filter(|x| *x >= lo && *x <= hi)
                .collect()
        }
        OutlierFilter::Mad => {
            let med = percentile_sorted(&sorted, 50.0).expect("non-empty");
            let scaled = 1.4826 * mad_sorted(&sorted);
            if scaled == 0.0 {
                // 散らばりが無い (または極小) ため、全件を残す。
                samples.to_vec()
            } else {
                samples
                    .iter()
                    .copied()
                    .filter(|x| (x - med).abs() <= 3.0 * scaled)
                    .collect()
            }
        }
    };

    // 全件が外れ値になる病的ケースでは、統計を計算できるよう入力を温存する。
    if keep.is_empty() {
        return (samples.to_vec(), 0);
    }
    let removed = samples.len() - keep.len();
    (keep, removed)
}

#[cfg(test)]
mod tests {
    use super::*;

    // 境界値・エラーパスは PBT (tests/stats_pbt.rs) で表現できないため
    // ここで個別に検証する。

    #[test]
    fn empty_inputs_return_none() {
        assert!(mean(&[]).is_none());
        assert!(percentile_sorted(&[], 50.0).is_none());
        assert!(Statistics::from_samples(&[]).is_none());
    }

    #[test]
    fn single_element_has_zero_std_dev() {
        let s = Statistics::from_samples(&[42.0]).expect("non-empty");
        assert_eq!(s.n, 1);
        assert_eq!(s.mean, 42.0);
        assert_eq!(s.median, 42.0);
        assert_eq!(s.std_dev, 0.0);
        assert_eq!(s.cv, 0.0);
    }

    #[test]
    fn std_dev_needs_two_elements() {
        assert!(std_dev(&[1.0]).is_none());
        assert!(std_dev(&[1.0, 1.0]).is_some());
    }

    #[test]
    fn percentile_endpoints() {
        let xs = [1.0, 2.0, 3.0, 4.0];
        assert_eq!(percentile_sorted(&xs, 0.0), Some(1.0));
        assert_eq!(percentile_sorted(&xs, 100.0), Some(4.0));
        assert_eq!(percentile_sorted(&xs, 50.0), Some(2.5));
    }

    #[test]
    fn tukey_removes_extreme_high_outlier() {
        // 1..=10 に大きな外れ値 1000 を加える。Tukey fence で除去されるはず。
        let mut xs: Vec<f64> = (1..=10).map(|i| i as f64).collect();
        xs.push(1000.0);
        let (kept, removed) = filter_outliers(&xs, OutlierFilter::Tukey);
        assert_eq!(removed, 1);
        assert!(!kept.contains(&1000.0));
    }

    #[test]
    fn none_filter_keeps_everything() {
        let xs = [1.0, 2.0, 1000.0];
        let (kept, removed) = filter_outliers(&xs, OutlierFilter::None);
        assert_eq!(removed, 0);
        assert_eq!(kept, xs);
    }

    #[test]
    fn mad_filter_keeps_all_when_dispersion_is_zero() {
        let xs = [5.0, 5.0, 5.0, 5.0];
        let (kept, removed) = filter_outliers(&xs, OutlierFilter::Mad);
        assert_eq!(removed, 0);
        assert_eq!(kept.len(), 4);
    }
}
