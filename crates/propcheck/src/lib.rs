//! Property-based test runner.
//!
//! Use [`run`] from inside a `#[test]` function to assert a property over
//! many randomly generated inputs:
//!
//! ```
//! use propcheck::{run, prop_assert_eq};
//!
//! run("addition is commutative", |&(a, b): &(i32, i32)| {
//!     prop_assert_eq!(a.wrapping_add(b), b.wrapping_add(a));
//!     true
//! });
//! ```
//!
//! Inside a property body you can use:
//!
//! - [`prop_assert!`], [`prop_assert_eq!`], [`prop_assert_ne!`] for
//!   assertions with rich failure messages.
//! - [`prop_assume!`] to discard cases that don't satisfy a precondition.
//!
//! Failure output includes the seed of the run so it can be reproduced
//! deterministically by setting the `PROPCHECK_SEED` env var.

use std::any::Any;
use std::env;
use std::panic::{self, AssertUnwindSafe};

pub use propcheck_core::{Arbitrary, Rng, XorShift64};

mod assert;
mod panic_hook;
pub mod strategy_runner;

pub use assert::{PropAssertFailure, PropDiscard};
pub use propcheck_core::strategy;
// `Arbitrary` as both the trait (from propcheck-core, type namespace) and the
// derive macro (from propcheck-derive, macro namespace). These live in
// different namespaces so the same name is fine.
pub use propcheck_derive::{propcheck, Arbitrary};
pub use strategy_runner::{forall_strategy, forall_strategy_with, run_strategy, run_strategy_with};

use panic_hook::SilentPanicHook;

/// Tunables for a property-based test run.
#[derive(Debug, Clone)]
pub struct Config {
    /// Target number of *passing* cases to execute.
    pub cases: usize,
    /// PRNG seed. Defaults to `PROPCHECK_SEED` env var or wall-clock entropy.
    pub seed: u64,
    /// Maximum number of shrink steps applied to a failing case.
    pub max_shrinks: usize,
    /// Upper bound on the size hint passed to `Arbitrary::arbitrary`.
    pub max_size: usize,
    /// Maximum total discards allowed before the run is aborted. Defaults
    /// to `cases * 10` — i.e. up to ten discards per kept case.
    pub max_discards: usize,
    /// If `true`, the global panic hook is silenced while the run executes
    /// so failing cases don't spam the terminal. Refcounted internally so
    /// concurrent runners share a single hook installation.
    pub silence_panic_hook: bool,
}

impl Default for Config {
    fn default() -> Self {
        let cases = 100;
        Self {
            cases,
            seed: env_seed(),
            max_shrinks: 1024,
            max_size: 100,
            max_discards: cases * 10,
            silence_panic_hook: true,
        }
    }
}

fn env_seed() -> u64 {
    if let Ok(s) = env::var("PROPCHECK_SEED") {
        if let Ok(n) = s.parse::<u64>() {
            return n;
        }
    }
    XorShift64::from_entropy().state()
}

/// Outcome of a property test.
#[derive(Debug)]
pub enum Outcome<A> {
    /// All required cases passed (possibly after some discards).
    Passed {
        /// Number of cases that ran the property to completion.
        cases: usize,
        /// Number of cases discarded via `prop_assume!`.
        discarded: usize,
    },
    /// A case failed.
    Failed {
        /// The first generated value that failed the property.
        original: A,
        /// The minimized counterexample after shrinking.
        shrunk: A,
        /// Human-readable failure reason.
        message: String,
        /// PRNG seed for this run, suitable for reproduction.
        seed: u64,
        /// 1-based index of the failing case among kept (non-discarded) cases.
        attempt: usize,
        /// Number of cases discarded before the failure.
        discarded: usize,
    },
    /// Run aborted before completion (e.g. too many discards).
    Aborted {
        /// Why the run aborted.
        reason: String,
        /// Number of cases that ran successfully before the abort.
        cases: usize,
        /// Number of cases discarded.
        discarded: usize,
        /// Seed used for the run.
        seed: u64,
    },
}

/// Runs `prop` against [`Config::default`] and returns the [`Outcome`].
pub fn forall<A, F>(prop: F) -> Outcome<A>
where
    A: Arbitrary,
    F: FnMut(&A) -> bool,
{
    forall_with(Config::default(), prop)
}

/// Runs `prop` with a custom [`Config`] and returns the [`Outcome`].
pub fn forall_with<A, F>(cfg: Config, mut prop: F) -> Outcome<A>
where
    A: Arbitrary,
    F: FnMut(&A) -> bool,
{
    if cfg.silence_panic_hook {
        let _guard = SilentPanicHook::install();
        run_loop(&cfg, &mut prop)
    } else {
        run_loop(&cfg, &mut prop)
    }
}

