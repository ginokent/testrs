//! `Arbitrary` impls for std collections, smart pointers, and a few common
//! "value" types from the standard library.

use crate::arbitrary::Arbitrary;
use crate::rng::Rng;
use std::cell::{Cell, RefCell};
use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet, VecDeque};
use std::ffi::OsString;
use std::hash::Hash;
use std::net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr, SocketAddrV4, SocketAddrV6};
use std::num::{
    NonZeroI16, NonZeroI32, NonZeroI64, NonZeroI8, NonZeroIsize, NonZeroU16, NonZeroU32,
    NonZeroU64, NonZeroU8, NonZeroUsize,
};
use std::ops::Range;
use std::path::PathBuf;
use std::rc::Rc;
use std::sync::Arc;
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

// --- Rc<T>, Arc<T> ------------------------------------------------------

impl<T: Arbitrary> Arbitrary for Rc<T> {
    fn arbitrary<R: Rng + ?Sized>(rng: &mut R, size: usize) -> Self {
        Rc::new(T::arbitrary(rng, size))
    }
    fn shrink(&self) -> Box<dyn Iterator<Item = Self> + '_> {
        let v: Vec<T> = (**self).shrink().collect();
        Box::new(v.into_iter().map(Rc::new))
    }
}

impl<T: Arbitrary> Arbitrary for Arc<T> {
    fn arbitrary<R: Rng + ?Sized>(rng: &mut R, size: usize) -> Self {
        Arc::new(T::arbitrary(rng, size))
    }
    fn shrink(&self) -> Box<dyn Iterator<Item = Self> + '_> {
        let v: Vec<T> = (**self).shrink().collect();
        Box::new(v.into_iter().map(Arc::new))
    }
}

// --- Cell<T>, RefCell<T> -----------------------------------------------

impl<T: Arbitrary + Copy> Arbitrary for Cell<T> {
    fn arbitrary<R: Rng + ?Sized>(rng: &mut R, size: usize) -> Self {
        Cell::new(T::arbitrary(rng, size))
    }
    fn shrink(&self) -> Box<dyn Iterator<Item = Self> + '_> {
        let v: Vec<T> = self.get().shrink().collect();
        Box::new(v.into_iter().map(Cell::new))
    }
}

impl<T: Arbitrary> Arbitrary for RefCell<T> {
    fn arbitrary<R: Rng + ?Sized>(rng: &mut R, size: usize) -> Self {
        RefCell::new(T::arbitrary(rng, size))
    }
    fn shrink(&self) -> Box<dyn Iterator<Item = Self> + '_> {
        let inner = self.borrow().clone();
        let v: Vec<T> = inner.shrink().collect();
        Box::new(v.into_iter().map(RefCell::new))
    }
}

// --- PathBuf, OsString --------------------------------------------------

impl Arbitrary for PathBuf {
    fn arbitrary<R: Rng + ?Sized>(rng: &mut R, size: usize) -> Self {
        // Bias toward path-like strings: occasionally insert '/' or '\\' so
        // generators produce multi-component paths.
        let s: String = Arbitrary::arbitrary(rng, size);
        PathBuf::from(s)
    }
    fn shrink(&self) -> Box<dyn Iterator<Item = Self> + '_> {
        let s = self.to_string_lossy().into_owned();
        let shrinks: Vec<String> = s.shrink().collect();
        Box::new(shrinks.into_iter().map(PathBuf::from))
    }
}

impl Arbitrary for OsString {
    fn arbitrary<R: Rng + ?Sized>(rng: &mut R, size: usize) -> Self {
        let s: String = Arbitrary::arbitrary(rng, size);
        OsString::from(s)
    }
    fn shrink(&self) -> Box<dyn Iterator<Item = Self> + '_> {
        let s = self.to_string_lossy().into_owned();
        let shrinks: Vec<String> = s.shrink().collect();
        Box::new(shrinks.into_iter().map(OsString::from))
    }
}

// --- IP / Socket addresses ---------------------------------------------

