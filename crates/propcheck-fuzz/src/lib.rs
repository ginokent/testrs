//! In-process mutation fuzzer.
//!
//! Drives a target function with a stream of mutated byte buffers, catches
//! panics as crashes, and minimizes the crashing input by repeatedly
//! deleting bytes that still trigger the panic.
//!
//! Features:
//!
//! - **Crash dedup + continue**: after finding a crash, set
//!   [`FuzzConfig::continue_after_crash`] to keep searching for additional,
//!   distinct crashes. Deduplication is by panic message by default.
//! - **Dictionary**: pre-supplied byte slices (magic numbers, keywords,
//!   protocol tokens) are spliced into mutated inputs with elevated
//!   probability, helping the dumb mutator reach interesting states without
//!   coverage feedback.
//! - **Corpus persistence**: point [`FuzzConfig::corpus_dir`] at a
//!   directory and the fuzzer will load entries on start and append
//!   newly-discovered interesting inputs over time.
//! - **Crash persistence**: [`FuzzConfig::crash_dir`] saves each unique
//!   crash to a file named by a hash of its message, so reproducers
//!   accumulate across runs.
//!
//! # Example
//!
//! ```no_run
//! use propcheck_fuzz::{fuzz, FuzzConfig};
//!
//! let report = fuzz(FuzzConfig::default(), |data: &[u8]| {
//!     if data.len() >= 3 && &data[..3] == b"BAD" {
//!         panic!("found magic bytes");
//!     }
//! });
//!
//! if let Some(f) = report.failure() {
//!     eprintln!("crash after {} iters: {:?}", report.iterations, f.input);
//! }
//! ```

use std::any::Any;
use std::collections::BTreeSet;
use std::env;
use std::fs;
use std::panic::{self, AssertUnwindSafe};
use std::path::PathBuf;

pub use propcheck_core::{Arbitrary, Rng, XorShift64};

mod typed;
pub use typed::{fuzz_typed, TypedFuzzConfig};

/// Tunables for a fuzzing run.
#[derive(Debug, Clone)]
pub struct FuzzConfig {
    /// Maximum number of inputs to run before giving up.
    pub iterations: usize,
    /// Maximum size (in bytes) of any generated input.
    pub max_input_len: usize,
    /// PRNG seed. Defaults to `PROPCHECK_FUZZ_SEED` or wall-clock entropy.
    pub seed: u64,
    /// Initial corpus. Empty inputs are added if both this and `corpus_dir`
    /// are empty.
    pub initial_corpus: Vec<Vec<u8>>,
    /// Maximum number of attempts spent minimizing a crashing input.
    pub minimize_steps: usize,
    /// If `true`, silence the panic hook while fuzzing so crash backtraces
    /// don't spam the terminal.
    pub silence_panic_hook: bool,
    /// If `true`, the fuzzer keeps running after finding a crash to
    /// discover additional, distinct crashes (subject to dedup).
    pub continue_after_crash: bool,
    /// If `true`, crashes with identical panic messages are reported once.
    pub dedup_by_message: bool,
    /// Byte slices spliced into inputs by the dictionary mutation. Useful
    /// for protocol/format magic numbers and keywords.
    pub dictionary: Vec<Vec<u8>>,
    /// Directory to load corpus entries from on start and append new
    /// "interesting" inputs to during the run. Files are named by a hash
    /// of their contents. `None` disables corpus persistence.
    pub corpus_dir: Option<PathBuf>,
    /// Directory to save reproducer files for each unique crash. Filenames
    /// are derived from a hash of the panic message. `None` disables crash
    /// persistence.
    pub crash_dir: Option<PathBuf>,
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
            continue_after_crash: false,
            dedup_by_message: true,
            dictionary: Vec::new(),
            corpus_dir: None,
            crash_dir: None,
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
#[derive(Debug, Clone)]
pub struct FuzzReport {
    /// How many target invocations completed.
    pub iterations: usize,
    /// PRNG seed used, suitable for reproduction.
    pub seed: u64,
    /// All unique crashes found, in order of first discovery.
    pub failures: Vec<Failure>,
}

impl FuzzReport {
    /// First crash discovered, or `None` if none.
    pub fn failure(&self) -> Option<&Failure> {
        self.failures.first()
    }
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

/// Runs `target` against `cfg`.
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