/// Convenience wrapper that converts [`Outcome::Failed`] into a `panic!`,
/// making it suitable for use directly inside `#[test]` functions.
pub fn run<A, F>(name: &str, prop: F)
where
    A: Arbitrary,
    F: FnMut(&A) -> bool,
{
    run_with(name, Config::default(), prop)
}

/// Same as [`run`] but takes an explicit [`Config`].
pub fn run_with<A, F>(name: &str, cfg: Config, prop: F)
where
    A: Arbitrary,
    F: FnMut(&A) -> bool,
{
    let seed = cfg.seed;
    match forall_with(cfg, prop) {
        Outcome::Passed { cases, discarded } => {
            if discarded == 0 {
                eprintln!("[propcheck] {name}: ok ({cases} cases, seed={seed})");
            } else {
                eprintln!(
                    "[propcheck] {name}: ok ({cases} cases, {discarded} discarded, seed={seed})"
                );
            }
        }
        Outcome::Failed {
            original,
            shrunk,
            message,
            seed,
            attempt,
            discarded,
        } => {
            panic!(
                "[propcheck] {name} FAILED at case #{attempt} (PROPCHECK_SEED={seed}, {discarded} discarded)\n  \
                 reason:   {message}\n  \
                 original: {original:?}\n  \
                 shrunk:   {shrunk:?}",
            );
        }
        Outcome::Aborted {
            reason,
            cases,
            discarded,
            seed,
        } => {
            panic!(
                "[propcheck] {name} ABORTED (seed={seed})\n  \
                 reason: {reason}\n  \
                 cases ran: {cases}, discarded: {discarded}",
            );
        }
    }
}

fn run_loop<A, F>(cfg: &Config, prop: &mut F) -> Outcome<A>
where
    A: Arbitrary,
    F: FnMut(&A) -> bool,
{
    let mut rng = XorShift64::seed_from_u64(cfg.seed);
    let target_cases = cfg.cases.max(1);
    let mut passed = 0usize;
    let mut discarded = 0usize;

    while passed < target_cases {
        if discarded > cfg.max_discards {
            return Outcome::Aborted {
                reason: format!(
                    "too many discards: {discarded} (limit {max}) before {target_cases} cases completed",
                    max = cfg.max_discards
                ),
                cases: passed,
                discarded,
                seed: cfg.seed,
            };
        }
        // Grow size with passed_count, not total attempts.
        let size = 1 + (passed * cfg.max_size / target_cases).min(cfg.max_size);
        let val: A = A::arbitrary(&mut rng, size);
        match run_prop(prop, &val) {
            CaseOutcome::Pass => {
                passed += 1;
            }
            CaseOutcome::Discard => {
                discarded += 1;
            }
            CaseOutcome::Fail(message) => {
                let shrunk = shrink_failure(val.clone(), prop, cfg.max_shrinks);
                return Outcome::Failed {
                    original: val,
                    shrunk,
                    message,
                    seed: cfg.seed,
                    attempt: passed + 1,
                    discarded,
                };
            }
        }
    }
    Outcome::Passed {
        cases: passed,
        discarded,
    }
}

/// Result of running the property on one case.
pub(crate) enum CaseOutcome {
    Pass,
    Fail(String),
    Discard,
}

pub(crate) fn run_prop<A, F: FnMut(&A) -> bool>(prop: &mut F, val: &A) -> CaseOutcome {
    match panic::catch_unwind(AssertUnwindSafe(|| prop(val))) {
        Ok(true) => CaseOutcome::Pass,
        Ok(false) => CaseOutcome::Fail("property returned false".to_string()),
        Err(payload) => classify_panic(&payload),
    }
}

pub(crate) fn classify_panic(payload: &Box<dyn Any + Send>) -> CaseOutcome {
    if payload.downcast_ref::<PropDiscard>().is_some() {
        return CaseOutcome::Discard;
    }
    if let Some(f) = payload.downcast_ref::<PropAssertFailure>() {
        return CaseOutcome::Fail(f.message.clone());
    }
    if let Some(s) = payload.downcast_ref::<&'static str>() {
        return CaseOutcome::Fail(format!("panicked: {s}"));
    }
    if let Some(s) = payload.downcast_ref::<String>() {
        return CaseOutcome::Fail(format!("panicked: {s}"));
    }
    CaseOutcome::Fail("<non-string panic payload>".to_string())
}

