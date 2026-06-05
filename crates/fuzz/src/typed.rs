//! [`crate::fuzz`] の上に構築された、型付き入力の fuzzing です。
//!
//! バイト指向の fuzzer は `&[u8]` を直接ミューテートするため、構造化された
//! 入力を取る API のテストには扱いづらいです。[`fuzz_typed`] はミューテート
//! されたバイト列から seed 可能な PRNG を導出し、`T: Arbitrary` に値を要求
//! することで、任意の `Arbitrary` 型に対して mutation 駆動のカバレッジ
//! を提供します。

use testrs_core::{Arbitrary, XorShift64};

use crate::{fuzz, FuzzConfig, FuzzReport};

/// 型付き fuzzing 固有の調整可能パラメータです。
#[derive(Debug, Clone)]
pub struct TypedFuzzConfig {
    /// 基盤となるバイト fuzzer の設定（イテレーション数、seed、corpus 等を制御します）。
    pub inner: FuzzConfig,
    /// `T::arbitrary` に渡されるサイズヒントです。大きな値はより大きい /
    /// より深い構造を生成します。
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

/// 型付きの値を取るターゲットを fuzzing します。mutator ーが生成する
/// 各バイトバッファは [`XorShift64`] PRNG の seed（および補助エントロピー）
/// として解釈され、その PRNG が `T::arbitrary` を駆動して `target` に渡される
/// 値を構築します。
///
/// ```no_run
/// use testrs_fuzz::{fuzz_typed, TypedFuzzConfig};
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

/// 小さなミキシング関数を用いてバイトスライスを u64 の seed へハッシュします。
/// 目的は単に、異なるバイト列が異なる seed へマップされることだけであり、
/// 暗号学的な性質は必要としません。
fn seed_from_bytes(bytes: &[u8]) -> u64 {
    // FNV-1a 風のミックスを行い、その後最終的なアバランチステップを適用します。
    // XorShift64 は 0 を嫌うため、空入力に対しては固定の非ゼロ seed を返します。
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for &b in bytes {
        h ^= b as u64;
        h = h.wrapping_mul(0x100_0000_01b3);
    }
    // アバランチ
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
        // バグ：生成された u32 が偶然小さく偶数だった場合に panic します。
        // u32 への Arbitrary の利用は完全に seed バイトによって駆動されます。
        let cfg = TypedFuzzConfig {
            inner: FuzzConfig {
                iterations: 100_000,
                max_input_len: 32,
                seed: 0x5EED_C0DE,
                initial_corpus: vec![b"seed-input".to_vec()],
                minimize_steps: 100,
                silence_panic_hook: false,
                ..FuzzConfig::default()
            },
            size: 16,
        };
        let report = fuzz_typed::<u8, _>(cfg, |n: &u8| {
            if *n == 0xAB {
                panic!("triggered on {n:#x}");
            }
        });
        assert!(report.failure().is_some(), "should find a crash");
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
                ..FuzzConfig::default()
            },
            size: 16,
        };
        let report = fuzz_typed::<Vec<u8>, _>(cfg, |_v: &Vec<u8>| {});
        assert!(report.failure().is_none());
    }
}
