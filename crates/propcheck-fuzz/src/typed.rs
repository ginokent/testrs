//! Typed-input fuzzing built on top of [`crate::fuzz`].
//!
//! The byte-oriented fuzzer mutates `&[u8]` directly, which is awkward for
//! testing APIs that take structured input. [`fuzz_typed`] derives a
//! seedable PRNG from the mutated bytes and then asks `T: Arbitrary` for
//! a value, giving you mutation-driven coverage of any `Arbitrary` type.

use propcheck_core::{Arbitrary, XorShift64};

use crate::{fuzz, FuzzConfig, FuzzReport};

/// Tunables specific to typed fuzzing.
#[derive(Debug, Clone)]
pub struct TypedFuzzConfig {
    /// Underlying byte-fuzzer config (controls iterations, seed, corpus, …).
    pub inner: FuzzConfig,
    /// Size hint passed to `T::arbitrary`. Larger values produce larger /
    /// deeper structures.
    pub size: usize,
}

impl Default for TypedFuzzConfig {
    fn default() -> Self {
        Self {
            inner: FuzzConfig::default(),
            size: 64,
        }
    }
}

/// Fuzzes a target taking a typed value. Each byte buffer the mutator
/// produces is interpreted as the seed (plus auxiliary entropy) of a
/// [`XorShift64`] PRNG, which then drives `T::arbitrary` to construct the
/// value handed to `target`.
///
/// ```no_run
/// use propcheck_fuzz::{fuzz_typed, TypedFuzzConfig};
///
/// fuzz_typed::<String, _>(TypedFuzzConfig::default(), |s: &String| {
///     // a planted bug: panic on a specific substring
///     if s.contains("crash") {
///         panic!("found 'crash' in {s:?}");
///     }
/// });
/// ```
pub fn fuzz_typed<T, F>(cfg: TypedFuzzConfig, mut target: F) -> FuzzReport
where
    T: Arbitrary,
    F: FnMut(&T),
{
    let size = cfg.size;
    fuzz(cfg.inner, move |bytes: &[u8]| {
        let seed = seed_from_bytes(bytes);
        let mut rng = XorShift64::seed_from_u64(seed);
        let value: T = T::arbitrary(&mut rng, size);
        target(&value);
    })
}

/// Hashes a byte slice into a u64 seed using a small mixing function. The
/// goal is just that different byte sequences map to different seeds; no
/// cryptographic property is needed.
fn seed_from_bytes(bytes: &[u8]) -> u64 {
    // FNV-1a-style mix, then a final avalanche step. Empty input yields a
    // fixed non-zero seed because XorShift64 dislikes 0.
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for &b in bytes {
        h ^= b as u64;
        h = h.wrapping_mul(0x100_0000_01b3);
    }
    // Avalanche
    h ^= h >> 33;
    h = h.wrapping_mul(0xff51_afd7_ed55_8ccd);
    h ^= h >> 33;
    if h == 0 {
        0x9E37_79B9_7F4A_7C15
    } else {
        h
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn finds_typed_crash() {
        // Bug: panic when generated u32 happens to be small and even.
        // Using Arbitrary on u32 is fully driven by the seed bytes.
        let cfg = TypedFuzzConfig {
            inner: FuzzConfig {
                iterations: 100_000,
                max_input_len: 32,
                seed: 0x5EED_C0DE,
                initial_corpus: vec![b"seed-input".to_vec()],
                minimize_steps: 100,
                silence_panic_hook: false,
            },
            size: 16,
        };
        let report = fuzz_typed::<u8, _>(cfg, |n: &u8| {
            if *n == 0xAB {
                panic!("triggered on {n:#x}");
            }
        });
        assert!(report.failure.is_some(), "should find a crash");
    }

    #[test]
    fn no_crash_for_benign_target() {
        let cfg = TypedFuzzConfig {
            inner: FuzzConfig {
                iterations: 500,
                max_input_len: 32,
                seed: 0xC0DE,
                initial_corpus: vec![b"seed".to_vec()],
                minimize_steps: 50,
                silence_panic_hook: false,
            },
            size: 16,
        };
        let report = fuzz_typed::<Vec<u8>, _>(cfg, |_v: &Vec<u8>| {});
        assert!(report.failure.is_none());
    }
}
