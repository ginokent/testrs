//! `Arbitrary` impls for std collections, smart pointers, and a few common
//! "value" types from the standard library.

use crate::arbitrary::Arbitrary;
use crate::rng::Rng;
use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet, VecDeque};
use std::hash::Hash;
use std::ops::Range;
use std::time::Duration;

// --- VecDeque<T> --------------------------------------------------------

impl<T: Arbitrary> Arbitrary for VecDeque<T> {
    fn arbitrary<R: Rng + ?Sized>(rng: &mut R, size: usize) -> Self {
        let v: Vec<T> = Arbitrary::arbitrary(rng, size);
        v.into_iter().collect()
    }

    fn shrink(&self) -> Box<dyn Iterator<Item = Self> + '_> {
        let v: Vec<T> = self.iter().cloned().collect();
        let shrinks: Vec<Vec<T>> = v.shrink().collect();
        Box::new(shrinks.into_iter().map(|v| v.into_iter().collect()))
    }
}

// --- HashSet<T>, BTreeSet<T> --------------------------------------------

impl<T: Arbitrary + Eq + Hash> Arbitrary for HashSet<T> {
    fn arbitrary<R: Rng + ?Sized>(rng: &mut R, size: usize) -> Self {
        let v: Vec<T> = Arbitrary::arbitrary(rng, size);
        v.into_iter().collect()
    }

    fn shrink(&self) -> Box<dyn Iterator<Item = Self> + '_> {
        let v: Vec<T> = self.iter().cloned().collect();
        let shrinks: Vec<Vec<T>> = v.shrink().collect();
        Box::new(shrinks.into_iter().map(|v| v.into_iter().collect()))
    }
}

impl<T: Arbitrary + Ord> Arbitrary for BTreeSet<T> {
    fn arbitrary<R: Rng + ?Sized>(rng: &mut R, size: usize) -> Self {
        let v: Vec<T> = Arbitrary::arbitrary(rng, size);
        v.into_iter().collect()
    }

    fn shrink(&self) -> Box<dyn Iterator<Item = Self> + '_> {
        let v: Vec<T> = self.iter().cloned().collect();
        let shrinks: Vec<Vec<T>> = v.shrink().collect();
        Box::new(shrinks.into_iter().map(|v| v.into_iter().collect()))
    }
}

// --- HashMap<K, V>, BTreeMap<K, V> --------------------------------------

impl<K: Arbitrary + Eq + Hash, V: Arbitrary> Arbitrary for HashMap<K, V> {
    fn arbitrary<R: Rng + ?Sized>(rng: &mut R, size: usize) -> Self {
        let pairs: Vec<(K, V)> = Arbitrary::arbitrary(rng, size);
        pairs.into_iter().collect()
    }

    fn shrink(&self) -> Box<dyn Iterator<Item = Self> + '_> {
        let pairs: Vec<(K, V)> = self.iter().map(|(k, v)| (k.clone(), v.clone())).collect();
        let shrinks: Vec<Vec<(K, V)>> = pairs.shrink().collect();
        Box::new(shrinks.into_iter().map(|p| p.into_iter().collect()))
    }
}

impl<K: Arbitrary + Ord, V: Arbitrary> Arbitrary for BTreeMap<K, V> {
    fn arbitrary<R: Rng + ?Sized>(rng: &mut R, size: usize) -> Self {
        let pairs: Vec<(K, V)> = Arbitrary::arbitrary(rng, size);
        pairs.into_iter().collect()
    }

    fn shrink(&self) -> Box<dyn Iterator<Item = Self> + '_> {
        let pairs: Vec<(K, V)> = self.iter().map(|(k, v)| (k.clone(), v.clone())).collect();
        let shrinks: Vec<Vec<(K, V)>> = pairs.shrink().collect();
        Box::new(shrinks.into_iter().map(|p| p.into_iter().collect()))
    }
}

// --- fixed-size arrays [T; N] ------------------------------------------

impl<T: Arbitrary, const N: usize> Arbitrary for [T; N] {
    fn arbitrary<R: Rng + ?Sized>(rng: &mut R, size: usize) -> Self {
        let v: Vec<T> = (0..N).map(|_| T::arbitrary(rng, size)).collect();
        match v.try_into() {
            Ok(arr) => arr,
            Err(_) => unreachable!("Vec length == N by construction"),
        }
    }

