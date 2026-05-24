//! [`Arbitrary`] トレイトと一般的な型に対するその実装です。
//!
//! 独自の型に対して [`Arbitrary`] を実装すれば、`propcheck` のプロパティベース
//! テストランナーで使えるようになります。このトレイトは2つの責務を兼ねています。
//!
//! - **生成:** [`Rng`] と `size` ヒントを受け取ってランダム値を生成します。
//!   呼び出し側は通常、実行中に `size` を徐々に大きくしていきます。
//! - **Shrinking:** 反例が与えられたとき、同じくプロパティを失敗させうる
//!   より小さい候補を提案します。ランナーはshrinkツリーを繰り返したどって
//!   最小の反例を探索します。

use crate::rng::Rng;
use std::fmt::Debug;

/// 値をランダム生成しshrinkできる型のためのトレイトです。
///
/// デフォルトの `shrink` は空のイテレータを返します（つまり「より小さい値は
/// 知られていない」という意味）。これは正しい実装ですが、独自の型にとっては
/// 役に立たない実装です。
pub trait Arbitrary: Sized + Clone + Debug {
    /// ランダムな値を生成します。`size` は値の「大きさ」（例えばコレクションの
    /// 長さや整数のビット幅など）の上限です。ランナーは通常、ケースをまたいで
    /// `size` を線形に増やしていきます。
    fn arbitrary<R: Rng + ?Sized>(rng: &mut R, size: usize) -> Self;

