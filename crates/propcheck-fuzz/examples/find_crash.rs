//! 仕込まれたバグのある parser ーに対して fuzzer を実行する小さなデモです。
//! `cargo run --release --example find_crash -p propcheck-fuzz` で実行します。

use propcheck_fuzz::{fuzz, FuzzConfig};

/// 「本物の」parser ーの代わりとして、マジックバイト 0xCC を含む任意の入力で
/// panic します。カバレッジフィードバックがないため、fuzzer は生のランダム
/// バイトに頼ります。1 バイトのシグネチャであれば、数万回のイテレーション以内に
/// 確実に発見できます。
fn parser(input: &[u8]) {
    if input.contains(&0xCC) {
        panic!("hit the booby trap (0xCC)");
    }
    // ループを本物らしく見せるための、それらしい「処理」です。
    let mut acc: u32 = 0;
    for b in input {
        acc = acc.wrapping_mul(31).wrapping_add(*b as u32);
    }
    std::hint::black_box(acc);
}

fn main() {
    let cfg = FuzzConfig {
        iterations: 200_000,
        max_input_len: 128,
        // corpus を空でない入力で seed すると、mutator ーに噛みごたえの
        // ある材料を与えられます。カバレッジフィードバックがないため、fuzzer は
        // 興味深い状態に到達するために初期素材に依存します。
        initial_corpus: vec![b"the quick brown fox jumps over the lazy dog".to_vec()],
        ..FuzzConfig::default()
    };
    let report = fuzz(cfg, parser);

    println!(
        "ran {} iterations, seed = {} (set PROPCHECK_FUZZ_SEED={} to reproduce)",
        report.iterations, report.seed, report.seed
    );
    if report.failures.is_empty() {
        println!("no crashes found");
    } else {
        for f in &report.failures {
            println!(
                "crash at iter {}: input = {:?} ({} bytes), message = {:?}",
                f.iteration,
                f.input,
                f.input.len(),
                f.message
            );
        }
    }
}
