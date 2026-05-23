//! Differential property testing.
//!
//! Runs two implementations of the same function against random inputs
//! and asserts they produce identical outputs. Commonly used to compare a
//! new implementation against a known-good reference, or two different
//! approaches against each other.

use std::fmt::Debug;

use crate::{prop_assert_eq, run_with, Arbitrary, Config};

/// Asserts that `lhs(input) == rhs(input)` for every randomly generated
/// `I`. On disagreement, the runner reports both outputs and the shrunk
/// input.
///
/// ```ignore
/// fn slow_sort(v: &Vec<i32>) -> Vec<i32> { /* ... */ }
/// fn fast_sort(v: &Vec<i32>) -> Vec<i32> { /* ... */ }
/// propcheck::differential("sort agrees", &slow_sort, &fast_sort);
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

/// Same as [`differential`] but with a custom [`Config`].
pub fn differential_with<I, O, F, G>(name: &str, cfg: Config, lhs: F, rhs: G)
where
    I: Arbitrary,
    O: PartialEq + Debug,
    F: Fn(&I) -> O,
    G: Fn(&I) -> O,
{
    // Use the closure-capturing form so a single `lhs` / `rhs` can be
    // shared across the whole run (rather than each case getting its own
    // copy through `Clone`).
    let lhs = &lhs;
    let rhs = &rhs;
    run_with(name, cfg, move |input: &I| -> bool {
        let l = lhs(input);
        let r = rhs(input);
        prop_assert_eq!(l, r);
        true
    });
}
