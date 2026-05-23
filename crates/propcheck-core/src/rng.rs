//! A tiny pseudo-random number generator.
//!
//! [`XorShift64`] implements Marsaglia/Vigna's xorshift64* algorithm. It is
//! not cryptographically secure but is fast, deterministic for a given seed,
//! and good enough for randomized testing.

use std::time::{SystemTime, UNIX_EPOCH};

/// A minimal PRNG trait used by [`crate::Arbitrary`] generators.
///
/// Implementors only need to provide [`Rng::next_u64`]; the rest of the
/// methods have correct (if not always optimal) default implementations.
pub trait Rng {
    /// Returns the next 64 random bits.
    fn next_u64(&mut self) -> u64;

    /// Returns the next 32 random bits.
    fn next_u32(&mut self) -> u32 {
        self.next_u64() as u32
    }

    /// Returns a uniformly random bit as a `bool`.
    fn gen_bool(&mut self) -> bool {
        (self.next_u64() & 1) == 1
    }

    /// Returns a `u64` in `[lo, hi)`.
    ///
    /// Uses a modulo reduction, which introduces a small bias when `hi - lo`
    /// does not evenly divide `u64::MAX + 1`. This is acceptable for testing.
    fn gen_range_u64(&mut self, lo: u64, hi: u64) -> u64 {
        assert!(lo < hi, "gen_range_u64: lo ({lo}) must be < hi ({hi})");
        let span = hi - lo;
        lo + (self.next_u64() % span)
    }

    /// Returns an `i64` in `[lo, hi)`.
    fn gen_range_i64(&mut self, lo: i64, hi: i64) -> i64 {
        assert!(lo < hi, "gen_range_i64: lo ({lo}) must be < hi ({hi})");
        let span = (hi as i128 - lo as i128) as u128;
        let r = (self.next_u64() as u128) % span;
        (lo as i128 + r as i128) as i64
    }

    /// Returns a `usize` in `[lo, hi)`.
    fn gen_range_usize(&mut self, lo: usize, hi: usize) -> usize {
        assert!(lo < hi, "gen_range_usize: lo ({lo}) must be < hi ({hi})");
        let span = (hi - lo) as u64;
        lo + (self.next_u64() % span) as usize
    }

    /// Fills `buf` with random bytes.
    fn fill_bytes(&mut self, buf: &mut [u8]) {
        let mut i = 0;
        while i < buf.len() {
            let chunk = self.next_u64().to_le_bytes();
            let n = (buf.len() - i).min(8);
            buf[i..i + n].copy_from_slice(&chunk[..n]);
            i += n;
        }
    }

    /// Returns a uniformly random reference to an element of `slice`, or
    /// `None` if the slice is empty.
    fn choose<'a, T>(&mut self, slice: &'a [T]) -> Option<&'a T> {
        if slice.is_empty() {
            None
        } else {
            Some(&slice[self.gen_range_usize(0, slice.len())])
        }
    }
}

/// xorshift64* PRNG. Deterministic for a given seed.
#[derive(Debug, Clone)]
pub struct XorShift64 {
    state: u64,
}

impl XorShift64 {
    /// Constructs an [`XorShift64`] from an arbitrary `u64` seed. A seed of
    /// `0` would degenerate the algorithm to always producing `0`, so it is
    /// silently replaced with a fixed non-zero constant.
    pub fn seed_from_u64(seed: u64) -> Self {
        let state = if seed == 0 { 0x9E37_79B9_7F4A_7C15 } else { seed };
        Self { state }
    }

    /// Constructs a seed from the current wall clock and process id. This is
    /// best-effort and intentionally not cryptographically secure.
    pub fn from_entropy() -> Self {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_nanos() as u64)
            .unwrap_or(0xA5A5_A5A5_A5A5_A5A5);
        let pid = std::process::id() as u64;
        Self::seed_from_u64(nanos ^ pid.rotate_left(17))
    }

    /// Returns the current internal state. Useful for logging the seed of a
    /// failing test so the run can be reproduced.
    pub fn state(&self) -> u64 {
        self.state
    }
}

impl Rng for XorShift64 {
    fn next_u64(&mut self) -> u64 {
        // xorshift64* (Marsaglia / Vigna). Period 2^64 - 1.
        let mut x = self.state;
        x ^= x >> 12;
        x ^= x << 25;
        x ^= x >> 27;
        self.state = x;
        x.wrapping_mul(0x2545_F491_4F6C_DD1D)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn xorshift_is_deterministic_for_same_seed() {
        let mut a = XorShift64::seed_from_u64(42);
        let mut b = XorShift64::seed_from_u64(42);
        for _ in 0..1000 {
            assert_eq!(a.next_u64(), b.next_u64());
        }
    }

    #[test]
    fn xorshift_different_seeds_diverge() {
        let mut a = XorShift64::seed_from_u64(1);
        let mut b = XorShift64::seed_from_u64(2);
        assert_ne!(a.next_u64(), b.next_u64());
    }

    #[test]
    fn zero_seed_is_replaced() {
        let r = XorShift64::seed_from_u64(0);
        assert_ne!(r.state(), 0);
    }

    #[test]
    fn gen_range_respects_bounds() {
        let mut r = XorShift64::seed_from_u64(7);
        for _ in 0..1000 {
            let v = r.gen_range_u64(10, 20);
            assert!((10..20).contains(&v));
            let v = r.gen_range_i64(-5, 5);
            assert!((-5..5).contains(&v));
            let v = r.gen_range_usize(0, 8);
            assert!(v < 8);
        }
    }

    #[test]
    fn fill_bytes_fills_all() {
        let mut r = XorShift64::seed_from_u64(123);
        let mut buf = [0u8; 17];
        r.fill_bytes(&mut buf);
        // Sanity: at least one byte is non-zero (probability of all-zero is ~2^-136).
        assert!(buf.iter().any(|&b| b != 0));
    }

    #[test]
    fn choose_handles_empty() {
        let mut r = XorShift64::seed_from_u64(1);
        let empty: [i32; 0] = [];
        assert!(r.choose(&empty).is_none());
        let one = [42];
        assert_eq!(r.choose(&one), Some(&42));
    }
}
