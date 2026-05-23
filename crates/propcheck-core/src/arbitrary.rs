//! The [`Arbitrary`] trait and impls for common types.
//!
//! Implement [`Arbitrary`] for your own types to make them usable with
//! `propcheck`'s property-based test runner. The trait combines two
//! responsibilities:
//!
//! - **Generation:** produce a random value given an [`Rng`] and a `size`
//!   hint that callers typically grow over the course of a run.
//! - **Shrinking:** given a counterexample, propose smaller candidates that
//!   may also fail the property. The runner repeatedly walks the shrink tree
//!   to find a minimal counterexample.

use crate::rng::Rng;
use std::fmt::Debug;

/// Trait for types that can be randomly generated and shrunk.
///
/// The default `shrink` returns an empty iterator (i.e. "no smaller value
/// known"), which is a valid but unhelpful implementation for custom types.
pub trait Arbitrary: Sized + Clone + Debug {
    /// Generates a random value. `size` is an upper bound on the "magnitude"
    /// of the value (e.g. collection length, integer bit width). The runner
    /// typically grows `size` linearly across cases.
    fn arbitrary<R: Rng + ?Sized>(rng: &mut R, size: usize) -> Self;

    /// Returns an iterator of strictly smaller candidates. Smaller-first
    /// ordering is preferred so shrinking converges quickly.
    fn shrink(&self) -> Box<dyn Iterator<Item = Self> + '_> {
        Box::new(std::iter::empty())
    }
}

// --- bool ---------------------------------------------------------------

impl Arbitrary for bool {
    fn arbitrary<R: Rng + ?Sized>(rng: &mut R, _size: usize) -> Self {
        rng.gen_bool()
    }

    fn shrink(&self) -> Box<dyn Iterator<Item = Self> + '_> {
        if *self {
            Box::new(std::iter::once(false))
        } else {
            Box::new(std::iter::empty())
        }
    }
}

// --- unsigned integers --------------------------------------------------

fn shrink_toward_zero_u128(n: u128) -> Vec<u128> {
    if n == 0 {
        return Vec::new();
    }
    let mut out = vec![0u128];
    let mut delta = n;
    loop {
        delta /= 2;
        if delta == 0 {
            break;
        }
        let candidate = n - delta;
        if candidate != n && !out.contains(&candidate) {
            out.push(candidate);
        }
    }
    out
}

macro_rules! impl_arbitrary_uint {
    ($($t:ty),*) => {$(
        impl Arbitrary for $t {
            fn arbitrary<R: Rng + ?Sized>(rng: &mut R, size: usize) -> Self {
                // Bias toward small values early in a run by capping bit-width to ~2 * size.
                let bits = (<$t>::BITS as usize).min(size.saturating_mul(2).max(8));
                let mask: u128 = if bits >= 128 { u128::MAX } else { (1u128 << bits) - 1 };
                let high = rng.next_u64() as u128;
                let low = rng.next_u64() as u128;
                let v = ((high << 64) | low) & mask;
                v as $t
            }

            fn shrink(&self) -> Box<dyn Iterator<Item = Self> + '_> {
                let v: Vec<$t> = shrink_toward_zero_u128(*self as u128)
                    .into_iter()
                    .map(|x| x as $t)
                    .collect();
                Box::new(v.into_iter())
            }
        }
    )*};
}

impl_arbitrary_uint!(u8, u16, u32, u64, u128, usize);

// --- signed integers ----------------------------------------------------

fn shrink_signed_i128(n: i128) -> Vec<i128> {
    if n == 0 {
        return Vec::new();
    }
    let mut out = vec![0i128];
    // Try flipping sign (the absolute value) as a first move.
    if n < 0 && n != i128::MIN {
        let abs = -n;
        if !out.contains(&abs) {
            out.push(abs);
        }
    }
    let mut delta = n;
    loop {
        delta /= 2;
        if delta == 0 {
            break;
        }
        let candidate = n - delta;
        if candidate != n && !out.contains(&candidate) {
            out.push(candidate);
        }
    }
    out
}