    // Load corpus: union of initial_corpus and corpus_dir contents.
    let mut corpus: Vec<Vec<u8>> = cfg.initial_corpus.clone();
    if let Some(dir) = &cfg.corpus_dir {
        corpus.extend(load_corpus_dir(dir));
    }
    if corpus.is_empty() {
        corpus.push(Vec::new());
    }
    // Track corpus contents we've already saved to disk (by hash).
    let mut persisted_hashes: BTreeSet<u64> = corpus.iter().map(|e| fnv_hash(e)).collect();

    let mut failures: Vec<Failure> = Vec::new();
    let mut seen_crash_messages: BTreeSet<String> = BTreeSet::new();

    for i in 0..cfg.iterations {
        let idx = rng.gen_range_usize(0, corpus.len());
        let base = corpus[idx].clone();
        let mut input = base;
        let n_mutations = rng.gen_range_u64(1, 5);
        for _ in 0..n_mutations {
            mutate(
                &mut rng,
                &mut input,
                &corpus,
                &cfg.dictionary,
                cfg.max_input_len,
            );
        }

        match invoke(&mut target, &input) {
            Ok(()) => {
                // Heuristic: keep a slice of executed inputs around for
                // future splicing. Without coverage feedback this is the
                // only "novelty" signal available.
                if corpus.len() < 1024 && i % 16 == 0 {
                    let h = fnv_hash(&input);
                    if persisted_hashes.insert(h) {
                        if let Some(dir) = &cfg.corpus_dir {
                            let _ = save_input(dir, &input, h);
                        }
                        corpus.push(input);
                    }
                }
            }
            Err(message) => {
                if cfg.dedup_by_message && !seen_crash_messages.insert(message.clone()) {
                    continue; // duplicate crash, ignore
                } else if !cfg.dedup_by_message {
                    // We still want to avoid re-saving the same crash
                    // multiple times in this run; track by message anyway.
                    seen_crash_messages.insert(message.clone());
                }
                let minimized = minimize(&input, &mut target, cfg.minimize_steps);
                let failure = Failure {
                    input: minimized,
                    message: message.clone(),
                    iteration: i + 1,
                };
                if let Some(dir) = &cfg.crash_dir {
                    let _ = save_crash(dir, &failure);
                }
                failures.push(failure);
                if !cfg.continue_after_crash {
                    break;
                }
            }
        }
    }

    FuzzReport {
        iterations: failures
            .last()
            .map(|f| f.iteration)
            .unwrap_or(cfg.iterations),
        seed: cfg.seed,
        failures,
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
                continue;
            }
            i += 1;
        }
        if !shrunk {
            break;
        }
    }
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
    dictionary: &[Vec<u8>],
    max_len: usize,
) {
    let has_dict = !dictionary.is_empty();
    // Bias slightly toward dictionary splicing when dict is non-empty.
    let strategy_count = if has_dict { 9 } else { 7 };
    for _ in 0..3 {
        let choice = rng.gen_range_u64(0, strategy_count);
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
                let idx = rng.gen_range_usize(0, INTERESTING.len());
                input[i] = INTERESTING[idx];
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
                let idx = rng.gen_range_usize(0, corpus.len());
                let other = corpus[idx].clone();
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
                let i = rng.gen_range_usize(0, input.len());
                let j = rng.gen_range_usize(0, input.len());
                input.swap(i, j);
                true
            }
            // Dictionary mutations: insert and overwrite.
            7 if has_dict && input.len() < max_len => {
                let dict_idx = rng.gen_range_usize(0, dictionary.len());
                let entry = &dictionary[dict_idx];
                let pos = rng.gen_range_usize(0, input.len() + 1);
                let mut new = input[..pos].to_vec();
                new.extend_from_slice(entry);
                new.extend_from_slice(&input[pos..]);
                new.truncate(max_len);
                *input = new;
                true
            }
            8 if has_dict && !input.is_empty() => {
                let dict_idx = rng.gen_range_usize(0, dictionary.len());
                let entry = &dictionary[dict_idx];
                if entry.is_empty() {
                    false
                } else {
                    let pos = rng.gen_range_usize(0, input.len());
                    let n = entry.len().min(input.len() - pos);
                    input[pos..pos + n].copy_from_slice(&entry[..n]);
                    true
                }
            }
            _ => false,
        };
        if did {
            return;
        }
    }
    if input.len() < max_len {
        input.push(rng.next_u64() as u8);
    }
}

