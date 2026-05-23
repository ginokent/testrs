//! In-process mutation fuzzer.
//!
//! Drives a target function with a stream of mutated byte buffers, catches
//! panics as crashes, and minimizes the crashing input by repeatedly
//! deleting bytes that still trigger the panic.
//!
//! # Example
//!
//! ```no_run
//! use propcheck_fuzz::{fuzz, FuzzConfig};
//!
//! let report = fuzz(FuzzConfig::default(), |data: &[u8]| {
//!     // The target under test. A panic counts as a crash.
//!     if data.len() >= 3 && &data[..3] == b"BAD" {
//!         panic!("found magic bytes");
//!     }
//! });
//!
//! if let Some(f) = report.failure {
//!     eprintln!("crash after {} iters: {:?}", report.iterations, f.input);
//! }
//! ```

use std::any::Any;
use std::env;
use std::panic::{self, AssertUnwindSafe};

pub use propcheck_core::{Rng, XorShift64};

/// Tunables for a fuzzing run.
#[derive(Debug, Clone)]
pub struct FuzzConfig {
    /// Maximum number of inputs to run before giving up.
    pub iterations: usize,
    /// Maximum size (in bytes) of any generated input.
    pub max_input_len: usize,
    /// PRNG seed. Defaults to `PROPCHECK_FUZZ_SEED` or wall-clock entropy.
    pub seed: u64,
    /// Initial corpus. Empty inputs are added if the corpus is empty.
    pub initial_corpus: Vec<Vec<u8>>,
    /// Maximum number of attempts spent minimizing a crashing input.
    pub minimize_steps: usize,
    /// If `true`, silence the panic hook while fuzzing so crash backtraces
    /// don't spam the terminal. Global state — see [`Self::silence_panic_hook`].
    pub silence_panic_hook: bool,
}

impl Default for FuzzConfig {
    fn default() -> Self {
        Self {
            iterations: 10_000,
            max_input_len: 4096,
            seed: env_seed(),
            initial_corpus: Vec::new(),
            minimize_steps: 500,
            silence_panic_hook: true,
        }
    }
}

fn env_seed() -> u64 {
    if let Ok(s) = env::var("PROPCHECK_FUZZ_SEED") {
        if let Ok(n) = s.parse::<u64>() {
            return n;
        }
    }
    XorShift64::from_entropy().state()
}

/// Result of a fuzzing run.
#[derive(Debug)]
pub struct FuzzReport {
    /// How many target invocations completed.
    pub iterations: usize,
    /// PRNG seed used, suitable for reproduction.
    pub seed: u64,
    /// `Some` if a crash was found, `None` if the iteration budget was exhausted.
    pub failure: Option<Failure>,
}

/// A crashing input the fuzzer discovered.
#[derive(Debug, Clone)]
pub struct Failure {
    /// Minimized crashing input.
    pub input: Vec<u8>,
    /// Panic message (or `<non-string panic payload>`).
    pub message: String,
    /// 1-based iteration index at which the crash was found.
    pub iteration: usize,
}

/// Runs `target` against [`FuzzConfig::default`].
pub fn fuzz<F>(cfg: FuzzConfig, target: F) -> FuzzReport
where
    F: FnMut(&[u8]),
{
    if cfg.silence_panic_hook {
        let prev = panic::take_hook();
        panic::set_hook(Box::new(|_| {}));
        let report = run_fuzz(cfg, target);
        panic::set_hook(prev);
        report
    } else {
        run_fuzz(cfg, target)
    }
}

fn run_fuzz<F>(cfg: FuzzConfig, mut target: F) -> FuzzReport
where
    F: FnMut(&[u8]),
{
    let mut rng = XorShift64::seed_from_u64(cfg.seed);
    let mut corpus: Vec<Vec<u8>> = if cfg.initial_corpus.is_empty() {
        vec![Vec::new()]
    } else {
        cfg.initial_corpus.clone()
    };

    for i in 0..cfg.iterations {
        let base = rng
            .choose(&corpus)
            .cloned()
            .unwrap_or_default();
        let mut input = base;
        // Apply 1..=4 mutations to amplify changes.
        let n_mutations = rng.gen_range_u64(1, 5);
        for _ in 0..n_mutations {
            mutate(&mut rng, &mut input, &corpus, cfg.max_input_len);
        }

        match invoke(&mut target, &input) {
            Ok(()) => {
                // Cheap "corpus growth" heuristic: occasionally keep an
                // input around for future splicing. We don't have coverage
                // feedback, so this is a stand-in for novelty.
                if corpus.len() < 256 && i % 16 == 0 {
                    corpus.push(input);
                }
            }
            Err(message) => {
                let minimized = minimize(&input, &mut target, cfg.minimize_steps);
                return FuzzReport {
                    iterations: i + 1,
                    seed: cfg.seed,
                    failure: Some(Failure {
                        input: minimized,
                        message,
                        iteration: i + 1,
                    }),
                };
            }
        }
    }

    FuzzReport {
        iterations: cfg.iterations,
        seed: cfg.seed,
        failure: None,
    }
}

fn invoke<F: FnMut(&[u8])>(target: &mut F, input: &[u8]) -> Result<(), String> {
    match panic::catch_unwind(AssertUnwindSafe(|| target(input))) {
        Ok(()) => Ok(()),
        Err(payload) => Err(extract_panic_message(&payload)),
    }
}

