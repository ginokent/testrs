//! Tiny demo that drives the fuzzer against a parser with a planted bug.
//! Run with `cargo run --release --example find_crash -p propcheck-fuzz`.

use propcheck_fuzz::{fuzz, FuzzConfig};

/// Stand-in for a "real" parser: panics on any input that contains the
/// magic byte 0xCC. Without coverage feedback the fuzzer relies on raw
/// random bytes; a single-byte signature is something it can reliably find
/// within tens of thousands of iterations.
fn parser(input: &[u8]) {
    if input.contains(&0xCC) {
        panic!("hit the booby trap (0xCC)");
    }
    // Some plausible "work" to make the loop look real.
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
        // Seeding the corpus with a non-empty input gives the mutator
        // something to chew on; without coverage feedback the fuzzer relies
        // on starting material to reach interesting states.
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
