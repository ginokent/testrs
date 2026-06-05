//! differential なプロパティテストを提供します。
//!
//! 同じ関数の 2 つの実装をランダムな入力に対して実行し、両者が同一の出力を
//! 生成することを assert します。よく使われる用途としては、新しい実装を
//! 既知の正しい参照実装と比較したり、2 つの異なるアプローチを互いに比較する
//! ことが挙げられます。

use std::fmt::Debug;

use crate::{prop_assert_eq, run_with, Arbitrary, Config};

/// ランダムに生成されたあらゆる `I` について `lhs(input) == rhs(input)` であることを
/// assert します。不一致があった場合、runner は両方の出力と shrink された入力を
/// 報告します。
///
/// ```ignore
/// fn slow_sort(v: &Vec<i32>) -> Vec<i32> { /* ... */ }
/// fn fast_sort(v: &Vec<i32>) -> Vec<i32> { /* ... */ }
/// testrs_pbt::differential("sort agrees", &slow_sort, &fast_sort);
/// ```
pub fn differential<I, O, F, G>(name: &str, lhs: F, rhs: G)
where
    I: Arbitrary,
    O: PartialEq + Debug,
    F: Fn(&I) -> O,
    G: Fn(&I) -> O,
{
    differential_with(name, Config::default(), lhs, rhs)
}

/// [`differential`] と同じですが、カスタムの [`Config`] を指定できます。
pub fn differential_with<I, O, F, G>(name: &str, cfg: Config, lhs: F, rhs: G)
where
    I: Arbitrary,
    O: PartialEq + Debug,
    F: Fn(&I) -> O,
    G: Fn(&I) -> O,
{
    // closure でキャプチャする形式を使用することで、各ケースが `Clone` で
    // 個別のコピーを取得するのではなく、1 つの `lhs` / `rhs` を実行全体で
    // 共有できるようにしています。
    let lhs = &lhs;
    let rhs = &rhs;
    run_with(name, cfg, move |input: &I| -> bool {
        let l = lhs(input);
        let r = rhs(input);
        prop_assert_eq!(l, r);
        true
    });
}