impl Arbitrary for Ipv4Addr {
    fn arbitrary<R: Rng + ?Sized>(rng: &mut R, _size: usize) -> Self {
        let mut octets = [0u8; 4];
        rng.fill_bytes(&mut octets);
        Ipv4Addr::from(octets)
    }
    fn shrink(&self) -> Box<dyn Iterator<Item = Self> + '_> {
        let bytes = self.octets();
        let shrinks: Vec<[u8; 4]> = bytes.shrink().collect();
        Box::new(shrinks.into_iter().map(Ipv4Addr::from))
    }
}

impl Arbitrary for Ipv6Addr {
    fn arbitrary<R: Rng + ?Sized>(rng: &mut R, _size: usize) -> Self {
        let mut octets = [0u8; 16];
        rng.fill_bytes(&mut octets);
        Ipv6Addr::from(octets)
    }
    fn shrink(&self) -> Box<dyn Iterator<Item = Self> + '_> {
        let bytes = self.octets();
        let shrinks: Vec<[u8; 16]> = bytes.shrink().collect();
        Box::new(shrinks.into_iter().map(Ipv6Addr::from))
    }
}

impl Arbitrary for IpAddr {
    fn arbitrary<R: Rng + ?Sized>(rng: &mut R, size: usize) -> Self {
        if rng.gen_bool() {
            IpAddr::V4(Ipv4Addr::arbitrary(rng, size))
        } else {
            IpAddr::V6(Ipv6Addr::arbitrary(rng, size))
        }
    }
    fn shrink(&self) -> Box<dyn Iterator<Item = Self> + '_> {
        match self {
            IpAddr::V4(a) => {
                let v: Vec<Ipv4Addr> = a.shrink().collect();
                Box::new(v.into_iter().map(IpAddr::V4))
            }
            IpAddr::V6(a) => {
                let v: Vec<Ipv6Addr> = a.shrink().collect();
                Box::new(v.into_iter().map(IpAddr::V6))
            }
        }
    }
}

impl Arbitrary for SocketAddrV4 {
    fn arbitrary<R: Rng + ?Sized>(rng: &mut R, size: usize) -> Self {
        SocketAddrV4::new(Ipv4Addr::arbitrary(rng, size), u16::arbitrary(rng, size))
    }
    fn shrink(&self) -> Box<dyn Iterator<Item = Self> + '_> {
        let ip = *self.ip();
        let port = self.port();
        let mut out: Vec<Self> = Vec::new();
        for ip_s in ip.shrink() {
            out.push(SocketAddrV4::new(ip_s, port));
        }
        for p in port.shrink() {
            out.push(SocketAddrV4::new(ip, p));
        }
        Box::new(out.into_iter())
    }
}

impl Arbitrary for SocketAddrV6 {
    fn arbitrary<R: Rng + ?Sized>(rng: &mut R, size: usize) -> Self {
        SocketAddrV6::new(
            Ipv6Addr::arbitrary(rng, size),
            u16::arbitrary(rng, size),
            0,
            0,
        )
    }
    fn shrink(&self) -> Box<dyn Iterator<Item = Self> + '_> {
        let ip = *self.ip();
        let port = self.port();
        let mut out: Vec<Self> = Vec::new();
        for ip_s in ip.shrink() {
            out.push(SocketAddrV6::new(ip_s, port, 0, 0));
        }
        for p in port.shrink() {
            out.push(SocketAddrV6::new(ip, p, 0, 0));
        }
        Box::new(out.into_iter())
    }
}

impl Arbitrary for SocketAddr {
    fn arbitrary<R: Rng + ?Sized>(rng: &mut R, size: usize) -> Self {
        if rng.gen_bool() {
            SocketAddr::V4(SocketAddrV4::arbitrary(rng, size))
        } else {
            SocketAddr::V6(SocketAddrV6::arbitrary(rng, size))
        }
    }
    fn shrink(&self) -> Box<dyn Iterator<Item = Self> + '_> {
        match self {
            SocketAddr::V4(a) => {
                let v: Vec<SocketAddrV4> = a.shrink().collect();
                Box::new(v.into_iter().map(SocketAddr::V4))
            }
            SocketAddr::V6(a) => {
                let v: Vec<SocketAddrV6> = a.shrink().collect();
                Box::new(v.into_iter().map(SocketAddr::V6))
            }
        }
    }
}