macro_rules! impl_arbitrary_int {
    ($($t:ty),*) => {$(
        impl Arbitrary for $t {
            fn arbitrary<R: Rng + ?Sized>(rng: &mut R, size: usize) -> Self {
                let bits = (<$t>::BITS as usize).min(size.saturating_mul(2).max(8));
                let mask: u128 = if bits >= 128 { u128::MAX } else { (1u128 << bits) - 1 };
                let high = rng.next_u64() as u128;
                let low = rng.next_u64() as u128;
                let bits_val = ((high << 64) | low) & mask;
                // Sign-extend from `bits` to the full width by casting through
                // the unsigned and then signed views.
                let max_pos: u128 = if bits >= 128 { u128::MAX } else { (1u128 << bits) - 1 };
                let half = max_pos / 2 + 1;
                let signed = if bits_val >= half && bits < 128 {
                    bits_val as i128 - (1i128 << bits)
                } else {
                    bits_val as i128
                };
                signed as $t
            }

            fn shrink(&self) -> Box<dyn Iterator<Item = Self> + '_> {
                let v: Vec<$t> = shrink_signed_i128(*self as i128)
                    .into_iter()
                    .filter(|&x| x >= <$t>::MIN as i128 && x <= <$t>::MAX as i128)
                    .map(|x| x as $t)
                    .collect();
                Box::new(v.into_iter())
            }
        }
    )*};
}

impl_arbitrary_int!(i8, i16, i32, i64, i128, isize);

// --- floats -------------------------------------------------------------

macro_rules! impl_arbitrary_float {
    ($t:ty, $u:ty, $bits:expr) => {
        impl Arbitrary for $t {
            fn arbitrary<R: Rng + ?Sized>(rng: &mut R, size: usize) -> Self {
                // 10% chance of an "interesting" boundary value, otherwise a
                // random float in roughly [-size, size].
                let pick = rng.gen_range_u64(0, 10);
                if pick == 0 {
                    let interesting: [$t; 6] = [
                        0.0, -0.0, 1.0, -1.0,
                        <$t>::INFINITY, <$t>::NEG_INFINITY,
                    ];
                    return *rng.choose(&interesting).unwrap();
                }
                let bits = rng.next_u64() as $u;
                let f = <$t>::from_bits(bits);
                if !f.is_finite() {
                    return 0.0;
                }
                let bound = (size as $t).max(1.0);
                // Squash into [-bound, bound] using rem (cheap and bounded).
                f % bound
            }

            fn shrink(&self) -> Box<dyn Iterator<Item = Self> + '_> {
                if *self == 0.0 || self.is_nan() {
                    return Box::new(std::iter::empty());
                }
                let mut out: Vec<$t> = Vec::new();
                out.push(0.0);
                if *self < 0.0 && self.is_finite() {
                    out.push(-*self);
                }
                if self.is_finite() {
                    let truncated = self.trunc();
                    if truncated != *self {
                        out.push(truncated);
                    }
                    let halved = *self / 2.0;
                    if halved != *self && halved != 0.0 {
                        out.push(halved);
                    }
                }
                Box::new(out.into_iter())
            }
        }
    };
}

impl_arbitrary_float!(f32, u32, 32);
impl_arbitrary_float!(f64, u64, 64);

// --- char ---------------------------------------------------------------

impl Arbitrary for char {
    fn arbitrary<R: Rng + ?Sized>(rng: &mut R, _size: usize) -> Self {
        // 80% printable ASCII, 20% wider Unicode (avoiding surrogates).
        if rng.gen_range_u64(0, 5) > 0 {
            let code = rng.gen_range_u64(0x20, 0x7f) as u32;
            char::from_u32(code).unwrap_or('a')
        } else {
            loop {
                let code = rng.gen_range_u64(0, 0x11_0000) as u32;
                if let Some(c) = char::from_u32(code) {
                    return c;
                }
            }
        }
    }

    fn shrink(&self) -> Box<dyn Iterator<Item = Self> + '_> {
        if *self == 'a' {
            return Box::new(std::iter::empty());
        }
        let mut out: Vec<char> = Vec::new();
        out.push('a');
        let code = *self as u32;
        let target = 'a' as u32;
        if code > target {
            let mut delta = code - target;
            loop {
                delta /= 2;
                if delta == 0 {
                    break;
                }
                let candidate_code = code - delta;
                if let Some(c) = char::from_u32(candidate_code) {
                    if c != *self && !out.contains(&c) {
                        out.push(c);
                    }
                }
            }
        }
        Box::new(out.into_iter())
    }
}

// --- Vec<T> -------------------------------------------------------------

impl<T: Arbitrary> Arbitrary for Vec<T> {
    fn arbitrary<R: Rng + ?Sized>(rng: &mut R, size: usize) -> Self {
        let max = size.max(1);
        let len = rng.gen_range_usize(0, max + 1);
        (0..len).map(|_| T::arbitrary(rng, size)).collect()
    }