// --- Persistence helpers -----------------------------------------------

fn load_corpus_dir(dir: &std::path::Path) -> Vec<Vec<u8>> {
    let Ok(entries) = fs::read_dir(dir) else {
        return Vec::new();
    };
    let mut out: Vec<Vec<u8>> = Vec::new();
    for entry in entries.flatten() {
        let path = entry.path();
        if !path.is_file() {
            continue;
        }
        if let Ok(bytes) = fs::read(&path) {
            out.push(bytes);
        }
    }
    out
}

fn save_input(dir: &std::path::Path, input: &[u8], hash: u64) -> std::io::Result<()> {
    fs::create_dir_all(dir)?;
    let path = dir.join(format!("input_{hash:016x}.bin"));
    if !path.exists() {
        fs::write(&path, input)?;
    }
    Ok(())
}

fn save_crash(dir: &std::path::Path, failure: &Failure) -> std::io::Result<()> {
    fs::create_dir_all(dir)?;
    let h = fnv_hash(failure.message.as_bytes());
    let path = dir.join(format!("crash_{h:016x}.bin"));
    if !path.exists() {
        fs::write(&path, &failure.input)?;
        let meta = dir.join(format!("crash_{h:016x}.txt"));
        let msg = format!(
            "iteration: {}\nbytes_len: {}\nmessage:\n{}\n",
            failure.iteration,
            failure.input.len(),
            failure.message
        );
        fs::write(&meta, msg)?;
    }
    Ok(())
}