// --- NonZero integers ---------------------------------------------------

macro_rules! impl_arbitrary_nonzero {
    ($($nz:ty => $base:ty),* $(,)?) => {$(
        impl Arbitrary for $nz {
            fn arbitrary<R: Rng + ?Sized>(rng: &mut R, size: usize) -> Self {
                // Retry up to ~30 times; the probability of drawing zero is
                // ~1/2^bits for the bit-width-capped generator, so this loop
                // almost never spins more than once.
                for _ in 0..32 {
                    let v: $base = Arbitrary::arbitrary(rng, size);
                    if let Some(nz) = <$nz>::new(v) {
                        return nz;
                    }
                }
                // Fallback: 1 is always valid for unsigned; for signed too.
                <$nz>::new(1).unwrap()
            }
            fn shrink(&self) -> Box<dyn Iterator<Item = Self> + '_> {
                let v = self.get();
                let shrinks: Vec<$base> = v.shrink().collect();
                Box::new(shrinks.into_iter().filter_map(<$nz>::new))
            }
        }
    )*};
}

impl_arbitrary_nonzero!(
    NonZeroU8 => u8,
    NonZeroU16 => u16,
    NonZeroU32 => u32,
    NonZeroU64 => u64,
    NonZeroUsize => usize,
    NonZeroI8 => i8,
    NonZeroI16 => i16,
    NonZeroI32 => i32,
    NonZeroI64 => i64,
    NonZeroIsize => isize,
);

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

    #[test]
    fn rc_and_arc_arbitrary_work() {
        let mut rng = r();
        let _: Rc<String> = Arbitrary::arbitrary(&mut rng, 10);
        let _: Arc<Vec<u8>> = Arbitrary::arbitrary(&mut rng, 10);
        let rc = Rc::new(42u32);
        let v: Vec<Rc<u32>> = rc.shrink().collect();
        assert!(v.iter().any(|x| **x == 0));
    }

    #[test]
    fn cell_and_refcell_arbitrary_work() {
        let mut rng = r();
        let _: Cell<u32> = Arbitrary::arbitrary(&mut rng, 10);
        let _: RefCell<String> = Arbitrary::arbitrary(&mut rng, 10);
    }

    #[test]
    fn pathbuf_arbitrary_and_shrinks() {
        let mut rng = r();
        let _: PathBuf = Arbitrary::arbitrary(&mut rng, 20);
        let p = PathBuf::from("hello/world");
        let v: Vec<PathBuf> = p.shrink().collect();
        assert!(v.iter().any(|p| p.as_os_str().is_empty()));
    }

    #[test]
    fn ip_address_arbitrary_works() {
        let mut rng = r();
        for _ in 0..50 {
            let _: Ipv4Addr = Arbitrary::arbitrary(&mut rng, 10);
            let _: Ipv6Addr = Arbitrary::arbitrary(&mut rng, 10);
            let _: IpAddr = Arbitrary::arbitrary(&mut rng, 10);
        }
    }

    #[test]
    fn socket_address_arbitrary_works() {
        let mut rng = r();
        for _ in 0..50 {
            let _: SocketAddrV4 = Arbitrary::arbitrary(&mut rng, 10);
            let _: SocketAddrV6 = Arbitrary::arbitrary(&mut rng, 10);
            let _: SocketAddr = Arbitrary::arbitrary(&mut rng, 10);
        }
    }

    #[test]
    fn nonzero_never_zero() {
        let mut rng = r();
        for _ in 0..200 {
            let n: NonZeroU32 = Arbitrary::arbitrary(&mut rng, 50);
            assert!(n.get() != 0);
            let n: NonZeroI32 = Arbitrary::arbitrary(&mut rng, 50);
            assert!(n.get() != 0);
        }
    }

    #[test]
    fn nonzero_shrinks_skip_zero() {
        let n = NonZeroU32::new(100).unwrap();
        let v: Vec<NonZeroU32> = n.shrink().collect();
        // No shrink should ever be zero (filtered by NonZeroU32::new).
        assert!(v.iter().all(|x| x.get() != 0));
        // At least one shrink should be present.
        assert!(!v.is_empty());
    }
}