    fn shrink(&self) -> Box<dyn Iterator<Item = Self> + '_> {
        let len = self.len();
        let mut candidates: Vec<Vec<T>> = Vec::new();
        if len == 0 {
            return Box::new(candidates.into_iter());
        }
        // Empty vec is the most aggressive shrink.
        candidates.push(Vec::new());
        // Halve-length prefixes.
        let mut chunk = len;
        loop {
            chunk /= 2;
            if chunk == 0 || chunk == len {
                break;
            }
            candidates.push(self[..(len - chunk)].to_vec());
        }
        // Remove single element at each index.
        if len > 1 {
            for i in 0..len {
                let mut v = self.clone();
                v.remove(i);
                candidates.push(v);
            }
        }
        // Shrink each element in place.
        for i in 0..len {
            let shrinks: Vec<T> = self[i].shrink().collect();
            for s in shrinks {
                let mut v = self.clone();
                v[i] = s;
                candidates.push(v);
            }
        }
        Box::new(candidates.into_iter())
    }
}

// --- String -------------------------------------------------------------

impl Arbitrary for String {
    fn arbitrary<R: Rng + ?Sized>(rng: &mut R, size: usize) -> Self {
        let max = size.max(1);
        let len = rng.gen_range_usize(0, max + 1);
        (0..len).map(|_| char::arbitrary(rng, size)).collect()
    }

    fn shrink(&self) -> Box<dyn Iterator<Item = Self> + '_> {
        let chars: Vec<char> = self.chars().collect();
        let candidates: Vec<Vec<char>> = chars.shrink().collect();
        Box::new(
            candidates
                .into_iter()
                .map(|cs| cs.into_iter().collect::<String>()),
        )
    }
}

// --- Option<T> ----------------------------------------------------------

impl<T: Arbitrary> Arbitrary for Option<T> {
    fn arbitrary<R: Rng + ?Sized>(rng: &mut R, size: usize) -> Self {
        if rng.gen_bool() {
            Some(T::arbitrary(rng, size))
        } else {
            None
        }
    }

    fn shrink(&self) -> Box<dyn Iterator<Item = Self> + '_> {
        match self {
            None => Box::new(std::iter::empty()),
            Some(x) => {
                let mut out: Vec<Option<T>> = vec![None];
                for s in x.shrink() {
                    out.push(Some(s));
                }
                Box::new(out.into_iter())
            }
        }
    }
}

// --- Result<T, E> -------------------------------------------------------

impl<T: Arbitrary, E: Arbitrary> Arbitrary for Result<T, E> {
    fn arbitrary<R: Rng + ?Sized>(rng: &mut R, size: usize) -> Self {
        if rng.gen_bool() {
            Ok(T::arbitrary(rng, size))
        } else {
            Err(E::arbitrary(rng, size))
        }
    }

    fn shrink(&self) -> Box<dyn Iterator<Item = Self> + '_> {
        match self {
            Ok(t) => {
                let v: Vec<Self> = t.shrink().map(Ok).collect();
                Box::new(v.into_iter())
            }
            Err(e) => {
                let v: Vec<Self> = e.shrink().map(Err).collect();
                Box::new(v.into_iter())
            }
        }
    }
}

// --- tuples -------------------------------------------------------------

impl<A: Arbitrary, B: Arbitrary> Arbitrary for (A, B) {
    fn arbitrary<R: Rng + ?Sized>(rng: &mut R, size: usize) -> Self {
        (A::arbitrary(rng, size), B::arbitrary(rng, size))
    }
    fn shrink(&self) -> Box<dyn Iterator<Item = Self> + '_> {
        let a_shrinks: Vec<A> = self.0.shrink().collect();
        let b_shrinks: Vec<B> = self.1.shrink().collect();
        let (a, b) = self.clone();
        let mut out: Vec<Self> = Vec::with_capacity(a_shrinks.len() + b_shrinks.len());
        for sa in a_shrinks {
            out.push((sa, b.clone()));
        }
        for sb in b_shrinks {
            out.push((a.clone(), sb));
        }
        Box::new(out.into_iter())
    }
}