    /// 厳密により小さい候補を返すイテレータです。shrinkが速く収束するように、
    /// 小さいものを先に並べる順序が好まれます。
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

// --- 符号なし整数 -------------------------------------------------------

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
                // 実行序盤では小さい値に偏らせるため、ビット幅を約 2 * size に制限します。
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

// --- 符号付き整数 -------------------------------------------------------

fn shrink_signed_i128(n: i128) -> Vec<i128> {
    if n == 0 {
        return Vec::new();
    }
    let mut out = vec![0i128];
    // 最初の手として、符号を反転した値（絶対値）を試します。
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
                // 符号なし、次に符号付きビューを経由してキャストすることで、
                // `bits` から完全な幅へ符号拡張します。
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

// --- 浮動小数点数 -------------------------------------------------------

macro_rules! impl_arbitrary_float {
    ($t:ty, $u:ty, $bits:expr) => {
        impl Arbitrary for $t {
            fn arbitrary<R: Rng + ?Sized>(rng: &mut R, size: usize) -> Self {
                // 約15%の確率で「興味深い」境界値を返します。これにはゼロ、
                // 単位値、非正規化数、max/min、NaN、無限大、整数境界に近い
                // 値が含まれます。
                let pick = rng.gen_range_u64(0, 20);
                if pick < 3 {
                    let interesting: [$t; 14] = [
                        0.0,
                        -0.0,
                        1.0,
                        -1.0,
                        0.5,
                        -0.5,
                        <$t>::INFINITY,
                        <$t>::NEG_INFINITY,
                        <$t>::NAN,
                        <$t>::MIN_POSITIVE,
                        -<$t>::MIN_POSITIVE,
                        <$t>::MIN,
                        <$t>::MAX,
                        <$t>::EPSILON,
                    ];
                    let idx = rng.gen_range_usize(0, interesting.len());
                    return interesting[idx];
                }
                let bits = rng.next_u64() as $u;
                let f = <$t>::from_bits(bits);
                if !f.is_finite() {
                    return 0.0;
                }
                let bound = (size as $t).max(1.0);
                f % bound
            }

            fn shrink(&self) -> Box<dyn Iterator<Item = Self> + '_> {
                if *self == 0.0 || self.is_nan() {
                    return Box::new(std::iter::empty());
                }
                let mut out: Vec<$t> = Vec::new();
                // 常に最も強いshrinkから先に試します。
                out.push(0.0);
                if *self < 0.0 && self.is_finite() {
                    out.push(-*self);
                }
                if self.is_finite() {
                    let truncated = self.trunc();
                    if truncated != *self {
                        out.push(truncated);
                    }
                    // 最も近い整数へのスナップを試します（デフォルトでは
                    // 偶数丸めですが、ここでは round() でも問題ありません）。
                    let rounded = self.round();
                    if rounded != *self && rounded != truncated {
                        out.push(rounded);
                    }
                    let halved = *self / 2.0;
                    if halved != *self && halved != 0.0 {
                        out.push(halved);
                    }
                    // 細粒度のshrinkのためにULP単位でゼロに向かって1ステップ進めます。
                    let bits = self.to_bits();
                    if bits != 0 {
                        let stepped = <$t>::from_bits(bits - 1);
                        if stepped.abs() < self.abs() {
                            out.push(stepped);
                        }
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
        // 80%は印字可能なASCII、20%はより広範なUnicode（サロゲートは除く）です。
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
        // 空の vec が最も強いshrinkです。
        candidates.push(Vec::new());
        // 長さを半分にしたプレフィックス。
        let mut chunk = len;
        loop {
            chunk /= 2;
            if chunk == 0 || chunk == len {
                break;
            }
            candidates.push(self[..(len - chunk)].to_vec());
        }
        // 各インデックスから要素を1つずつ取り除きます。
        if len > 1 {
            for i in 0..len {
                let mut v = self.clone();
                v.remove(i);
                candidates.push(v);
            }
        }
        // 各要素をその場でshrinkします。
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

// --- タプル -------------------------------------------------------------

impl Arbitrary for () {
    fn arbitrary<R: Rng + ?Sized>(_rng: &mut R, _size: usize) -> Self {}
    fn shrink(&self) -> Box<dyn Iterator<Item = Self> + '_> {
        Box::new(std::iter::empty())
    }
}

impl<A: Arbitrary> Arbitrary for (A,) {
    fn arbitrary<R: Rng + ?Sized>(rng: &mut R, size: usize) -> Self {
        (A::arbitrary(rng, size),)
    }
    fn shrink(&self) -> Box<dyn Iterator<Item = Self> + '_> {
        let v: Vec<A> = self.0.shrink().collect();
        Box::new(v.into_iter().map(|a| (a,)))
    }
}

macro_rules! impl_arbitrary_tuple {
    ($($name:ident: $idx:tt),+) => {
        impl<$($name: Arbitrary),+> Arbitrary for ($($name,)+) {
            fn arbitrary<R: Rng + ?Sized>(rng: &mut R, size: usize) -> Self {
                ($($name::arbitrary(rng, size),)+)
            }
            fn shrink(&self) -> Box<dyn Iterator<Item = Self> + '_> {
                let mut out: Vec<Self> = Vec::new();
                let cloned = self.clone();
                $(
                    {
                        let shrinks: Vec<_> = self.$idx.shrink().collect();
                        for s in shrinks {
                            let mut new_tup = cloned.clone();
                            new_tup.$idx = s;
                            out.push(new_tup);
                        }
                    }
                )+
                Box::new(out.into_iter())
            }
        }
    };
}

impl_arbitrary_tuple!(A: 0, B: 1);
impl_arbitrary_tuple!(A: 0, B: 1, C: 2);
impl_arbitrary_tuple!(A: 0, B: 1, C: 2, D: 3);
impl_arbitrary_tuple!(A: 0, B: 1, C: 2, D: 3, E: 4);
impl_arbitrary_tuple!(A: 0, B: 1, C: 2, D: 3, E: 4, F: 5);
impl_arbitrary_tuple!(A: 0, B: 1, C: 2, D: 3, E: 4, F: 5, G: 6);
impl_arbitrary_tuple!(A: 0, B: 1, C: 2, D: 3, E: 4, F: 5, G: 6, H: 7);

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
        // -i8::MIN がオーバーフローするにもかかわらず、i8::MIN でも panic してはいけません。
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
            // コンパイルが通って構築できれば、構築の仕方から有効な UTF-8 です。
            // panic しないことを確認するためだけに使ってみます。
            let _ = s.len();
        }
    }
}