    fn shrink(&self) -> Box<dyn Iterator<Item = Self> + '_> {
        let mut out: Vec<Self> = Vec::new();
        for i in 0..N {
            let shrinks: Vec<T> = self[i].shrink().collect();
            for s in shrinks {
                let mut arr = self.clone();
                arr[i] = s;
                out.push(arr);
            }
        }
        Box::new(out.into_iter())
    }
}

// --- Box<T> -------------------------------------------------------------

impl<T: Arbitrary> Arbitrary for Box<T> {
    fn arbitrary<R: Rng + ?Sized>(rng: &mut R, size: usize) -> Self {
        Box::new(T::arbitrary(rng, size))
    }

    fn shrink(&self) -> Box<dyn Iterator<Item = Self> + '_> {
        let v: Vec<T> = (**self).shrink().collect();
        Box::new(v.into_iter().map(Box::new))
    }
}

// --- std::time::Duration ------------------------------------------------

impl Arbitrary for Duration {
    fn arbitrary<R: Rng + ?Sized>(rng: &mut R, size: usize) -> Self {
        let secs = u64::arbitrary(rng, size);
        let nanos = rng.gen_range_u64(0, 1_000_000_000) as u32;
        Duration::new(secs, nanos)
    }

    fn shrink(&self) -> Box<dyn Iterator<Item = Self> + '_> {
        let secs = self.as_secs();
        let nanos = self.subsec_nanos();
        let mut out: Vec<Duration> = Vec::new();
        if *self != Duration::ZERO {
            out.push(Duration::ZERO);
        }
        for s in secs.shrink() {
            out.push(Duration::new(s, nanos));
        }
        if nanos > 0 {
            out.push(Duration::new(secs, 0));
        }
        Box::new(out.into_iter())
    }
}

// --- Range<T> (half-open) -----------------------------------------------

impl<T> Arbitrary for Range<T>
where
    T: Arbitrary + PartialOrd,
{
    fn arbitrary<R: Rng + ?Sized>(rng: &mut R, size: usize) -> Self {
        let a = T::arbitrary(rng, size);
        let b = T::arbitrary(rng, size);
        if a <= b {
            a..b
        } else {
            b..a
        }
    }

    fn shrink(&self) -> Box<dyn Iterator<Item = Self> + '_> {
        let mut out: Vec<Range<T>> = Vec::new();
        out.push(self.start.clone()..self.start.clone());
        for s in self.start.shrink() {
            if s <= self.end {
                out.push(s..self.end.clone());
            }
        }
        for e in self.end.shrink() {
            if self.start <= e {
                out.push(self.start.clone()..e);
            }
        }
        Box::new(out.into_iter())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rng::XorShift64;

    fn r() -> XorShift64 {
        XorShift64::seed_from_u64(0xBADC0DE)
    }

    #[test]
    fn hashmap_arbitrary_is_constructible() {
        let mut rng = r();
        let m: HashMap<u8, u8> = Arbitrary::arbitrary(&mut rng, 10);
        let _ = m.len();
    }

    #[test]
    fn hashset_shrinks_to_empty() {
        let mut s = HashSet::new();
        s.insert(1u32);
        s.insert(2u32);
        let v: Vec<HashSet<u32>> = s.shrink().collect();
        assert!(v.iter().any(|x| x.is_empty()));
    }

    #[test]
    fn btreemap_arbitrary_is_constructible() {
        let mut rng = r();
        let m: BTreeMap<u8, String> = Arbitrary::arbitrary(&mut rng, 5);
        let _ = m.len();
    }

    #[test]
    fn array_arbitrary_has_fixed_length() {
        let mut rng = r();
        let a: [u8; 5] = Arbitrary::arbitrary(&mut rng, 10);
        assert_eq!(a.len(), 5);
    }

    #[test]
    fn array_shrinks_each_slot() {
        let arr: [u32; 3] = [5, 10, 15];
        let v: Vec<[u32; 3]> = arr.shrink().collect();
        assert!(v.iter().any(|a| a[0] == 0));
    }

    #[test]
    fn boxed_shrinks_through_inner() {
        let b = Box::new(100u32);
        let v: Vec<Box<u32>> = b.shrink().collect();
        assert!(v.iter().any(|x| **x == 0));
    }

    #[test]
    fn duration_shrinks_to_zero() {
        let d = Duration::new(7, 500);
        let v: Vec<Duration> = d.shrink().collect();
        assert!(v.contains(&Duration::ZERO));
    }

    #[test]
    fn range_arbitrary_is_well_ordered() {
        let mut rng = r();
        for _ in 0..100 {
            let r: Range<i32> = Arbitrary::arbitrary(&mut rng, 50);
            assert!(r.start <= r.end);
        }
    }
}