impl<A: Arbitrary, B: Arbitrary, C: Arbitrary> Arbitrary for (A, B, C) {
    fn arbitrary<R: Rng + ?Sized>(rng: &mut R, size: usize) -> Self {
        (
            A::arbitrary(rng, size),
            B::arbitrary(rng, size),
            C::arbitrary(rng, size),
        )
    }
    fn shrink(&self) -> Box<dyn Iterator<Item = Self> + '_> {
        let (a, b, c) = self.clone();
        let a_s: Vec<A> = self.0.shrink().collect();
        let b_s: Vec<B> = self.1.shrink().collect();
        let c_s: Vec<C> = self.2.shrink().collect();
        let mut out: Vec<Self> = Vec::with_capacity(a_s.len() + b_s.len() + c_s.len());
        for sa in a_s {
            out.push((sa, b.clone(), c.clone()));
        }
        for sb in b_s {
            out.push((a.clone(), sb, c.clone()));
        }
        for sc in c_s {
            out.push((a.clone(), b.clone(), sc));
        }
        Box::new(out.into_iter())
    }
}

impl<A: Arbitrary, B: Arbitrary, C: Arbitrary, D: Arbitrary> Arbitrary for (A, B, C, D) {
    fn arbitrary<R: Rng + ?Sized>(rng: &mut R, size: usize) -> Self {
        (
            A::arbitrary(rng, size),
            B::arbitrary(rng, size),
            C::arbitrary(rng, size),
            D::arbitrary(rng, size),
        )
    }
    fn shrink(&self) -> Box<dyn Iterator<Item = Self> + '_> {
        let (a, b, c, d) = self.clone();
        let a_s: Vec<A> = self.0.shrink().collect();
        let b_s: Vec<B> = self.1.shrink().collect();
        let c_s: Vec<C> = self.2.shrink().collect();
        let d_s: Vec<D> = self.3.shrink().collect();
        let mut out: Vec<Self> =
            Vec::with_capacity(a_s.len() + b_s.len() + c_s.len() + d_s.len());
        for sa in a_s {
            out.push((sa, b.clone(), c.clone(), d.clone()));
        }
        for sb in b_s {
            out.push((a.clone(), sb, c.clone(), d.clone()));
        }
        for sc in c_s {
            out.push((a.clone(), b.clone(), sc, d.clone()));
        }
        for sd in d_s {
            out.push((a.clone(), b.clone(), c.clone(), sd));
        }
        Box::new(out.into_iter())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rng::XorShift64;

    fn r() -> XorShift64 {
        XorShift64::seed_from_u64(0xC0FFEE)
    }

    #[test]
    fn bool_shrinks_true_to_false() {
        assert_eq!(true.shrink().collect::<Vec<_>>(), vec![false]);
        assert!(false.shrink().next().is_none());
    }

    #[test]
    fn unsigned_shrink_starts_with_zero() {
        let candidates: Vec<u32> = 100u32.shrink().collect();
        assert_eq!(candidates[0], 0);
        assert!(candidates.iter().all(|&x| x < 100));
        assert!(0u32.shrink().next().is_none());
    }

    #[test]
    fn signed_shrink_handles_negative() {
        let candidates: Vec<i32> = (-100i32).shrink().collect();
        assert_eq!(candidates[0], 0);
        assert!(candidates.contains(&100));
        assert!(0i32.shrink().next().is_none());
    }

    #[test]
    fn signed_shrink_handles_min() {
        // Must not panic on i8::MIN despite -i8::MIN overflowing.
        let candidates: Vec<i8> = i8::MIN.shrink().collect();
        assert_eq!(candidates[0], 0);
    }

    #[test]
    fn vec_shrink_yields_empty_first() {
        let v = vec![1, 2, 3];
        let candidates: Vec<Vec<i32>> = v.shrink().collect();
        assert_eq!(candidates[0], Vec::<i32>::new());
        assert!(Vec::<i32>::new().shrink().next().is_none());
    }

    #[test]
    fn string_shrink_yields_empty_first() {
        let s = String::from("hello");
        let candidates: Vec<String> = s.shrink().collect();
        assert_eq!(candidates[0], "");
    }

    #[test]
    fn option_shrink_drops_to_none() {
        let candidates: Vec<Option<u32>> = Some(5u32).shrink().collect();
        assert_eq!(candidates[0], None);
    }

    #[test]
    fn arbitrary_is_deterministic_under_seed() {
        let mut a = r();
        let mut b = r();
        for _ in 0..50 {
            let va: Vec<i32> = Arbitrary::arbitrary(&mut a, 20);
            let vb: Vec<i32> = Arbitrary::arbitrary(&mut b, 20);
            assert_eq!(va, vb);
        }
    }

    #[test]
    fn arbitrary_string_is_valid_utf8() {
        let mut rng = r();
        for _ in 0..100 {
            let s: String = Arbitrary::arbitrary(&mut rng, 20);
            // If it compiles and was constructed, it's valid UTF-8 by
            // construction. Just exercise it to make sure no panics.
            let _ = s.len();
        }
    }
}