fn shrink_failure<A, F>(initial: A, prop: &mut F, max: usize) -> A
where
    A: Arbitrary,
    F: FnMut(&A) -> bool,
{
    let mut current = initial;
    let mut attempts = 0;
    'outer: loop {
        let candidates: Vec<A> = current.shrink().collect();
        if candidates.is_empty() {
            return current;
        }
        for c in candidates {
            if attempts >= max {
                return current;
            }
            attempts += 1;
            // Only treat actual failures as "still failing"; discards count
            // as passes for shrinking purposes (we can't minimize toward a
            // value the property never sees).
            if matches!(run_prop(prop, &c), CaseOutcome::Fail(_)) {
                current = c;
                continue 'outer;
            }
        }
        return current;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cfg(seed: u64) -> Config {
        Config {
            cases: 200,
            seed,
            max_shrinks: 512,
            max_size: 50,
            max_discards: 5000,
            silence_panic_hook: false,
        }
    }

    #[test]
    fn passes_for_true_property() {
        match forall_with(cfg(1), |&(a, b): &(i32, i32)| {
            a.wrapping_add(b) == b.wrapping_add(a)
        }) {
            Outcome::Passed { cases, .. } => assert_eq!(cases, 200),
            other => panic!("expected pass, got {other:?}"),
        }
    }

    #[test]
    fn shrinks_to_small_counterexample() {
        let outcome = forall_with(cfg(42), |v: &Vec<i32>| !v.contains(&7));
        match outcome {
            Outcome::Failed { shrunk, .. } => {
                assert_eq!(shrunk, vec![7]);
            }
            other => panic!("expected failure, got {other:?}"),
        }
    }

    #[test]
    fn captures_panic_message() {
        let outcome = forall_with(cfg(7), |&n: &u8| {
            if n > 50 {
                panic!("oh no: {n}");
            }
            true
        });
        match outcome {
            Outcome::Failed {
                message, shrunk, ..
            } => {
                assert!(
                    message.starts_with("panicked: oh no"),
                    "message was {message:?}"
                );
                assert_eq!(shrunk, 51);
            }
            other => panic!("expected failure, got {other:?}"),
        }
    }

    #[test]
    fn prop_assert_carries_message() {
        let outcome = forall_with(cfg(11), |&n: &u8| {
            crate::prop_assert!(n < 100, "expected n < 100, got {n}");
            true
        });
        match outcome {
            Outcome::Failed { message, .. } => {
                assert!(message.contains("expected n < 100"));
                assert!(message.contains("prop_assert!"));
            }
            other => panic!("expected failure, got {other:?}"),
        }
    }

    #[test]
    fn prop_assert_eq_shows_both_sides() {
        // n is never equal to n.wrapping_add(1), so this fails on case #1
        // and the message must include both sides.
        let outcome = forall_with(cfg(13), |&n: &u8| {
            crate::prop_assert_eq!(n, n.wrapping_add(1));
            true
        });
        match outcome {
            Outcome::Failed { message, .. } => {
                assert!(message.contains("left:"));
                assert!(message.contains("right:"));
            }
            other => panic!("expected failure, got {other:?}"),
        }
    }

    #[test]
    fn prop_assume_discards_unwanted_cases() {
        let cfg = Config {
            cases: 50,
            ..cfg(17)
        };
        let outcome = forall_with(cfg, |&n: &i32| {
            crate::prop_assume!(n > 0);
            n > 0
        });
        match outcome {
            Outcome::Passed { cases, discarded } => {
                assert_eq!(cases, 50);
                assert!(discarded > 0, "expected some discards from negative inputs");
            }
            other => panic!("expected pass, got {other:?}"),
        }
    }

    #[test]
    fn too_many_discards_aborts() {
        let cfg = Config {
            cases: 100,
            max_discards: 50,
            ..cfg(19)
        };
        let outcome = forall_with(cfg, |_: &i32| {
            crate::prop_assume!(false);
            true
        });
        match outcome {
            Outcome::Aborted { discarded, .. } => {
                assert!(discarded > 50);
            }
            other => panic!("expected abort, got {other:?}"),
        }
    }

    #[test]
    fn same_seed_reproduces_failure() {
        let a = forall_with(cfg(123), |&n: &u16| n < 200);
        let b = forall_with(cfg(123), |&n: &u16| n < 200);
        match (a, b) {
            (
                Outcome::Failed {
                    original: o1,
                    attempt: a1,
                    ..
                },
                Outcome::Failed {
                    original: o2,
                    attempt: a2,
                    ..
                },
            ) => {
                assert_eq!(o1, o2);
                assert_eq!(a1, a2);
            }
            _ => panic!("both should fail at the same case"),
        }
    }
}
