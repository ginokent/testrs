//! 統計モジュールの数学的性質を PBT で検証する。
//!
//! testrs-bench 自体は std のみに依存するが、テストでは testrs-pbt を
//! dev-dependency として用い、型情報 (Strategy) に基づく入力生成で
//! プロパティを検証する。サンプルは有限・非負のナノ秒値を模すため、
//! `f64_range(0.0..1e9)` で生成する (NaN / 無限大は混入させない)。

use testrs_bench::stats::{self, OutlierFilter, Statistics};
use testrs_pbt::run_strategy;
use testrs_pbt::strategy::{f64_range, vec_of};

/// 非空サンプルの平均は [min, max] に収まる。
#[test]
fn mean_is_within_bounds() {
    let s = vec_of(f64_range(0.0..1e9), 1..200);
    run_strategy("mean within [min, max]", s, |xs: &Vec<f64>| {
        let m = stats::mean(xs).expect("non-empty");
        let lo = xs.iter().copied().fold(f64::INFINITY, f64::min);
        let hi = xs.iter().copied().fold(f64::NEG_INFINITY, f64::max);
        // 浮動小数の丸めを許容する微小マージン。
        m >= lo - 1e-3 && m <= hi + 1e-3
    });
}

/// 中央値とパーセンタイルは [min, max] に収まる。
#[test]
fn statistics_ordering_holds() {
    let s = vec_of(f64_range(0.0..1e9), 1..200);
    run_strategy("min <= p25 <= median <= p75 <= max", s, |xs: &Vec<f64>| {
        let st = Statistics::from_samples(xs).expect("non-empty");
        st.min <= st.p25 + 1e-3
            && st.p25 <= st.median + 1e-3
            && st.median <= st.p75 + 1e-3
            && st.p75 <= st.max + 1e-3
    });
}

/// 標準偏差・MAD・変動係数は非負。
#[test]
fn dispersion_is_non_negative() {
    let s = vec_of(f64_range(0.0..1e9), 1..200);
    run_strategy("std_dev, mad, cv are non-negative", s, |xs: &Vec<f64>| {
        let st = Statistics::from_samples(xs).expect("non-empty");
        st.std_dev >= 0.0 && st.mad >= 0.0 && st.cv >= 0.0
    });
}

/// パーセンタイルは p に対して単調非減少。
#[test]
fn percentile_is_monotonic() {
    let s = vec_of(f64_range(0.0..1e9), 1..200);
    run_strategy("percentile monotonic in p", s, |xs: &Vec<f64>| {
        let mut sorted = xs.clone();
        sorted.sort_by(|a, b| a.partial_cmp(b).expect("finite"));
        let mut prev = f64::NEG_INFINITY;
        for p in [0.0, 10.0, 25.0, 50.0, 75.0, 90.0, 100.0] {
            let v = stats::percentile_sorted(&sorted, p).expect("non-empty");
            if v < prev - 1e-3 {
                return false;
            }
            prev = v;
        }
        true
    });
}

/// 外れ値除去後のサンプルは常に元の部分集合で、除去数と整合する。
#[test]
fn filter_keeps_subset_and_counts_correctly() {
    let s = vec_of(f64_range(0.0..1e9), 1..200);
    run_strategy(
        "filtered <= input and removed accounting",
        s,
        |xs: &Vec<f64>| {
            for filter in [
                OutlierFilter::None,
                OutlierFilter::Tukey,
                OutlierFilter::Mad,
            ] {
                let (kept, removed) = stats::filter_outliers(xs, filter);
                if kept.len() + removed != xs.len() {
                    return false;
                }
                if kept.len() > xs.len() {
                    return false;
                }
                // 残った値はすべて元の集合に存在する。
                for k in &kept {
                    if !xs.iter().any(|x| x == k) {
                        return false;
                    }
                }
            }
            true
        },
    );
}

/// None フィルタは入力をそのまま残す。
#[test]
fn none_filter_is_identity() {
    let s = vec_of(f64_range(0.0..1e9), 1..200);
    run_strategy("None filter is identity", s, |xs: &Vec<f64>| {
        let (kept, removed) = stats::filter_outliers(xs, OutlierFilter::None);
        removed == 0 && kept == *xs
    });
}
