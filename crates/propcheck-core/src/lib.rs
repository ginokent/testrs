//! Core abstractions for property-based testing and fuzzing.
//!
//! Zero external dependencies. The crate exposes four pieces:
//!
//! - [`Rng`]: a minimal, object-safe pseudo-random number generator trait.
//! - [`XorShift64`]: a fast, seedable PRNG implementing [`Rng`].
//! - [`Arbitrary`]: trait for types that can be randomly generated and shrunk.
//! - [`strategy`]: composable generator combinators (`int_range`, `vec_of`,
//!   `one_of`, `.map`, `.filter`, …).

pub mod arbitrary;
pub mod collections;
pub mod rng;
pub mod strategy;

pub use arbitrary::Arbitrary;
pub use rng::{Rng, XorShift64};
pub use strategy::{Strategy, StrategyExt};
