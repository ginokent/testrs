//! プロパティベーステストとファジング向けのコア抽象です。
//!
//! 外部依存はありません。このクレートは4つの要素を公開します。
//!
//! - [`Rng`] — 最小限かつオブジェクトセーフな擬似乱数生成器トレイトです。
//! - [`XorShift64`] — [`Rng`] を実装する高速でシード可能な PRNG です。
//! - [`Arbitrary`] — 値をランダム生成し shrink できる型のためのトレイトです。
//! - [`strategy`] — 組み合わせ可能なジェネレータコンビネータ群です (`int_range`、`vec_of`、
//!   `one_of`、`.map`、`.filter` など)。

pub mod arbitrary;
pub mod collections;
pub mod rng;
pub mod strategy;
mod strategy_str;

pub use arbitrary::Arbitrary;
pub use rng::{Rng, XorShift64};
pub use strategy::{Strategy, StrategyExt};
