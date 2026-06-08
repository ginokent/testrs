//! データスループット (bytes/s) 計測と sweep のデモです。
//!
//! 実行: `cargo run --release --example throughput_sweep -p testrs-bench`

use std::hint::black_box;
use testrs_bench::{bench_throughput, format_sweep, sweep, BenchResult};

fn main() {
    // 1 反復が 64 KiB を処理するベンチ。出力に data (bytes/s) 行が自動で出る。
    let data = vec![1u8; 64 * 1024];
    bench_throughput("sum 64KiB", data.len() as u64, || {
        black_box(data.iter().map(|&b| b as u64).sum::<u64>())
    });

    // 入力サイズを振った sweep。各結果にそのサイズを throughput メタとして
    // 付与し、bytes/s 列込みの表で出力する。
    // 注: このデモでは各反復でバッファを確保するため確保コストも含む。
    let sizes = [1024usize, 4096, 16384];
    let results: Vec<BenchResult> = sweep("sum", &sizes, |&n| {
        let buf = vec![1u8; n];
        black_box(buf.iter().map(|&b| b as u64).sum::<u64>())
    })
    .into_iter()
    .zip(sizes)
    .map(|(r, n)| r.with_bytes_per_iter(n as u64))
    .collect();
    eprintln!("{}", format_sweep(&results));
}