fn fnv_hash(bytes: &[u8]) -> u64 {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for &b in bytes {
        h ^= b as u64;
        h = h.wrapping_mul(0x100_0000_01b3);
    }
    h
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
            ..FuzzConfig::default()
        }
    }

    #[test]
    fn finds_simple_crash_and_minimizes_it() {
        let report = fuzz(cfg(0xDEAD_BEEF, 50_000), |data: &[u8]| {
            if data.contains(&0xCC) {
                panic!("trigger");
            }
        });
        let failure = report.failure().expect("should have found a crash");
        assert!(failure.input.contains(&0xCC));
        assert_eq!(failure.input, vec![0xCC]);
        assert!(failure.message.contains("trigger"));
    }

    #[test]
    fn no_crash_means_no_failure() {
        let report = fuzz(cfg(1, 200), |_data: &[u8]| {});
        assert!(report.failure().is_none());
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
        match (a.failure(), b.failure()) {
            (Some(fa), Some(fb)) => {
                assert_eq!(fa.iteration, fb.iteration);
                assert_eq!(fa.input, fb.input);
            }
            (None, None) => {}
            _ => panic!("non-deterministic outcome"),
        }
    }

    #[test]
    fn dictionary_helps_find_multi_byte_signature() {
        // Without the dictionary, a 4-byte magic is statistically out of
        // reach in 50k iters. With it, we find it in the first few hundred.
        let cfg = FuzzConfig {
            iterations: 50_000,
            max_input_len: 32,
            seed: 1,
            initial_corpus: vec![b"x".to_vec()],
            minimize_steps: 100,
            silence_panic_hook: false,
            dictionary: vec![b"MAGIC".to_vec()],
            ..FuzzConfig::default()
        };
        let report = fuzz(cfg, |data: &[u8]| {
            if data.windows(5).any(|w| w == b"MAGIC") {
                panic!("found magic");
            }
        });
        assert!(
            report.failure().is_some(),
            "dictionary mutation should find MAGIC"
        );
    }

    #[test]
    fn continue_after_crash_collects_multiple_unique_failures() {
        let cfg = FuzzConfig {
            iterations: 20_000,
            max_input_len: 32,
            seed: 7,
            initial_corpus: vec![b"hello".to_vec()],
            minimize_steps: 50,
            silence_panic_hook: false,
            continue_after_crash: true,
            dedup_by_message: true,
            ..FuzzConfig::default()
        };
        let report = fuzz(cfg, |data: &[u8]| {
            if data.contains(&0xAA) {
                panic!("alpha");
            }
            if data.contains(&0xBB) {
                panic!("beta");
            }
        });
        // We should find both panics — alpha and beta — in distinct
        // failures.
        let msgs: Vec<&str> = report.failures.iter().map(|f| f.message.as_str()).collect();
        assert!(msgs.iter().any(|m| m.contains("alpha")));
        assert!(msgs.iter().any(|m| m.contains("beta")));
    }

    #[test]
    fn dedup_keeps_only_unique_crash_messages() {
        let cfg = FuzzConfig {
            iterations: 5_000,
            max_input_len: 32,
            seed: 11,
            initial_corpus: vec![b"hi".to_vec()],
            minimize_steps: 50,
            silence_panic_hook: false,
            continue_after_crash: true,
            dedup_by_message: true,
            ..FuzzConfig::default()
        };
        let report = fuzz(cfg, |data: &[u8]| {
            if data.contains(&0xCC) {
                panic!("same message every time");
            }
        });
        // Only one unique crash, no matter how many times it triggers.
        assert_eq!(report.failures.len(), 1);
    }

    #[test]
    fn corpus_and_crash_persistence_roundtrip() {
        let temp = std::env::temp_dir().join(format!("propcheck-fuzz-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&temp);
        let corpus_dir = temp.join("corpus");
        let crash_dir = temp.join("crashes");

        // First run: discover a crash and let it persist.
        let cfg1 = FuzzConfig {
            iterations: 5_000,
            max_input_len: 16,
            seed: 0xCAFE,
            initial_corpus: vec![b"x".to_vec()],
            minimize_steps: 50,
            silence_panic_hook: false,
            corpus_dir: Some(corpus_dir.clone()),
            crash_dir: Some(crash_dir.clone()),
            ..FuzzConfig::default()
        };
        let r1 = fuzz(cfg1, |data: &[u8]| {
            if data.contains(&0xCC) {
                panic!("p");
            }
        });
        assert!(r1.failure().is_some());

        // Crash file should exist.
        let crashes: Vec<_> = std::fs::read_dir(&crash_dir).unwrap().collect();
        assert!(!crashes.is_empty(), "no crash files written");

        // Second run: load corpus from disk (without explicit initial_corpus).
        let cfg2 = FuzzConfig {
            iterations: 1_000,
            max_input_len: 16,
            seed: 0xBEEF,
            initial_corpus: Vec::new(),
            minimize_steps: 50,
            silence_panic_hook: false,
            corpus_dir: Some(corpus_dir.clone()),
            crash_dir: None,
            ..FuzzConfig::default()
        };
        let r2 = fuzz(cfg2, |_data: &[u8]| { /* benign */ });
        assert!(r2.failure().is_none());

        std::fs::remove_dir_all(&temp).unwrap();
    }
}