fn extract_panic_message(payload: &Box<dyn Any + Send>) -> String {
    if let Some(s) = payload.downcast_ref::<&'static str>() {
        (*s).to_string()
    } else if let Some(s) = payload.downcast_ref::<String>() {
        s.clone()
    } else {
        "<non-string panic payload>".to_string()
    }
}

fn minimize<F: FnMut(&[u8])>(crash: &[u8], target: &mut F, max_steps: usize) -> Vec<u8> {
    let mut current = crash.to_vec();
    let mut steps = 0;
    // Pass 1: delete single bytes until stable.
    loop {
        if current.is_empty() {
            return current;
        }
        let mut shrunk = false;
        let mut i = 0;
        while i < current.len() {
            if steps >= max_steps {
                return current;
            }
            steps += 1;
            let mut candidate = current.clone();
            candidate.remove(i);
            if invoke(target, &candidate).is_err() {
                current = candidate;
                shrunk = true;
                // Restart from the same index (the array shifted left).
                continue;
            }
            i += 1;
        }
        if !shrunk {
            break;
        }
    }
    // Pass 2: try zeroing bytes (canonical form).
    for i in 0..current.len() {
        if steps >= max_steps {
            return current;
        }
        steps += 1;
        if current[i] == 0 {
            continue;
        }
        let mut candidate = current.clone();
        candidate[i] = 0;
        if invoke(target, &candidate).is_err() {
            current = candidate;
        }
    }
    current
}

fn mutate<R: Rng + ?Sized>(
    rng: &mut R,
    input: &mut Vec<u8>,
    corpus: &[Vec<u8>],
    max_len: usize,
) {
    // Pick a mutation; fall back to "append byte" if the chosen one would be
    // a no-op (e.g. delete from an empty buffer).
    for _ in 0..3 {
        let choice = rng.gen_range_u64(0, 7);
        let did = match choice {
            0 if !input.is_empty() => {
                let i = rng.gen_range_usize(0, input.len());
                let bit = rng.gen_range_u64(0, 8) as u8;
                input[i] ^= 1 << bit;
                true
            }
            1 if !input.is_empty() => {
                let i = rng.gen_range_usize(0, input.len());
                input[i] = rng.next_u64() as u8;
                true
            }
            2 if !input.is_empty() => {
                let i = rng.gen_range_usize(0, input.len());
                const INTERESTING: [u8; 7] = [0x00, 0x01, 0x10, 0x7f, 0x80, 0xfe, 0xff];
                input[i] = *rng.choose(&INTERESTING).unwrap();
                true
            }
            3 if input.len() < max_len => {
                let i = rng.gen_range_usize(0, input.len() + 1);
                input.insert(i, rng.next_u64() as u8);
                true
            }
            4 if !input.is_empty() => {
                let i = rng.gen_range_usize(0, input.len());
                input.remove(i);
                true
            }
            5 if !corpus.is_empty() && !input.is_empty() => {
                let other = rng.choose(corpus).cloned().unwrap_or_default();
                if other.is_empty() {
                    false
                } else {
                    let cut_self = rng.gen_range_usize(0, input.len() + 1);
                    let cut_other = rng.gen_range_usize(0, other.len());
                    let mut new = input[..cut_self].to_vec();
                    new.extend_from_slice(&other[cut_other..]);
                    new.truncate(max_len);
                    *input = new;
                    true
                }
            }
            6 if !input.is_empty() && input.len() > 1 => {
                // Swap two bytes.
                let i = rng.gen_range_usize(0, input.len());
                let j = rng.gen_range_usize(0, input.len());
                input.swap(i, j);
                true
            }
            _ => false,
        };
        if did {
            return;
        }
    }
    // Last resort: grow.
    if input.len() < max_len {
        input.push(rng.next_u64() as u8);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cfg(seed: u64, iters: usize) -> FuzzConfig {
        FuzzConfig {
            iterations: iters,
            max_input_len: 64,
            seed,
            initial_corpus: vec![b"hello world".to_vec()],
            minimize_steps: 200,
            silence_panic_hook: false,
        }
    }

    #[test]
    fn finds_simple_crash_and_minimizes_it() {
        // The "bug": panic if the input contains the byte 0xCC.
        let report = fuzz(cfg(0xDEAD_BEEF, 50_000), |data: &[u8]| {
            if data.contains(&0xCC) {
                panic!("trigger");
            }
        });

        let failure = report.failure.expect("should have found a crash");
        assert!(failure.input.contains(&0xCC));
        // Minimization should reduce to a single byte.
        assert_eq!(failure.input, vec![0xCC]);
        assert!(failure.message.contains("trigger"));
    }

    #[test]
    fn no_crash_means_no_failure() {
        let report = fuzz(cfg(1, 200), |_data: &[u8]| { /* well-behaved */ });
        assert!(report.failure.is_none());
        assert_eq!(report.iterations, 200);
    }

    #[test]
    fn run_is_reproducible_for_same_seed() {
        let a = fuzz(cfg(0xABC, 5_000), |data: &[u8]| {
            if data.len() >= 2 && data[0] == b'!' && data[1] == b'?' {
                panic!("match");
            }
        });
        let b = fuzz(cfg(0xABC, 5_000), |data: &[u8]| {
            if data.len() >= 2 && data[0] == b'!' && data[1] == b'?' {
                panic!("match");
            }
        });
        match (a.failure, b.failure) {
            (Some(fa), Some(fb)) => {
                assert_eq!(fa.iteration, fb.iteration);
                assert_eq!(fa.input, fb.input);
            }
            (None, None) => {} // both budgets exhausted; still reproducible
            _ => panic!("non-deterministic outcome"),
        }
    }
}
