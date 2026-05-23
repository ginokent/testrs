//! Core abstractions for property-based testing and fuzzing.
//!
//! This crate is intentionally tiny and has zero external dependencies. It
//! exposes three things:
//!
//! - [`Rng`]: a minimal pseudo-random number generator trait.
//! - [`XorShift64`]: a fast, seedable PRNG implementing [`Rng`] (xorshift64*).
//! - [`Arbitrary`]: a trait for types that can be randomly generated and
//!   shrunk to smaller counterexamples.
//!
//! Higher-level test runners live in the sibling `propcheck` and
//! `propcheck-fuzz` crates.

pub mod arbitrary;
pub mod rng;

pub use arbitrary::Arbitrary;
pub use rng::{Rng, XorShift64};
