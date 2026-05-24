//! 小さな擬似乱数生成器です。
//!
//! [`XorShift64`] は Marsaglia/Vigna の xorshift64* アルゴリズムを実装します。
//! 暗号学的に安全ではありませんが、高速で、与えられた seed に対して
//! 決定的に動作し、ランダム化テストには十分です。

use std::time::{SystemTime, UNIX_EPOCH};

/// [`crate::Arbitrary`] generator が利用する最小限の PRNG trait です。
///
/// 実装者は [`Rng::next_u64`] のみを提供すればよく、その他のメソッドは
/// (常に最適とは限らないものの) 正しいデフォルト実装を備えています。
pub trait Rng {
    /// 次の 64 ビットの乱数を返します。
    fn next_u64(&mut self) -> u64;

    /// 次の 32 ビットの乱数を返します。
    fn next_u32(&mut self) -> u32 {
        self.next_u64() as u32
    }

    /// 一様乱数ビットを `bool` として返します。
    fn gen_bool(&mut self) -> bool {
        (self.next_u64() & 1) == 1
    }

    /// `[lo, hi)` の範囲の `u64` を返します。
    ///
    /// 剰余による削減を用いているため、`hi - lo` が `u64::MAX + 1` を
    /// 割り切らない場合にわずかな偏りが生じます。テスト用途には許容されます。
    fn gen_range_u64(&mut self, lo: u64, hi: u64) -> u64 {
        assert!(lo < hi, "gen_range_u64: lo ({lo}) must be < hi ({hi})");
        let span = hi - lo;
        lo + (self.next_u64() % span)
    }

    /// `[lo, hi)` の範囲の `i64` を返します。
    fn gen_range_i64(&mut self, lo: i64, hi: i64) -> i64 {
        assert!(lo < hi, "gen_range_i64: lo ({lo}) must be < hi ({hi})");
        let span = (hi as i128 - lo as i128) as u128;
        let r = (self.next_u64() as u128) % span;
        (lo as i128 + r as i128) as i64
    }

    /// `[lo, hi)` の範囲の `usize` を返します。
    fn gen_range_usize(&mut self, lo: usize, hi: usize) -> usize {
        assert!(lo < hi, "gen_range_usize: lo ({lo}) must be < hi ({hi})");
        let span = (hi - lo) as u64;
        lo + (self.next_u64() % span) as usize
    }

    /// `buf` をランダムなバイトで埋めます。
    fn fill_bytes(&mut self, buf: &mut [u8]) {
        let mut i = 0;
        while i < buf.len() {
            let chunk = self.next_u64().to_le_bytes();
            let n = (buf.len() - i).min(8);
            buf[i..i + n].copy_from_slice(&chunk[..n]);
            i += n;
        }
    }

    /// `slice` の要素への一様乱数な参照を返します。スライスが空の場合は
    /// `None` を返します。
    ///
    /// `Self: Sized` で制約しているのは、trait の残りの部分をオブジェクト安全に
    /// 保つためです。`&mut dyn Rng` を経由した内部ディスパッチには、
    /// ディスパッチ対象 trait にジェネリックメソッドが存在しないことが必要です。
    fn choose<'a, T>(&mut self, slice: &'a [T]) -> Option<&'a T>
    where
        Self: Sized,
    {
        if slice.is_empty() {
            None
        } else {
            Some(&slice[self.gen_range_usize(0, slice.len())])
        }
    }
}

/// xorshift64* PRNG です。与えられた seed に対して決定的に動作します。
#[derive(Debug, Clone)]
pub struct XorShift64 {
    state: u64,
}

impl XorShift64 {
    /// 任意の `u64` seed から [`XorShift64`] を構築します。seed が `0`
    /// の場合、アルゴリズムが常に `0` を生成するように退化してしまうため、
    /// 暗黙のうちに固定の非ゼロ定数で置き換えられます。
    pub fn seed_from_u64(seed: u64) -> Self {
        let state = if seed == 0 {
            0x9E37_79B9_7F4A_7C15
        } else {
            seed
        };
        Self { state }
    }

    /// 現在の実時間とプロセス ID から seed を構築します。これはベストエフォートで、
    /// 意図的に暗号学的安全性を持たせていません。
    pub fn from_entropy() -> Self {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_nanos() as u64)
            .unwrap_or(0xA5A5_A5A5_A5A5_A5A5);
        let pid = std::process::id() as u64;
        Self::seed_from_u64(nanos ^ pid.rotate_left(17))
    }

    /// 現在の内部状態を返します。失敗したテストの seed をログに記録し、
    /// 実行を再現できるようにするのに便利です。
    pub fn state(&self) -> u64 {
        self.state
    }
}

impl Rng for XorShift64 {
    fn next_u64(&mut self) -> u64 {
        // xorshift64* (Marsaglia / Vigna)。周期は 2^64 - 1。
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
        // サニティチェック: 少なくとも 1 バイトが非ゼロであること (すべてゼロになる確率は約 2^-136)。
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
