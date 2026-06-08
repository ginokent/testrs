//! `sort` と `sort_unstable` を相対比較するデモです。
//!
//! 実行: `cargo run --release --example compare_sorts -p testrs-bench`

use std::hint::black_box;
use testrs_bench::{bench, bench_compare};

fn main() {
    // 単発の計測。
    bench("vec push 0..1000", || {
        let mut v = Vec::new();
        for i in 0..1000u32 {
            v.push(i);
        }
        v
    });

    // 同じ入力に対する 2 つのソート実装の相対比較。
    // 入力生成はクロージャ内に閉じ込め、black_box で最適化による
    // 畳み込みを防ぐ。
    let data: Vec<i32> = (0..1000).rev().collect();
    bench_compare(
        "sort_unstable",
        || {
            let mut v = black_box(data.clone());
            v.sort_unstable();
            v
        },
        "sort",
        || {
            let mut v = black_box(data.clone());
            v.sort();
            v
        },
    );
}
